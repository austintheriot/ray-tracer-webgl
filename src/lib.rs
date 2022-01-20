#![feature(format_args_capture)]
extern crate console_error_panic_hook;
#[macro_use]
extern crate lazy_static;

mod m4;
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
use web_sys::Element;
use web_sys::HtmlParagraphElement;
use web_sys::MouseEvent;
use web_sys::{
    Event, HtmlAnchorElement, HtmlButtonElement, HtmlDivElement, HtmlScriptElement, KeyboardEvent,
    WebGl2RenderingContext, WebGlFramebuffer, WebGlProgram, WebGlShader, WebGlTexture, WheelEvent,
};

pub const BYTES_PER_PIXEL: u32 = 4;
pub const LOOK_SENSITIVITY: f64 = 1.;
pub const MOVEMENT_SPEED: f64 = 0.001;
pub const VELOCITY_DAMPING: f64 = 0.5;

#[derive(Default, Debug)]
struct KeydownMap {
    w: bool,
    a: bool,
    s: bool,
    d: bool,
    space: bool,
    shift: bool,
}

impl KeydownMap {
    pub fn all_false(&self) -> bool {
        !self.w && !self.a && !self.s && !self.d && !self.space && !self.shift
    }
}

struct State {
    width: u32,
    height: u32,
    aspect_ratio: f64,
    samples_per_pixel: u32,
    max_depth: u32,
    focal_length: f64,
    camera_origin: Point,
    pitch: f64,
    yaw: f64,
    camera_front: Point,
    vup: Vec3,
    /// stored in radians
    camera_field_of_view: f64,
    viewport_height: f64,
    viewport_width: f64,
    horizontal: Vec3,
    vertical: Vec3,
    lower_left_corner: Point,

    // RENDER STATE
    /// is the modal up that asks the user to enable first-person viewing mode?
    is_paused: bool,
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
    render_count: u32,
    /// The weight of the last frame compared to the each frame before.
    last_frame_weight: f32,
    /// Limiting the counted renders allows creating a sliding average of frames
    max_render_count: u32,
    /// Used for calculating time delta in animation loop
    prev_now: f64,

    // MOVEMENT
    keydown_map: KeydownMap,
    look_sensitivity: f64,

    // ANALYTICS
    prev_fps_update_time: f64,
    prev_fps: [f64; 50],
}

impl Default for State {
    fn default() -> Self {
        let width = 1200;
        let height = 900;
        let aspect_ratio = (width as f64) / (height as f64);
        let camera_field_of_view = PI / 2.;
        let camera_h = (camera_field_of_view / 2.).tan();
        let camera_origin = Point(0., 0., 0.);
        let pitch = 0.;
        let yaw = -90.; // look down the z axis by default
        let camera_front = Point(
            f64::cos(degrees_to_radians(yaw)) * f64::cos(degrees_to_radians(pitch)),
            f64::sin(degrees_to_radians(pitch)),
            f64::sin(degrees_to_radians(yaw)) * f64::cos(degrees_to_radians(pitch)),
        );
        let look_at = &camera_origin + &camera_front;
        let vup = Vec3(0., 1., 0.);
        let w = Vec3::normalize(&camera_origin - &look_at);
        let u = Vec3::normalize(Vec3::cross(&vup, &w));
        let v = Vec3::cross(&w, &u);
        let viewport_height = 2. * camera_h;
        let viewport_width = viewport_height * aspect_ratio;
        let horizontal = viewport_width * u;
        let vertical = viewport_height * v;
        let focal_length = 1.;
        let lower_left_corner = &camera_origin - &horizontal / 2. - &vertical / 2. - w;

        let samples_per_pixel = 1;
        let max_depth = 10;
        let should_average = true;
        let should_render = true;
        let should_save = false;
        let even_odd_count = 0;
        let render_count = 0;
        let last_frame_weight = 1.;
        let max_render_count = 100_000;
        let prev_now = 0.;

        let is_paused = true;

        let look_sensitivity = 0.1;
        let keydown_map = KeydownMap::default();

        let prev_fps_update_time = 0.;
        let prev_fps = [0.; 50];

        State {
            width,
            height,
            aspect_ratio,
            samples_per_pixel,
            max_depth,
            focal_length,
            pitch,
            yaw,
            camera_origin,
            camera_front,
            vup,
            camera_field_of_view,
            viewport_height,
            viewport_width,
            horizontal: horizontal,
            vertical: vertical,
            lower_left_corner,

            is_paused,
            should_average,
            should_render,
            should_save,
            even_odd_count,
            render_count,
            last_frame_weight,
            max_render_count,
            prev_now,

            prev_fps_update_time,
            prev_fps,

            keydown_map,
            look_sensitivity,
        }
    }
}

