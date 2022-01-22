use std::sync::MutexGuard;

use crate::{dom, state::State, STATE};
use wasm_bindgen::{prelude::Closure, JsCast, JsValue};
use web_sys::{
    Element, Event, HtmlAnchorElement, HtmlButtonElement, HtmlDivElement, KeyboardEvent,
    MouseEvent, WheelEvent,
};

pub const MAX_CANVAS_SIZE: u32 = 1280;

pub fn window() -> web_sys::Window {
    web_sys::window().expect("no global `window` exists")
}

pub fn document() -> web_sys::Document {
    window()
        .document()
        .expect("should have a document on window")
}

pub fn canvas() -> web_sys::HtmlCanvasElement {
    document()
        .query_selector("canvas")
        .unwrap()
        .unwrap()
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .unwrap()
}

pub fn handle_wheel(e: WheelEvent) {
    // can take a mutex guard here, because it will never be called while render loop is running
    let mut state = (*STATE).lock().unwrap();
    let adjustment = 1. * e.delta_y().signum();
    let new_value = state.camera_field_of_view * (1. + adjustment * 0.01);
    state.set_fov(new_value);
}

pub fn handle_reset() {
    // can take a mutex guard here, because it will never be called while render loop is running
    let mut state = (*STATE).lock().unwrap();
    *state = State::default();
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
}

pub fn handle_resize() {
    // can take a mutex guard here, because it will never be called while render loop is running
    let mut state = (*STATE).lock().unwrap();
    state.should_update_to_match_window_size = true;
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
}

pub fn handle_mouse_move(e: MouseEvent) {
    let mut state = (*STATE).lock().unwrap();
    // camera should move slower when more "zoomed in"
    let dx = (e.movement_x() as f64) * state.look_sensitivity * state.camera_field_of_view;
    let dy = -(e.movement_y() as f64) * state.look_sensitivity * state.camera_field_of_view;
    let yaw = state.yaw + dx;
    let pitch = state.pitch + dy;
    state.set_camera_angles(yaw, pitch);
}

/// Waits until immediately after rendering on the next frame to save the image
/// so that the canvas isn't blank
pub fn handle_save_image(_: MouseEvent) {
    // can take a mutex guard here, because it will never be called while render loop is running
    let mut state = (*STATE).lock().unwrap();
    state.should_render = true;
    state.should_save = true;
}

/// if user has requested to save, save immediately after rendering
pub fn save_image(state: &mut MutexGuard<State>) {
    if state.should_save {
        state.should_save = false;
        let data_url = canvas()
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
}

pub fn update_fps_indicator(now: f64, state: &mut MutexGuard<State>) {
    let fps_indicator = dom::document()
        .query_selector("#fps")
        .unwrap()
        .unwrap()
        .dyn_into::<web_sys::HtmlParagraphElement>()
        .unwrap();

    if now - state.prev_fps_update_time > 250. {
        state.prev_fps_update_time = now;
        let average_fps: f64 = state.prev_fps.iter().sum::<f64>() / (state.prev_fps.len() as f64);
        fps_indicator.set_text_content(Some(&format!("{:.2} fps", average_fps)))
    }
}

pub fn add_listeners() -> Result<(), JsValue> {
    // GET ELEMENTS
    let window = dom::window();
    let document = dom::document();
    let canvas = document
        .query_selector("canvas")?
        .unwrap()
        .dyn_into::<web_sys::HtmlCanvasElement>()?;

    let enable_button = document
        .query_selector("#enable")?
        .unwrap()
        .dyn_into::<HtmlButtonElement>()?;

    let save_image_button = document
        .query_selector("#save-image")?
        .unwrap()
        .dyn_into::<HtmlButtonElement>()?;

    let reset_button = document
        .query_selector("#reset")?
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
    let handle_wheel = Closure::wrap(Box::new(dom::handle_wheel) as Box<dyn FnMut(WheelEvent)>);
    window.set_onwheel(Some(handle_wheel.as_ref().unchecked_ref()));
    handle_wheel.forget();

    let handle_resize = Closure::wrap(Box::new(dom::handle_resize) as Box<dyn FnMut()>);
    window.set_onresize(Some(handle_resize.as_ref().unchecked_ref()));
    handle_resize.forget();

    let handle_reset = Closure::wrap(Box::new(dom::handle_reset) as Box<dyn FnMut()>);
    reset_button.set_onclick(Some(handle_reset.as_ref().unchecked_ref()));
    handle_reset.forget();

    let handle_save_image =
        Closure::wrap(Box::new(dom::handle_save_image) as Box<dyn FnMut(MouseEvent)>);
    save_image_button.set_onclick(Some(handle_save_image.as_ref().unchecked_ref()));
    handle_save_image.forget();

    let handle_keydown =
        Closure::wrap(Box::new(dom::handle_keydown) as Box<dyn FnMut(KeyboardEvent)>);
    window.set_onkeydown(Some(handle_keydown.as_ref().unchecked_ref()));
    handle_keydown.forget();

    let handle_keyup = Closure::wrap(Box::new(dom::handle_keyup) as Box<dyn FnMut(KeyboardEvent)>);
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
        Closure::wrap(Box::new(dom::handle_mouse_move) as Box<dyn FnMut(MouseEvent)>);
    canvas.set_onmousemove(Some(handle_mouse_move.as_ref().unchecked_ref()));
    handle_mouse_move.forget();

    Ok(())
}

// limit max canvas dimensions to a reasonable number
// (to prevent off-the-charts GPU work on large screen sizes)
pub fn get_adjusted_screen_dimensions() -> (u32, u32) {
    let raw_screen_width = dom::window().inner_width().unwrap().as_f64().unwrap();
    let raw_screen_height = dom::window().inner_height().unwrap().as_f64().unwrap();
    let aspect_ratio = raw_screen_width / raw_screen_height;

    return if raw_screen_width > raw_screen_height {
        let adjusted_width = raw_screen_width.min(MAX_CANVAS_SIZE as f64);
        let adjusted_height = adjusted_width / aspect_ratio;
        (adjusted_width as u32, adjusted_height as u32)
    } else {
        let adjusted_height = raw_screen_width.min(MAX_CANVAS_SIZE as f64);
        let adjusted_width = adjusted_height * aspect_ratio;
        (adjusted_width as u32, adjusted_height as u32)
    };
}

pub fn request_animation_frame(f: &Closure<dyn FnMut()>) {
    dom::window()
        .request_animation_frame(f.as_ref().unchecked_ref())
        .expect("should register `requestAnimationFrame` OK");
}
