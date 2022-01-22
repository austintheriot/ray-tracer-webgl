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
use web_sys::WebGl2RenderingContext;

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
    let uniforms = webgl::setup_uniforms(&gl, &program);

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
    {
        let f = f.clone();
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

            if should_render {
                state::update_render_globals(&mut state);
                state::update_moving_fps_array(now, &mut state, dt);

                uniforms.run_setters(&state, &gl, now);

                webgl::render(&gl, &state, &textures, &framebuffer_objects);

                dom::save_image(&mut state);
                dom::update_fps_indicator(now, &mut state);
            }
            dom::request_animation_frame((*f).borrow().as_ref().unwrap());
        }) as Box<dyn FnMut()>));
    }

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