impl State {
    // updates all "downstream" variables once a rendering/camera variable has been changed
    fn update_pipeline(&mut self) {
        self.aspect_ratio = (self.width as f64) / (self.height as f64);
        let camera_h = (self.camera_field_of_view / 2.).tan();
        self.camera_front = Point(
            f64::cos(degrees_to_radians(self.yaw)) * f64::cos(degrees_to_radians(self.pitch)),
            f64::sin(degrees_to_radians(self.pitch)),
            f64::sin(degrees_to_radians(self.yaw)) * f64::cos(degrees_to_radians(self.pitch)),
        );
        let look_at = &self.camera_origin + &self.camera_front;
        let w = Vec3::normalize(&self.camera_origin - &look_at);
        let u = Vec3::normalize(Vec3::cross(&self.vup, &w));
        let v = Vec3::cross(&w, &u);
        self.viewport_height = 2. * camera_h;
        self.viewport_width = self.viewport_height * self.aspect_ratio;
        self.horizontal = self.viewport_width * u;
        self.vertical = self.viewport_height * v;
        self.lower_left_corner =
            &self.camera_origin - &self.horizontal / 2. - &self.vertical / 2. - w;

        self.render_count = 0;
        self.should_render = true;
    }

    fn set_fov(&mut self, new_fov_radians: f64) {
        self.camera_field_of_view = new_fov_radians.clamp(0.1, PI * 0.75);
        self.update_pipeline();
    }

