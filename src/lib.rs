#![feature(format_args_capture)]
extern crate console_error_panic_hook;
#[macro_use]
extern crate lazy_static;

mod dom;
mod math;
mod state;
mod webgl;

use state::State;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlAnchorElement, WebGl2RenderingContext};

lazy_static! {
    static ref STATE: Arc<Mutex<State>> = Arc::new(Mutex::new(State::default()));
}

pub async fn async_main() -> Result<(), JsValue> {
    // GET ELEMENTS
    let canvas = dom::canvas();
    let gl = canvas
        .get_context("webgl2")?
        .unwrap()
        .dyn_into::<WebGl2RenderingContext>()?;

    let state = (*STATE).lock().unwrap();
    canvas.set_width(state.width);
    canvas.set_height(state.height);
    drop(state);

    dom::add_listeners()?;

    let program = webgl::setup_program(&gl).await?;

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
    let lens_radius_u_location = gl.get_uniform_location(&program, "u_lens_radius");
    let u_u_location = gl.get_uniform_location(&program, "u_u");
    let v_u_location = gl.get_uniform_location(&program, "u_v");
    let w_u_location = gl.get_uniform_location(&program, "u_w");

    webgl::setup_vertex_buffer(&gl, &program)?;
    let state = (*STATE).lock().unwrap();
    let textures = [
        webgl::create_texture(&gl, &state),
        webgl::create_texture(&gl, &state),
    ];
    let framebuffer_objects = [
        webgl::create_framebuffer(&gl, &textures[0]),
        webgl::create_framebuffer(&gl, &textures[1]),
    ];
    webgl::set_geometry(&state, &gl, &program);
    drop(state);

    // RENDER LOOP
    let f = Rc::new(RefCell::new(None));
    let g = f.clone();
    *g.borrow_mut() = Some(Closure::wrap(Box::new(move || {
        // it's ok to borrow this as mutable for the entire block,
        // since it is synchronous and no other function calls can
        // try to lock the mutex while it is in use
        let mut state = (*STATE).lock().unwrap();
        let now = dom::window().performance().unwrap().now();
        let dt = now - state.prev_now;

        state::update_position(&mut state, dt);

        // don't render while paused unless trying to save
        // OR unless it's the very first frame
        let should_render = (state.should_render && !state.is_paused)
            || (state.should_render && state.is_paused && state.should_save)
            || (state.should_render
                && state.is_paused
                && !state.should_save
                && state.render_count == 0);

        // debounce resize handler
        if state.should_update_to_match_window_size && now - state.last_resize_time > 500. {
            state.should_update_to_match_window_size = false;
            state::update_render_dimensions_to_match_window(
                &mut state, &gl, &textures, &canvas, now,
            );
        }

        // increase sample rate when paused (such as on first render and when resizing)
        // it's ok to do some heavy lifting here, since it's not being continually rendered at this output
        let samples_per_pixel = if state.is_paused {
            state.samples_per_pixel.max(25)
        } else {
            state.samples_per_pixel
        };

        if should_render {
            state::update_render_globals(&mut state);
            state::update_moving_fps_array(now, &mut state, dt);

            // SET UNIFORMS
            gl.uniform1i(texture_u_location.as_ref(), 0);
            gl.uniform1f(width_u_location.as_ref(), state.width as f32);
            gl.uniform1f(height_u_location.as_ref(), state.height as f32);
            gl.uniform1i(max_depth_u_location.as_ref(), state.max_depth as i32);
            gl.uniform1f(time_u_location.as_ref(), now as f32);
            gl.uniform1i(
                samples_per_pixel_u_location.as_ref(),
                samples_per_pixel as i32,
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
            gl.uniform1f(lens_radius_u_location.as_ref(), state.lens_radius as f32);
            gl.uniform3fv_with_f32_array(u_u_location.as_ref(), &state.u.to_array());
            gl.uniform3fv_with_f32_array(v_u_location.as_ref(), &state.v.to_array());
            gl.uniform3fv_with_f32_array(w_u_location.as_ref(), &state.w.to_array());

            // RENDER
            // use texture previously rendered to
            gl.bind_texture(
                WebGl2RenderingContext::TEXTURE_2D,
                Some(&textures[((state.even_odd_count + 1) % 2) as usize]),
            );

            // draw to canvas
            gl.bind_framebuffer(WebGl2RenderingContext::FRAMEBUFFER, None);
            webgl::draw(&gl, &state);

            // only need to draw to framebuffer when doing averages of previous frames
            if state.should_average {
                // RENDER (TO FRAMEBUFFER)
                gl.bind_framebuffer(
                    WebGl2RenderingContext::FRAMEBUFFER,
                    Some(&framebuffer_objects[(state.even_odd_count % 2) as usize]),
                );
                webgl::draw(&gl, &state);
            }

            // if user has requested to save, save immediately after rendering
            if state.should_save {
                state.should_save = false;
                let data_url = canvas
                    .to_data_url()
                    .unwrap()
                    .replace("image/png", "image/octet-stream");
                let a = dom::document()
                    .create_element("a")
                    .unwrap()
                    .dyn_into::<HtmlAnchorElement>()
                    .unwrap();

                a.set_href(&data_url);
                a.set_download("canvas.png");
                a.click();
            }

            dom::update_fps_indicator(now, &mut state);
        }
        dom::request_animation_frame((*f).borrow().as_ref().unwrap());
    }) as Box<dyn FnMut()>));

    dom::request_animation_frame((*g).borrow().as_ref().unwrap());

    Ok(())
}

/// Entry function cannot be async, so spawns a local Future for running the real main function
#[wasm_bindgen]
pub fn main() -> Result<(), JsValue> {
    // enables more helpful stack traces
    console_error_panic_hook::set_once();
    wasm_logger::init(wasm_logger::Config::default());

    spawn_local(async {
        async_main().await.unwrap();
    });

    Ok(())
}
