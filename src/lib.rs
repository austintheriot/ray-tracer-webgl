#![feature(format_args_capture)]
extern crate console_error_panic_hook;
#[macro_use]
extern crate lazy_static;

mod math;
mod vec3;

use log::info;
use std::cell::RefCell;
use std::f64::consts::PI;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use vec3::{Point, Vec3};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HtmlParagraphElement;
use web_sys::{
    HtmlAnchorElement, HtmlScriptElement, WebGl2RenderingContext, WebGlFramebuffer, WebGlProgram,
    WebGlShader, WebGlTexture, WheelEvent,
};

pub const BYTES_PER_PIXEL: u32 = 4;

struct State {
    width: u32,
    height: u32,
    aspect_ratio: f64,
    samples_per_pixel: u32,
    max_depth: u32,
    focal_length: f64,
    camera_origin: Point,
    /// stored in radians
    camera_field_of_view: f64,
    viewport_height: f64,
    viewport_width: f64,
    viewport_horizontal_vec: Vec3,
    viewport_vertical_vec: Vec3,
    lower_left_corner: Point,
    /// If the render should render incrementally, averaging together previous frames
    should_average: bool,
    /// Unless averaging is taking place, this is set to false after revery render
    /// only updated back to true if something changes (i.e. input)
    should_render: bool,
    /// Whether the browser should save a screenshot of the canvas
    should_save: bool,
    /// Used to alternate which framebuffer to render to
    even_odd_count: u32,
    /// Used for averaging previous frames together
    render_count: f32,
    /// The weight of the last frame compared to the each frame before.
    last_frame_weight: f32,
    /// Limiting the counted renders allows creating a sliding average of frames
    max_render_count: f32,
    prev_now: f64,
    prev_fps_update_time: f64,
    prev_fps: [f64; 50],
}

impl Default for State {
    fn default() -> Self {
        let width = 1200;
        let height = 900;
        let camera_origin = Point(0., 0., 0.);
        let aspect_ratio = (width as f64) / (height as f64);
        let camera_field_of_view = PI / 2.;
        let camera_h = (camera_field_of_view / 2.).tan();
        let viewport_height = 2. * camera_h;
        let viewport_width = viewport_height * aspect_ratio;
        let viewport_horizontal_vec = Vec3(viewport_width, 0., 0.);
        let viewport_vertical_vec = Vec3(0., viewport_height, 0.);
        let focal_length = 1.;
        let lower_left_corner = Vec3(
            camera_origin.x() - viewport_horizontal_vec.0 / 2.,
            camera_origin.y() - viewport_vertical_vec.1 / 2.,
            camera_origin.z() - focal_length,
        );

        State {
            width,
            height,
            aspect_ratio,
            samples_per_pixel: 1,
            max_depth: 10,
            focal_length: 1.,
            camera_origin,
            camera_field_of_view,
            viewport_height,
            viewport_width,
            viewport_horizontal_vec,
            viewport_vertical_vec,
            lower_left_corner,
            should_average: true,
            should_render: true,
            should_save: false,
            even_odd_count: 0,
            render_count: 0.,
            last_frame_weight: 1.,
            max_render_count: 5.,
            prev_now: 0.,
            prev_fps_update_time: 0.,
            prev_fps: [0.; 50],
        }
    }
}

impl State {
    fn set_fov(&mut self, new_fov_radians: f64) {
        // update all variables dependent on this variable
        self.camera_field_of_view = new_fov_radians.clamp(0.1, PI * 0.75);
        let camera_h = (self.camera_field_of_view / 2.).tan();
        self.viewport_height = 2. * camera_h;
        self.viewport_width = self.viewport_height * self.aspect_ratio;
        self.viewport_horizontal_vec = Vec3(self.viewport_width, 0., 0.);
        self.viewport_vertical_vec = Vec3(0., self.viewport_height, 0.);
        self.focal_length = 1.;
        self.lower_left_corner = Vec3(
            self.camera_origin.x() - self.viewport_horizontal_vec.0 / 2.,
            self.camera_origin.y() - self.viewport_vertical_vec.1 / 2.,
            self.camera_origin.z() - self.focal_length,
        );

        // should render the new change
        self.should_render = true;
    }
}