    fn set_camera_angles(&mut self, yaw: f64, pitch: f64) {
        self.yaw = yaw;
        self.pitch = f64::clamp(pitch, -89., 89.);
        self.update_pipeline();
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

fn update_moving_fps_array(now: f64, state: &mut MutexGuard<State>, dt: f64) {
    // calculate moving fps
    state.prev_now = now;
    let fps = 1000. / dt;
    let last_index = state.prev_fps.len() - 1;
    for (i, el) in state.prev_fps.into_iter().skip(1).enumerate() {
        state.prev_fps[i] = el;
    }
    state.prev_fps[last_index] = fps;
}

fn update_position(state: &mut MutexGuard<State>, dt: f64) {
    if state.keydown_map.all_false() {
        return;
    }

    let camera_front = state.camera_front.clone();
    let vup = state.vup.clone();
    if state.keydown_map.w {
        state.camera_origin += &camera_front * MOVEMENT_SPEED * dt;
    }
    if state.keydown_map.a {
        state.camera_origin -= Vec3::cross(&camera_front, &vup) * MOVEMENT_SPEED * dt;
    }
    if state.keydown_map.s {
        state.camera_origin -= &camera_front * MOVEMENT_SPEED * dt;
    }
    if state.keydown_map.d {
        state.camera_origin += Vec3::cross(&camera_front, &vup) * MOVEMENT_SPEED * dt;
    }
    if state.keydown_map.space {
        state.camera_origin += &vup * MOVEMENT_SPEED * dt;
    }
    if state.keydown_map.shift {
        state.camera_origin -= &vup * MOVEMENT_SPEED * dt;
    }

    state.update_pipeline();
}

fn update_render_globals(state: &mut MutexGuard<State>) {
    if !state.should_average {
        // only continuously render when averaging is being done
        state.should_render = false;
    }
    state.even_odd_count += 1;
    state.render_count = (state.render_count + 1).min(state.max_render_count);
}

fn degrees_to_radians(degrees: f64) -> f64 {
    (degrees * PI) / 180.
}

pub fn increase_fov(degrees: f64) {
    // can take a mutex guard here, because it will never be called while render loop is running
    let mut state = (*STATE).lock().unwrap();
    let radians = degrees_to_radians(degrees);
    state.camera_field_of_view += radians;
}

pub fn handle_wheel(e: WheelEvent) {
    // can take a mutex guard here, because it will never be called while render loop is running
    let mut state = (*STATE).lock().unwrap();
    let adjustment = 1. * e.delta_y().signum();
    let radians = degrees_to_radians(adjustment);
    let new_value = state.camera_field_of_view + radians;
    state.set_fov(new_value);
}

pub fn handle_keydown(e: KeyboardEvent) {
    // can take a mutex guard here, because it will never be called while render loop is running
    let mut state = (*STATE).lock().unwrap();
    match e.key().as_str() {
        "w" => state.keydown_map.w = true,
        "a" => state.keydown_map.a = true,
        "s" => state.keydown_map.s = true,
        "d" => state.keydown_map.d = true,
        " " => state.keydown_map.space = true,
        "Shift" => state.keydown_map.shift = true,
        _ => {}
    }
    info!("{:#?}", state.keydown_map);
}

pub fn handle_keyup(e: KeyboardEvent) {
    // can take a mutex guard here, because it will never be called while render loop is running
    let mut state = (*STATE).lock().unwrap();
    match e.key().as_str() {
        "w" => state.keydown_map.w = false,
        "a" => state.keydown_map.a = false,
        "s" => state.keydown_map.s = false,
        "d" => state.keydown_map.d = false,
        " " => state.keydown_map.space = false,
        "Shift" => state.keydown_map.shift = false,
        _ => {}
    }
    info!("{:#?}", state.keydown_map);
}

pub fn handle_mouse_move(e: MouseEvent) {
    let mut state = (*STATE).lock().unwrap();
    let dx = (e.movement_x() as f64) * state.look_sensitivity;
    let dy = -(e.movement_y() as f64) * state.look_sensitivity;
    let yaw = state.yaw + dx;
    let pitch = state.pitch + dy;
    state.set_camera_angles(yaw, pitch);
}

pub fn handle_save_image(_: MouseEvent) {
    // can take a mutex guard here, because it will never be called while render loop is running
    let mut state = (*STATE).lock().unwrap();
    state.should_render = true;
    state.should_save = true;
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

    let enable_button = document
        .query_selector("#enable")?
        .unwrap()
        .dyn_into::<HtmlButtonElement>()?;

    let save_image_button = document
        .query_selector("#save-image")?
        .unwrap()
        .dyn_into::<HtmlButtonElement>()?;

    let backdrop = document
        .query_selector("#backdrop")?
        .unwrap()
        .dyn_into::<HtmlDivElement>()?;

    let state = (*STATE).lock().unwrap();
    canvas.set_width(state.width);
    canvas.set_height(state.height);
    drop(state);

    // ADD LISTENERS
    // not planning on removing any of these listeners for the
    // duration of the program, so using `forget()` here is fine for now
    let handle_wheel = Closure::wrap(Box::new(handle_wheel) as Box<dyn FnMut(WheelEvent)>);
    window.set_onwheel(Some(handle_wheel.as_ref().unchecked_ref()));
    handle_wheel.forget();

    let handle_save_image =
        Closure::wrap(Box::new(handle_save_image) as Box<dyn FnMut(MouseEvent)>);
    save_image_button.set_onclick(Some(handle_save_image.as_ref().unchecked_ref()));
    handle_save_image.forget();

    let handle_keydown = Closure::wrap(Box::new(handle_keydown) as Box<dyn FnMut(KeyboardEvent)>);
    window.set_onkeydown(Some(handle_keydown.as_ref().unchecked_ref()));
    handle_keydown.forget();

    let handle_keyup = Closure::wrap(Box::new(handle_keyup) as Box<dyn FnMut(KeyboardEvent)>);
    window.set_onkeyup(Some(handle_keyup.as_ref().unchecked_ref()));
    handle_keyup.forget();

    let handle_enable_button_click = {
        let canvas = canvas.clone();
        Closure::wrap(Box::new(move |_| {
            let element: &Element = canvas.as_ref();
            element.request_pointer_lock();
        }) as Box<dyn FnMut(MouseEvent)>)
    };
    enable_button.set_onclick(Some(handle_enable_button_click.as_ref().unchecked_ref()));
    handle_enable_button_click.forget();

    let handle_onpointerlockchange = {
        let canvas = canvas.clone();
        let document = document.clone();
        let state = STATE.clone();
        Closure::wrap(Box::new(move |_| {
            if let Some(pointer_lock_element) = document.pointer_lock_element() {
                let canvas_as_element: &Element = canvas.as_ref();
                if &pointer_lock_element == canvas_as_element {
                    backdrop.class_list().add_1("hide").unwrap();
                    (*state).lock().unwrap().is_paused = false;
                    return;
                }
            }
            backdrop.class_list().remove_1("hide").unwrap();
            (*state).lock().unwrap().is_paused = true;
        }) as Box<dyn FnMut(Event)>)
    };
    document.set_onpointerlockchange(Some(handle_onpointerlockchange.as_ref().unchecked_ref()));
    handle_onpointerlockchange.forget();

    let handle_mouse_move =
        Closure::wrap(Box::new(handle_mouse_move) as Box<dyn FnMut(MouseEvent)>);
    canvas.set_onmousemove(Some(handle_mouse_move.as_ref().unchecked_ref()));
    handle_mouse_move.forget();

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
    let horizontal_u_location = gl.get_uniform_location(&program, "u_horizontal");
    let vertical_u_location = gl.get_uniform_location(&program, "u_vertical");
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

        let now = window.performance().unwrap().now();
        let dt = now - state.prev_now;
        update_position(&mut state, dt);

        // don't render while paused unless trying to save
        // OR unless it's the very first frame
        let should_render = (state.should_render && !state.is_paused)
            || (state.should_render && state.is_paused && state.should_save)
            || (state.should_render
                && state.is_paused
                && !state.should_save
                && state.render_count == 0);

        if should_render {
            update_render_globals(&mut state);
            update_moving_fps_array(now, &mut state, dt);

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
                horizontal_u_location.as_ref(),
                &state.horizontal.to_array(),
            );
            gl.uniform3fv_with_f32_array(vertical_u_location.as_ref(), &state.vertical.to_array());
            gl.uniform3fv_with_f32_array(
                lower_left_corner_u_location.as_ref(),
                &state.lower_left_corner.to_array(),
            );
            gl.uniform1i(render_count_u_location.as_ref(), state.render_count as i32);
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
