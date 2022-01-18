#![feature(format_args_capture)]
extern crate console_error_panic_hook;
#[macro_use]
extern crate lazy_static;

mod camera;
mod hit;
mod math;
mod ray;
mod sphere;
mod vec3;

use camera::Camera;
use hit::{Hit, HitResult, HittableList};
use log::info;
use ray::Ray;
use sphere::Sphere;
use vec3::{Color, Vec3};
use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

// image
pub const WIDTH: u32 = 320;
pub const HEIGHT: u32 = 180;
pub const BYTES_PER_PIXEL: u32 = 4;
pub const PIXEL_ARRAY_LENGTH: usize = (WIDTH * HEIGHT * BYTES_PER_PIXEL) as usize;
pub const ASPECT_RATIO: f64 = (WIDTH as f64) / (HEIGHT as f64);
pub const SAMPLES_PER_PIXEL: u32 = 100;
pub const MAX_DEPTH: u32 = 50;
static mut PIXELS: [u8; PIXEL_ARRAY_LENGTH] = [0; PIXEL_ARRAY_LENGTH];

lazy_static! {
    static ref CAMERA: Camera = Camera::default();
}

// world
lazy_static! {
    static ref WORLD: HittableList = hittable_list![
        Sphere {
            center: Vec3(0., 0., -1.),
            radius: 0.5,
        },
        Sphere {
            center: Vec3(0., -10000.5, -1.),
            radius: 10000.,
        }
    ];
}

pub fn write_color(accumulated_color: &Color, pixels: &mut [u8], index: usize) {
    let mut r = accumulated_color.r();
    let mut g = accumulated_color.g();
    let mut b = accumulated_color.b();

    // get average pixel color
    let scale = 1.0 / (SAMPLES_PER_PIXEL as f64);
    r *= scale;
    g *= scale;
    b *= scale;

    // gama correction (gamma = 2.0)
    r = r.sqrt();
    g = g.sqrt();
    b = b.sqrt();

    // map 0->1 to 0-> 255
    let r = (256. * r.clamp(0., 0.999)) as u8;
    let g = (256. * g.clamp(0., 0.999)) as u8;
    let b = (256. * b.clamp(0., 0.999)) as u8;

    pixels[index * BYTES_PER_PIXEL as usize] = r;
    pixels[index * BYTES_PER_PIXEL as usize + 1] = g;
    pixels[index * BYTES_PER_PIXEL as usize + 2] = b;
    pixels[index * BYTES_PER_PIXEL as usize + 3] = 255;
}

/// returns what color a ray should be based on what objects, if any, it intersects with
fn ray_color(ray: &Ray, depth: u32) -> Color {
    // max bounce recursions reached
    if depth >= MAX_DEPTH {
        return Color(0., 0., 0.);
    }

    if let HitResult::Hit { data, .. } = WORLD.hit(ray, 0.001, f64::INFINITY) {
        // generate a random point along a unit sphere from the tip of the normal, diffusing the light
        let new_target = &data.hit_point + &data.normal + Vec3::random_unit_vector();
        return 0.5
            * ray_color(
                &Ray {
                    origin: data.hit_point.clone(),
                    direction: new_target - data.hit_point,
                },
                depth + 1,
            );
    }

    // interpolate between white and blue based on the y value of the direction
    let unit_direction = Vec3::normalize(&ray.direction);
    let t = 0.5 * (unit_direction.y() + 1.);
    (1.0 - t) * Color(1.0, 1.0, 1.0) + t * Color(0.5, 0.7, 1.0)
}

/// iterate through each pixel in the image and determine its color
/// by shooting ray(s) in the direction of the pixel and finding what objects,
/// if any, they intersect with
fn update_pixels(pixels: &mut [u8], camera: &Camera) {
    for y in 0..HEIGHT {
        info!("Rows remaining = {}", HEIGHT - y);
        for x in 0..WIDTH {
            let mut accumulated_color = Color::splat(0.);
            for _ in 0..SAMPLES_PER_PIXEL {
                // u = percentage along image's width
                // v = percentage along image's height
                let u = (x as f64 + js_sys::Math::random()) / (WIDTH - 1) as f64;
                let v = (y as f64 + js_sys::Math::random()) / (HEIGHT - 1) as f64;
                let ray = camera.get_ray(u, v);
                accumulated_color += ray_color(&ray, 0);
            }
            // because canvas uses low y as being the TOP rather than the bottom
            let y_inverted = HEIGHT - y - 1;
            let index = ((y_inverted * WIDTH) + x) as usize;
            write_color(&accumulated_color, pixels, index);
        }
    }
}

// This is the entry point for the web app
#[wasm_bindgen]
pub fn ray_trace() -> Result<(), JsValue> {
    // enables ore helpful stack traces
    console_error_panic_hook::set_once();
    wasm_logger::init(wasm_logger::Config::default());

    // cast rays
    update_pixels(unsafe { &mut PIXELS }, &CAMERA);

    Ok(())
}

/// enables retrieving canvas data from JS
#[wasm_bindgen]
pub struct CanvasPixelData {
    pub pixels_ptr: *const u8,
    pub pixels_len: u32,
    pub canvas_width: u32,
    pub canvas_height: u32,
}

#[wasm_bindgen]
pub fn get_canvas_data() -> CanvasPixelData {
    CanvasPixelData {
        pixels_ptr: (unsafe { &PIXELS } as *const u8),
        pixels_len: (unsafe { PIXELS }.len() as u32),
        canvas_width: WIDTH,
        canvas_height: HEIGHT,
    }
}
