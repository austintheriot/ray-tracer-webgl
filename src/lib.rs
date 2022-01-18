#![feature(format_args_capture)]
extern crate console_error_panic_hook;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{HtmlScriptElement, WebGl2RenderingContext, WebGlProgram, WebGlShader};

pub const WIDTH: u32 = 1600;
pub const HEIGHT: u32 = 900;
pub const BYTES_PER_PIXEL: u32 = 4;
pub const ASPECT_RATIO: f64 = (WIDTH as f64) / (HEIGHT as f64);
pub const SAMPLES_PER_PIXEL: u32 = 100;
pub const MAX_DEPTH: u32 = 50;

pub const VERTICES: [f32; 12] = [
    -1.0, 1.0, 1.0, 1.0, -1.0, -1.0, -1.0, -1.0, 1.0, 1.0, 1.0, -1.0,
];

pub fn compile_shader(
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

pub fn link_program(
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

/// Entry function cannot be async, so spawns a local Future for running the real main function
#[wasm_bindgen]
pub fn main() -> Result<(), JsValue> {
    // enables ore helpful stack traces
    console_error_panic_hook::set_once();
    wasm_logger::init(wasm_logger::Config::default());

    // GET ELEMENTS
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
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

    // SETUP VAO
    let vao = gl.create_vertex_array().ok_or("Couldn't create vao")?;
    gl.bind_vertex_array(Some(&vao));

    // GET LOCATIONS
    let vertex_attribute_position = gl.get_attrib_location(&program, "a_position") as u32;
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

    // SET VERTEX BUFFER
    let buffer = gl.create_buffer().ok_or("failed to create buffer")?;
    gl.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&buffer));
    let vertex_array = unsafe { js_sys::Float32Array::view(&VERTICES) };
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

    // SET UNIFORMS
    gl.uniform1f(width_u_location.as_ref(), WIDTH as f32);
    gl.uniform1f(height_u_location.as_ref(), HEIGHT as f32);
    gl.uniform1f(
        time_u_location.as_ref(),
        window.performance().unwrap().now() as f32,
    );
    // int samples_per_pixel = 100;
    gl.uniform1f(
        samples_per_pixel_u_location.as_ref(),
        SAMPLES_PER_PIXEL as f32,
    );
    // float aspect_ratio = u_width / u_height;
    // float viewport_height = 2.0;
    // float focal_length = 1.0;
    // vec3 camera_origin = vec3(0.);
    // float viewport_width = aspect_ratio * viewport_height;
    // vec3 viewport_horizontal_vec = vec3(viewport_width, 0., 0.);
    // vec3 viewport_vertical_vec = vec3(0., viewport_height, 0.);
    // vec3 lower_left_corner = camera_origin - viewport_horizontal_vec / 2. - viewport_vertical_vec / 2. - vec3(0., 0., focal_length);

    // RENDER
    gl.clear_color(0.0, 0.0, 0.0, 1.0);
    gl.viewport(0, 0, WIDTH as i32, HEIGHT as i32);
    gl.clear(WebGl2RenderingContext::COLOR_BUFFER_BIT);
    gl.draw_arrays(
        WebGl2RenderingContext::TRIANGLES,
        0,
        (VERTICES.len() / 2) as i32,
    );

    Ok(())
}
