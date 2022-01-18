use crate::ray::Ray;

use super::vec3::{Point, Vec3};

pub struct Camera {
    aspect_ratio: f64,
    viewport_height: f64,
    viewport_width: f64,
    focal_length: f64,

    origin: Point,
    viewport_horizontal_vec: Vec3,
    viewport_vertical_vec: Vec3,
    lower_left_corner: Point,
}

impl Default for Camera {
    fn default() -> Self {
        let aspect_ratio = 16.0 / 9.0;
        let viewport_height = 2.0;
        let viewport_width = aspect_ratio * viewport_height;
        let focal_length = 1.0;
        let origin = Point(0., 0., 0.);
        let viewport_horizontal_vec = Vec3(viewport_width, 0., 0.);
        let viewport_vertical_vec = Vec3(0., viewport_height, 0.);
        let lower_left_corner = &origin
            - &viewport_horizontal_vec / 2.
            - &viewport_vertical_vec / 2.
            - Vec3(0., 0., focal_length);

        Camera {
            aspect_ratio,
            viewport_height,
            viewport_width,
            focal_length,
            origin,
            viewport_horizontal_vec,
            viewport_vertical_vec,
            lower_left_corner,
        }
    }
}

impl Camera {
    // produce ray from camera location in direction of viewport
    pub fn get_ray(&self, u: f64, v: f64) -> Ray {
        Ray {
            origin: self.origin.clone(),
            direction: &self.lower_left_corner
                + u * &self.viewport_horizontal_vec
                + v * &self.viewport_vertical_vec
                - &self.origin,
        }
    }
}
