use js_sys::Math::sqrt;

use super::{
    hit::{Hit, HitResult, HitResultData},
    vec3::{Point, Vec3},
};

pub struct Sphere {
    pub center: Point,
    pub radius: f64,
}

impl Hit for Sphere {
    fn hit(&self, ray: &super::ray::Ray, t_min: f64, t_max: f64) -> HitResult {
        let oc = &ray.origin - &self.center;
        let a = ray.direction.length_squared();
        let half_b = Vec3::dot(&oc, &ray.direction);
        let c = oc.length_squared() - self.radius.powi(2);
        let discriminant = half_b.powi(2) - a * c;

        // no hit
        if discriminant < 0. {
            return HitResult::NoHit;
        }

        // there is a hit, but it may not be within the acceptable range:
        // find the nearest root that lies in the acceptable range.
        let sqrt_discriminant = sqrt(discriminant);
        let mut root = (-half_b - sqrt_discriminant) / a;

        // t is out of range, so count it as a no hit
        if root < t_min || t_max < root {
            root = (-half_b + sqrt_discriminant) / a;
            if root < t_min || t_max < root {
                return HitResult::NoHit;
            }
        }

        let hit_point = ray.at(root);
        let outward_normal = (&hit_point - &self.center) / self.radius;

        let hit_result_data = HitResultData::builder()
            .t(root)
            .hit_point(hit_point)
            .front_face_and_normal(ray, &outward_normal)
            .build();

        HitResult::Hit {
            data: hit_result_data,
        }
    }
}