unsafe impl Send for State {}
unsafe impl Sync for State {}

lazy_static! {
    static ref STATE: Arc<Mutex<State>> = Arc::new(Mutex::new(State::default()));
}

pub const SIMPLE_QUAD_VERTICES: [f32; 12] = [
    -1.0, 1.0, 1.0, 1.0, -1.0, -1.0, -1.0, -1.0, 1.0, 1.0, 1.0, -1.0,
];

fn compile_shader(
    gl: &WebGl2RenderingContext,
    shader_type: u32,
    source: &str,
) -> Result<WebGlShader, String> {
    let shader = gl
        .create_shader(shader_type)
        .ok_or_else(|| String::from("Unable to create shader object"))?;
    gl.shader_source(&shader, source);
    gl.compile_shader(&shader);

    if gl
        .get_shader_parameter(&shader, WebGl2RenderingContext::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(shader)
    } else {
        Err(gl
            .get_shader_info_log(&shader)
            .unwrap_or_else(|| String::from("Unknown error creating shader")))
    }
}

fn link_program(
    gl: &WebGl2RenderingContext,
    vert_shader: &WebGlShader,
    frag_shader: &WebGlShader,
) -> Result<WebGlProgram, String> {
    let program = gl
        .create_program()
        .ok_or_else(|| String::from("Unable to create shader object"))?;

    gl.attach_shader(&program, vert_shader);
    gl.attach_shader(&program, frag_shader);
    gl.link_program(&program);

    if gl
        .get_program_parameter(&program, WebGl2RenderingContext::LINK_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(program)
    } else {
        Err(gl
            .get_program_info_log(&program)
            .unwrap_or_else(|| String::from("Unknown error creating program object")))
    }
}

fn create_texture(gl: &WebGl2RenderingContext, state: &MutexGuard<State>) -> WebGlTexture {
    let texture = gl.create_texture();
    gl.bind_texture(WebGl2RenderingContext::TEXTURE_2D, texture.as_ref());

    // Set the parameters so we don't need mips, we're not filtering, and we don't repeat
    gl.tex_parameteri(
        WebGl2RenderingContext::TEXTURE_2D,
        WebGl2RenderingContext::TEXTURE_WRAP_S,
        WebGl2RenderingContext::CLAMP_TO_EDGE as i32,
    );
    gl.tex_parameteri(
        WebGl2RenderingContext::TEXTURE_2D,
        WebGl2RenderingContext::TEXTURE_WRAP_T,
        WebGl2RenderingContext::CLAMP_TO_EDGE as i32,
    );
    gl.tex_parameteri(
        WebGl2RenderingContext::TEXTURE_2D,
        WebGl2RenderingContext::TEXTURE_MIN_FILTER,
        WebGl2RenderingContext::LINEAR as i32,
    );
    gl.tex_parameteri(
        WebGl2RenderingContext::TEXTURE_2D,
        WebGl2RenderingContext::TEXTURE_MAG_FILTER,
        WebGl2RenderingContext::LINEAR as i32,
    );

    // load empty texture into gpu -- this will get rendered into later
    gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
        WebGl2RenderingContext::TEXTURE_2D,
        0,
        WebGl2RenderingContext::RGBA as i32,
        state.width as i32,
        state.height as i32,
        0,
        WebGl2RenderingContext::RGBA,
        WebGl2RenderingContext::UNSIGNED_BYTE,
        None,
    )
    .unwrap();
    drop(state);

    texture.unwrap()
}

fn create_framebuffer(gl: &WebGl2RenderingContext, texture: &WebGlTexture) -> WebGlFramebuffer {
    let framebuffer_object = gl.create_framebuffer();
    gl.bind_framebuffer(
        WebGl2RenderingContext::FRAMEBUFFER,
        framebuffer_object.as_ref(),
    );
    gl.framebuffer_texture_2d(
        WebGl2RenderingContext::FRAMEBUFFER,
        WebGl2RenderingContext::COLOR_ATTACHMENT0,
        WebGl2RenderingContext::TEXTURE_2D,
        Some(&texture),
        0,
    );
    framebuffer_object.unwrap()
}

