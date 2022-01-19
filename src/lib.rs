#![feature(format_args_capture)]
extern crate console_error_panic_hook;

mod math;
mod vec3;

use std::cell::RefCell;
use std::rc::Rc;
use vec3::{Point, Vec3};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    HtmlScriptElement, WebGl2RenderingContext, WebGlFramebuffer, WebGlProgram, WebGlShader,
    WebGlTexture,
};

pub const WIDTH: u32 = 1600;
pub const HEIGHT: u32 = 900;
pub const BYTES_PER_PIXEL: u32 = 4;
pub const PIXEL_ARRAY_LENGTH: usize = (WIDTH * HEIGHT * BYTES_PER_PIXEL) as usize;
pub const ASPECT_RATIO: f64 = (WIDTH as f64) / (HEIGHT as f64);
pub const SAMPLES_PER_PIXEL: u32 = 1;
pub const MAX_DEPTH: u32 = 5;
pub const VIEWPORT_HEIGHT: f64 = 2.0;
pub const VIEWPORT_WIDTH: f64 = ASPECT_RATIO * VIEWPORT_HEIGHT;
pub const FOCAL_LENGTH: f64 = 1.0;
pub const CAMERA_ORIGIN: Point = Point(0., 0., 0.);
pub const VIEWPORT_HORIZONTAL_VEC: Vec3 = Vec3(VIEWPORT_WIDTH, 0., 0.);
pub const VIEWPORT_VERTICAL_VEC: Vec3 = Vec3(0., VIEWPORT_HEIGHT, 0.);
pub const LOWER_LEFT_CORNER: Point = Vec3(
    CAMERA_ORIGIN.0 - VIEWPORT_HORIZONTAL_VEC.0 / 2.,
    CAMERA_ORIGIN.1 - VIEWPORT_VERTICAL_VEC.1 / 2.,
    CAMERA_ORIGIN.2 - FOCAL_LENGTH,
);
pub const SIMPLE_QUAD_VERTICES: [f32; 12] = [
    -1.0, 1.0, 1.0, 1.0, -1.0, -1.0, -1.0, -1.0, 1.0, 1.0, 1.0, -1.0,
];
static mut SHOULD_RENDER: bool = true;
static mut EVEN_ODD_COUNT: u32 = 0;
static mut RENDER_COUNT: f32 = 0.;

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
        WebGl2RenderingContext::NEAREST as i32,
    );
    gl.tex_parameteri(
        WebGl2RenderingContext::TEXTURE_2D,
        WebGl2RenderingContext::TEXTURE_MAG_FILTER,
        WebGl2RenderingContext::NEAREST as i32,
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
            unsafe {
                // set to false to only render on updates
                SHOULD_RENDER = true;
                EVEN_ODD_COUNT += 1;
                RENDER_COUNT += 1.;
            }

            // SET UNIFORMS
            // tell the fragment shader to get the u_image texture from texture unit 0 (default)
            gl.uniform1i(texture_u_location.as_ref(), 0);
            gl.uniform1f(width_u_location.as_ref(), WIDTH as f32);
            gl.uniform1f(height_u_location.as_ref(), HEIGHT as f32);
            gl.uniform1i(max_depth_u_location.as_ref(), MAX_DEPTH as i32);
            gl.uniform1f(
                time_u_location.as_ref(),
                window.performance().unwrap().now() as f32,
            );
            gl.uniform1i(
                samples_per_pixel_u_location.as_ref(),
                SAMPLES_PER_PIXEL as i32,
            );
            // float aspect_ratio = u_width / u_height;
            gl.uniform1f(aspect_ratio_u_location.as_ref(), ASPECT_RATIO as f32);
            // float viewport_height = 2.0;
            gl.uniform1f(viewport_height_u_location.as_ref(), VIEWPORT_HEIGHT as f32);
            // float focal_length = 1.0;
            gl.uniform1f(focal_length_u_location.as_ref(), FOCAL_LENGTH as f32);
            // vec3 camera_origin = vec3(0.);
            gl.uniform3fv_with_f32_array(
                camera_origin_u_location.as_ref(),
                &CAMERA_ORIGIN.to_array(),
            );
            // float viewport_width = aspect_ratio * viewport_height;
            gl.uniform1f(viewport_width_u_location.as_ref(), VIEWPORT_WIDTH as f32);
            // vec3 viewport_horizontal_vec = vec3(viewport_width, 0., 0.);
            gl.uniform3fv_with_f32_array(
                viewport_horizontal_vec_u_location.as_ref(),
                &VIEWPORT_HORIZONTAL_VEC.to_array(),
            );
            // vec3 viewport_vertical_vec = vec3(0., viewport_height, 0.);
            gl.uniform3fv_with_f32_array(
                viewport_vertical_vec_u_location.as_ref(),
                &VIEWPORT_VERTICAL_VEC.to_array(),
            );
            // vec3 lower_left_corner = camera_origin - viewport_horizontal_vec / 2. - viewport_vertical_vec / 2. - vec3(0., 0., focal_length);
            gl.uniform3fv_with_f32_array(
                lower_left_corner_u_location.as_ref(),
                &LOWER_LEFT_CORNER.to_array(),
            );
            gl.uniform1f(render_count_u_location.as_ref(), unsafe { RENDER_COUNT });

            // use texture previously rendered to
            gl.bind_texture(
                WebGl2RenderingContext::TEXTURE_2D,
                Some(&textures[(unsafe { EVEN_ODD_COUNT + 1 } % 2) as usize]),
            );

            // RENDER (TO CANVAS)
            gl.bind_framebuffer(WebGl2RenderingContext::FRAMEBUFFER, None);
            draw(&gl);

            // RENDER (TO FRAMEBUFFER)
            gl.bind_framebuffer(
                WebGl2RenderingContext::FRAMEBUFFER,
                Some(&framebuffer_objects[(unsafe { EVEN_ODD_COUNT } % 2) as usize]),
            );
            draw(&gl);
        }
        request_animation_frame(f.borrow().as_ref().unwrap());
    }) as Box<dyn FnMut()>));

    request_animation_frame(g.borrow().as_ref().unwrap());

    Ok(())
}
