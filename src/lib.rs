#![feature(format_args_capture)]
extern crate console_error_panic_hook;
#[macro_use]
extern crate lazy_static;

mod math;
mod vec3;

use std::cell::RefCell;
use std::f64::consts::PI;
use std::rc::Rc;
use vec3::{Point, Vec3};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HtmlParagraphElement;
use web_sys::{
    HtmlAnchorElement, HtmlScriptElement, WebGl2RenderingContext, WebGlFramebuffer, WebGlProgram,
    WebGlShader, WebGlTexture,
};

pub const WIDTH: u32 = 1280;
pub const HEIGHT: u32 = 720;
pub const BYTES_PER_PIXEL: u32 = 4;
pub const PIXEL_ARRAY_LENGTH: usize = (WIDTH * HEIGHT * BYTES_PER_PIXEL) as usize;
pub const ASPECT_RATIO: f64 = (WIDTH as f64) / (HEIGHT as f64);
pub const SAMPLES_PER_PIXEL: u32 = 1;
pub const MAX_DEPTH: u32 = 10;
pub const FOCAL_LENGTH: f64 = 1.0;
pub const CAMERA_ORIGIN: Point = Point(0., 0., 0.);
pub const CAMERA_FIELD_OF_VIEW: f64 = 90.;
lazy_static! {
    static ref CAMERA_FIELD_OF_VIEW_RADIANS: f64 = (CAMERA_FIELD_OF_VIEW * PI) / 180.;
    static ref CAMERA_H: f64 = (*CAMERA_FIELD_OF_VIEW_RADIANS / 2.).tan();
    static ref VIEWPORT_HEIGHT: f64 = 2. * (*CAMERA_H);
    static ref VIEWPORT_WIDTH: f64 = ASPECT_RATIO * (*VIEWPORT_HEIGHT);
    static ref VIEWPORT_HORIZONTAL_VEC: Vec3 = Vec3(*VIEWPORT_WIDTH, 0., 0.);
    static ref VIEWPORT_VERTICAL_VEC: Vec3 = Vec3(0., *VIEWPORT_HEIGHT, 0.);
    static ref LOWER_LEFT_CORNER: Point = Vec3(
        CAMERA_ORIGIN.0 - VIEWPORT_HORIZONTAL_VEC.0 / 2.,
        CAMERA_ORIGIN.1 - VIEWPORT_VERTICAL_VEC.1 / 2.,
        CAMERA_ORIGIN.2 - FOCAL_LENGTH,
    );
}

pub const SIMPLE_QUAD_VERTICES: [f32; 12] = [
    -1.0, 1.0, 1.0, 1.0, -1.0, -1.0, -1.0, -1.0, 1.0, 1.0, 1.0, -1.0,
];
/// If the render should render incrementally, averaging together previous frames
pub const SHOULD_AVERAGE: bool = true;
/// Unless averaging is taking place, this is set to false after revery render
/// only updated back to true if something changes (i.e. input)
static mut SHOULD_RENDER: bool = true;
/// Whether the browser should save a screenshot of the canvas
static mut SHOULD_SAVE: bool = false;
/// Used to alternate which framebuffer to render to
static mut EVEN_ODD_COUNT: u32 = 0;
/// Used for averaging previous frames together
static mut RENDER_COUNT: f32 = 0.;
/// The weight of the last frame compared to the each frame before.
pub const LAST_FRAME_WEIGHT: f32 = 1.;
/// Limiting the counted renders allows creating a sliding average of frames
static mut MAX_RENDER_COUNT: f32 = 10000.;
static mut PREV_NOW: f64 = 0.;
static mut PREV_FPS_UPDATE_TIME: f64 = 0.;
static mut PREV_FPS: [f64; 50] = [0.; 50];

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

fn create_texture(gl: &WebGl2RenderingContext) -> WebGlTexture {
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
        WIDTH as i32,
        HEIGHT as i32,
        0,
        WebGl2RenderingContext::RGBA,
        WebGl2RenderingContext::UNSIGNED_BYTE,
        None,
    )
    .unwrap();

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

fn draw(gl: &WebGl2RenderingContext) {
    gl.clear_color(0.0, 0.0, 0.0, 1.0);
    gl.viewport(0, 0, WIDTH as i32, HEIGHT as i32);
    gl.clear(WebGl2RenderingContext::COLOR_BUFFER_BIT);
    gl.draw_arrays(
        WebGl2RenderingContext::TRIANGLES,
        0,
        (SIMPLE_QUAD_VERTICES.len() / 2) as i32,
    );
}

fn update_fps_indicator(p: &HtmlParagraphElement, now: f64) {
    unsafe {
        if now - PREV_FPS_UPDATE_TIME > 250. {
            PREV_FPS_UPDATE_TIME = now;
            let average_fps: f64 = PREV_FPS.iter().sum::<f64>() / (PREV_FPS.len() as f64);
            p.set_text_content(Some(&format!("{:.2} fps", average_fps)))
        }
    }
}