fn window() -> web_sys::Window {
    web_sys::window().expect("no global `window` exists")
}

fn document() -> web_sys::Document {
    window()
        .document()
        .expect("should have a document on window")
}

fn request_animation_frame(f: &Closure<dyn FnMut()>) {
    window()
        .request_animation_frame(f.as_ref().unchecked_ref())
        .expect("should register `requestAnimationFrame` OK");
}

fn draw(gl: &WebGl2RenderingContext, state: &MutexGuard<State>) {
    gl.clear_color(0.0, 0.0, 0.0, 1.0);
    gl.viewport(0, 0, state.width as i32, state.height as i32);
    gl.clear(WebGl2RenderingContext::COLOR_BUFFER_BIT);
    gl.draw_arrays(
        WebGl2RenderingContext::TRIANGLES,
        0,
        (SIMPLE_QUAD_VERTICES.len() / 2) as i32,
    );
}

fn update_fps_indicator(p: &HtmlParagraphElement, now: f64, state: &mut MutexGuard<State>) {
    if now - state.prev_fps_update_time > 250. {
        state.prev_fps_update_time = now;
        let average_fps: f64 = state.prev_fps.iter().sum::<f64>() / (state.prev_fps.len() as f64);
        p.set_text_content(Some(&format!("{:.2} fps", average_fps)))
    }
}

fn update_moving_fps_array(now: f64, state: &mut MutexGuard<State>) {
    // calculate moving fps
    let dt = now - state.prev_now;
    state.prev_now = now;
    let fps = 1000. / dt;
    let last_index = state.prev_fps.len() - 1;
    for (i, el) in state.prev_fps.into_iter().skip(1).enumerate() {
        state.prev_fps[i] = el;
    }
    state.prev_fps[last_index] = fps;
}

fn update_render_globals(state: &mut MutexGuard<State>) {
    if !state.should_average {
        // only continuously render when averaging is being done
        state.should_render = false;
    }
    state.even_odd_count += 1;
    state.render_count = (state.render_count + 1.).min(state.max_render_count);
}

fn degrees_to_radians(degrees: f64) -> f64 {
    (degrees * PI) / 180.
}

#[wasm_bindgen]
pub fn increase_fov(degrees: f64) {
    // can take a mutex guard here, because it will never be called while render loop is running
    let mut state = (*STATE).lock().unwrap();
    let radians = degrees_to_radians(degrees);
    state.camera_field_of_view += radians;
}

#[wasm_bindgen]
pub fn handle_wheel(e: WheelEvent) {
    // can take a mutex guard here, because it will never be called while render loop is running
    let mut state = (*STATE).lock().unwrap();
    let adjustment = 1. * e.delta_y().signum();
    let radians = degrees_to_radians(adjustment);
    let new_value = state.camera_field_of_view + radians;
    state.set_fov(new_value);
}

#[wasm_bindgen]
pub fn save_image() -> Result<(), JsValue> {
    // can take a mutex guard here, because it will never be called while render loop is running
    let mut state = (*STATE).lock().unwrap();
    state.should_render = true;
    state.should_save = true;

    Ok(())
}