fn update_moving_fps_array(now: f64) {
    unsafe {
        // calculate moving fps
        let dt = now - PREV_NOW;
        PREV_NOW = now;
        let fps = 1000. / dt;
        for (i, el) in PREV_FPS.into_iter().skip(1).enumerate() {
            PREV_FPS[i] = el;
        }
        PREV_FPS[PREV_FPS.len() - 1] = fps;
    }
}

fn update_render_globals() {
    unsafe {
        if !SHOULD_AVERAGE {
            // only continuously render when averaging is being done
            SHOULD_RENDER = false;
        }
        EVEN_ODD_COUNT += 1;
        RENDER_COUNT = (RENDER_COUNT + 1.).min(MAX_RENDER_COUNT);
    };
}

#[wasm_bindgen]
pub fn save_image() -> Result<(), JsValue> {
    unsafe {
        SHOULD_RENDER = true;
        SHOULD_SAVE = true;
    }

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

    canvas.set_width(WIDTH);
    canvas.set_height(HEIGHT);

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
    let textures = [create_texture(&gl), create_texture(&gl)];

    // CREATE FRAMEBUFFER & ATTACH TEXTURE
    let framebuffer_objects = [
        create_framebuffer(&gl, &textures[0]),
        create_framebuffer(&gl, &textures[1]),
    ];

    // RENDER LOOP
    let f = Rc::new(RefCell::new(None));
    let g = f.clone();
    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        if unsafe { SHOULD_RENDER } {
            let now = window.performance().unwrap().now();
            update_render_globals();
            update_moving_fps_array(now);

            // SET UNIFORMS
            gl.uniform1i(texture_u_location.as_ref(), 0);
            gl.uniform1f(width_u_location.as_ref(), WIDTH as f32);
            gl.uniform1f(height_u_location.as_ref(), HEIGHT as f32);
            gl.uniform1i(max_depth_u_location.as_ref(), MAX_DEPTH as i32);
            gl.uniform1f(time_u_location.as_ref(), now as f32);
            gl.uniform1i(
                samples_per_pixel_u_location.as_ref(),
                SAMPLES_PER_PIXEL as i32,
            );
            gl.uniform1f(aspect_ratio_u_location.as_ref(), ASPECT_RATIO as f32);
            gl.uniform1f(viewport_height_u_location.as_ref(), *VIEWPORT_HEIGHT as f32);
            gl.uniform1f(focal_length_u_location.as_ref(), FOCAL_LENGTH as f32);
            gl.uniform3fv_with_f32_array(
                camera_origin_u_location.as_ref(),
                &CAMERA_ORIGIN.to_array(),
            );
            gl.uniform1f(viewport_width_u_location.as_ref(), *VIEWPORT_WIDTH as f32);
            gl.uniform3fv_with_f32_array(
                viewport_horizontal_vec_u_location.as_ref(),
                &VIEWPORT_HORIZONTAL_VEC.to_array(),
            );
            gl.uniform3fv_with_f32_array(
                viewport_vertical_vec_u_location.as_ref(),
                &VIEWPORT_VERTICAL_VEC.to_array(),
            );
            gl.uniform3fv_with_f32_array(
                lower_left_corner_u_location.as_ref(),
                &LOWER_LEFT_CORNER.to_array(),
            );
            gl.uniform1f(render_count_u_location.as_ref(), unsafe { RENDER_COUNT });
            gl.uniform1i(should_average_u_location.as_ref(), SHOULD_AVERAGE as i32);
            gl.uniform1f(
                last_frame_weight_u_location.as_ref(),
                LAST_FRAME_WEIGHT as f32,
            );

            // RENDER
            // use texture previously rendered to
            gl.bind_texture(
                WebGl2RenderingContext::TEXTURE_2D,
                Some(&textures[(unsafe { EVEN_ODD_COUNT + 1 } % 2) as usize]),
            );

            // draw to canvas
            gl.bind_framebuffer(WebGl2RenderingContext::FRAMEBUFFER, None);
            draw(&gl);

            // only need to draw to framebuffer when doing averages of previous frames
            if SHOULD_AVERAGE {
                // RENDER (TO FRAMEBUFFER)
                gl.bind_framebuffer(
                    WebGl2RenderingContext::FRAMEBUFFER,
                    Some(&framebuffer_objects[(unsafe { EVEN_ODD_COUNT } % 2) as usize]),
                );
                draw(&gl);
            }

            // if user has requested to save, save immediately after rendering
            if unsafe { SHOULD_SAVE } {
                unsafe {
                    SHOULD_SAVE = false;
                }
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

            update_fps_indicator(&fps_indicator, now);
        }
        request_animation_frame(f.borrow().as_ref().unwrap());
    }) as Box<dyn FnMut()>));

    request_animation_frame(g.borrow().as_ref().unwrap());

    Ok(())
}