/// Entry function cannot be async, so spawns a local Future for running the real main function
#[wasm_bindgen]
pub fn main() -> Result<(), JsValue> {
    // enables ore helpful stack traces
    console_error_panic_hook::set_once();
    wasm_logger::init(wasm_logger::Config::default());

    // GET ELEMENTS
    let window = window();
    let document = document();
    let canvas = document
        .query_selector("canvas")?
        .unwrap()
        .dyn_into::<web_sys::HtmlCanvasElement>()?;

    let gl = canvas
        .get_context("webgl2")?
        .unwrap()
        .dyn_into::<WebGl2RenderingContext>()?;

    let fps_indicator = document
        .query_selector("#fps")?
        .unwrap()
        .dyn_into::<web_sys::HtmlParagraphElement>()?;

    let state = (*STATE).lock().unwrap();
    canvas.set_width(state.width);
    canvas.set_height(state.height);
    drop(state);

    // ADD LISTENERS
    let handle_wheel = Closure::wrap(Box::new(handle_wheel) as Box<dyn FnMut(WheelEvent)>);
    window.set_onwheel(Some(handle_wheel.as_ref().unchecked_ref()));
    handle_wheel.forget();

    //  SETUP PROGRAM
    let vertex_shader = {
        let shader_source = &document
            .query_selector("#vertex-shader")?
            .unwrap()
            .dyn_into::<HtmlScriptElement>()?
            .text()?;
        compile_shader(&gl, WebGl2RenderingContext::VERTEX_SHADER, shader_source).unwrap()
    };
    let fragment_shader = {
        let shader_source = &document
            .query_selector("#fragment-shader")?
            .unwrap()
            .dyn_into::<HtmlScriptElement>()?
            .text()?;
        compile_shader(&gl, WebGl2RenderingContext::FRAGMENT_SHADER, shader_source).unwrap()
    };
    let program = link_program(&gl, &vertex_shader, &fragment_shader)?;
    gl.use_program(Some(&program));

    // GET LOCATIONS
    let vertex_attribute_position = gl.get_attrib_location(&program, "a_position") as u32;
    let texture_u_location = gl.get_uniform_location(&program, "u_texture");
    let width_u_location = gl.get_uniform_location(&program, "u_width");
    let height_u_location = gl.get_uniform_location(&program, "u_height");
    let time_u_location = gl.get_uniform_location(&program, "u_time");
    let samples_per_pixel_u_location = gl.get_uniform_location(&program, "u_samples_per_pixel");
    let aspect_ratio_u_location = gl.get_uniform_location(&program, "u_aspect_ratio");
    let viewport_height_u_location = gl.get_uniform_location(&program, "u_viewport_height");
    let viewport_width_u_location = gl.get_uniform_location(&program, "u_viewport_width");
    let focal_length_u_location = gl.get_uniform_location(&program, "u_focal_length");
    let camera_origin_u_location = gl.get_uniform_location(&program, "u_camera_origin");
    let viewport_horizontal_vec_u_location =
        gl.get_uniform_location(&program, "u_viewport_horizontal_vec");
    let viewport_vertical_vec_u_location =
        gl.get_uniform_location(&program, "u_viewport_vertical_vec");
    let lower_left_corner_u_location = gl.get_uniform_location(&program, "u_lower_left_corner");
    let max_depth_u_location = gl.get_uniform_location(&program, "u_max_depth");
    let render_count_u_location = gl.get_uniform_location(&program, "u_render_count");
    let should_average_u_location = gl.get_uniform_location(&program, "u_should_average");
    let last_frame_weight_u_location = gl.get_uniform_location(&program, "u_last_frame_weight");

    // SET VERTEX BUFFER
    let buffer = gl.create_buffer().ok_or("failed to create buffer")?;
    gl.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&buffer));
    // requires `unsafe` since we're creating a raw view into wasm memory,
    // but this array is static, so it shouldn't cause any issues
    let vertex_array = unsafe { js_sys::Float32Array::view(&SIMPLE_QUAD_VERTICES) };
    gl.buffer_data_with_array_buffer_view(
        WebGl2RenderingContext::ARRAY_BUFFER,
        &vertex_array,
        WebGl2RenderingContext::STATIC_DRAW,
    );
    gl.enable_vertex_attrib_array(vertex_attribute_position);
    gl.vertex_attrib_pointer_with_i32(
        vertex_attribute_position,
        2,
        WebGl2RenderingContext::FLOAT,
        false,
        0,
        0,
    );

    // CREATE TEXTURE
    let state = (*STATE).lock().unwrap();
    let textures = [create_texture(&gl, &state), create_texture(&gl, &state)];
    drop(state);

    // CREATE FRAMEBUFFER & ATTACH TEXTURE
    let framebuffer_objects = [
        create_framebuffer(&gl, &textures[0]),
        create_framebuffer(&gl, &textures[1]),
    ];

    // RENDER LOOP
    let f = Rc::new(RefCell::new(None));
    let g = f.clone();
    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        // it's ok to borrow this as mutable for the entire block,
        // since it is synchronous and no other function calls can
        // try to lock the mutex while it is in use
        let mut state = (*STATE).lock().unwrap();
        if state.should_render {
            let now = window.performance().unwrap().now();

            update_render_globals(&mut state);
            update_moving_fps_array(now, &mut state);

            // SET UNIFORMS
            gl.uniform1i(texture_u_location.as_ref(), 0);
            gl.uniform1f(width_u_location.as_ref(), state.width as f32);
            gl.uniform1f(height_u_location.as_ref(), state.height as f32);
            gl.uniform1i(max_depth_u_location.as_ref(), state.max_depth as i32);
            gl.uniform1f(time_u_location.as_ref(), now as f32);
            gl.uniform1i(
                samples_per_pixel_u_location.as_ref(),
                state.samples_per_pixel as i32,
            );
            gl.uniform1f(aspect_ratio_u_location.as_ref(), state.aspect_ratio as f32);
            gl.uniform1f(
                viewport_height_u_location.as_ref(),
                state.viewport_height as f32,
            );
            gl.uniform1f(focal_length_u_location.as_ref(), state.focal_length as f32);
            gl.uniform3fv_with_f32_array(
                camera_origin_u_location.as_ref(),
                &state.camera_origin.to_array(),
            );
            gl.uniform1f(
                viewport_width_u_location.as_ref(),
                state.viewport_width as f32,
            );
            gl.uniform3fv_with_f32_array(
                viewport_horizontal_vec_u_location.as_ref(),
                &state.viewport_horizontal_vec.to_array(),
            );
            gl.uniform3fv_with_f32_array(
                viewport_vertical_vec_u_location.as_ref(),
                &state.viewport_vertical_vec.to_array(),
            );
            gl.uniform3fv_with_f32_array(
                lower_left_corner_u_location.as_ref(),
                &state.lower_left_corner.to_array(),
            );
            gl.uniform1f(render_count_u_location.as_ref(), state.render_count);
            gl.uniform1i(
                should_average_u_location.as_ref(),
                state.should_average as i32,
            );
            gl.uniform1f(
                last_frame_weight_u_location.as_ref(),
                state.last_frame_weight as f32,
            );

            // RENDER
            // use texture previously rendered to
            gl.bind_texture(
                WebGl2RenderingContext::TEXTURE_2D,
                Some(&textures[((state.even_odd_count + 1) % 2) as usize]),
            );

            // draw to canvas
            gl.bind_framebuffer(WebGl2RenderingContext::FRAMEBUFFER, None);
            draw(&gl, &state);

            // only need to draw to framebuffer when doing averages of previous frames
            if state.should_average {
                // RENDER (TO FRAMEBUFFER)
                gl.bind_framebuffer(
                    WebGl2RenderingContext::FRAMEBUFFER,
                    Some(&framebuffer_objects[(state.even_odd_count % 2) as usize]),
                );
                draw(&gl, &state);
            }

            // if user has requested to save, save immediately after rendering
            if state.should_save {
                state.should_save = false;
                let data_url = canvas
                    .to_data_url()
                    .unwrap()
                    .replace("image/png", "image/octet-stream");
                let a = document
                    .create_element("a")
                    .unwrap()
                    .dyn_into::<HtmlAnchorElement>()
                    .unwrap();

                a.set_href(&data_url);
                a.set_download("canvas.png");
                a.click();
            }

            update_fps_indicator(&fps_indicator, now, &mut state);
        }

        request_animation_frame((*f).borrow().as_ref().unwrap());
    }) as Box<dyn FnMut()>));

    request_animation_frame((*g).borrow().as_ref().unwrap());

    Ok(())
}
