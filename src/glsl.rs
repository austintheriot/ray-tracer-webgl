//! This crate is a mirror of much of the GLSL code already written
//! and is intended to interop well with the GPU side of things.

use super::math::{Point, Vec3};
use crate::{ray::Ray, state::State};
use js_sys::Math::sqrt;
use std::sync::MutexGuard;

#[derive(Clone, PartialEq, Debug)]
pub enum MaterialType {
    Diffuse,
    Metal,
    Glass,
}

impl MaterialType {
    pub fn value(&self) -> i32 {
        match self {
            MaterialType::Diffuse => 0,
            MaterialType::Metal => 1,
            MaterialType::Glass => 2,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Material {
    pub material_type: MaterialType,
    pub albedo: Vec3,          // or "reflectance"
    pub fuzz: f32,             // used for duller metals
    pub refraction_index: f32, // used for glass
}

#[derive(Clone, PartialEq, Debug)]
pub struct Sphere {
    pub center: Vec3,
    pub radius: f64,
    pub material: Material,
    pub uuid: i32,
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
            .uuid(self.uuid.clone())
            .build();

        HitResult::Hit {
            data: hit_result_data,
        }
    }
}

pub fn set_sphere_uuids(spheres: &mut Vec<Sphere>) {
    for (i, sphere) in spheres.iter_mut().enumerate() {
        sphere.uuid = i as i32;
    }
}

#[derive(Debug)]
pub enum HitResult {
    Hit { data: HitResultData },
    NoHit,
}

#[derive(Debug, Default, Clone)]
pub struct HitResultData {
    pub hit_point: Point,
    pub normal: Vec3,
    pub t: f64,
    pub front_face: bool,
    pub uuid: i32,
}

impl HitResultData {
    pub fn builder() -> HitResultDataBuilder {
        HitResultDataBuilder::new()
    }
}

#[derive(Debug, Default)]
pub struct HitResultDataBuilder {
    hit_point: Point,
    t: f64,
    normal: Vec3,
    front_face: bool,
    uuid: i32,
}

impl HitResultDataBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn hit_point(mut self, hit_point: Point) -> Self {
        self.hit_point = hit_point;
        self
    }

    pub fn t(mut self, t: f64) -> Self {
        self.t = t;
        self
    }

    pub fn uuid(mut self, uuid: i32) -> Self {
        self.uuid = uuid;
        self
    }

    pub fn front_face_and_normal(mut self, r: &Ray, outward_normal: &Vec3) -> Self {
        self.front_face = Vec3::dot(&r.direction, outward_normal) < 0.;
        self.normal = if self.front_face {
            outward_normal.clone()
        } else {
            -outward_normal.clone()
        };
        self
    }

    pub fn build(self) -> HitResultData {
        HitResultData {
            hit_point: self.hit_point,
            normal: self.normal,
            t: self.t,
            front_face: self.front_face,
            uuid: self.uuid,
        }
    }
}

/// Any object can test whether the ray has hit it
/// t_min and t_max represent the range along a ray
/// where we count a hit "valid"
pub trait Hit {
    fn hit(&self, ray: &Ray, t_min: f64, t_max: f64) -> HitResult;
}

pub struct HittableList {
    pub list: Vec<Box<dyn Hit>>,
}

unsafe impl Send for HittableList {}
unsafe impl Sync for HittableList {}

/// creates a list of hittable objects without having to write `Box::new()`
/// around each item that is included in the list.
#[macro_export]
macro_rules! hittable_list {
  ($($hittable: expr),*) => {{
       let mut list: Vec<Box<dyn Hit>> = Vec::new();
       $( list.push(Box::new($hittable)); )*
       HittableList { list }
  }}
}

impl Hit for HittableList {
    fn hit(&self, ray: &Ray, t_min: f64, t_max: f64) -> HitResult {
        let mut prev_hit_result = HitResult::NoHit;

        for hittable in &self.list {
            let new_hit_result = hittable.hit(ray, t_min, t_max);

            // this object was a hit
            if let HitResult::Hit { data: new_hit_data } = &new_hit_result {
                // replace saved hit result if previous was no-hit or was behind this new one
                match &prev_hit_result {
                    HitResult::NoHit => prev_hit_result = new_hit_result,
                    HitResult::Hit {
                        data: prev_hit_data,
                    } => {
                        if &new_hit_data.hit_point.z() > &prev_hit_data.hit_point.z() {
                            prev_hit_result = new_hit_result
                        }
                    }
                }
            }
        }

        prev_hit_result
    }
}

pub fn get_center_hit(state: &MutexGuard<State>) -> HitResult {
    let spheres = &state.sphere_list;

    let ray = Ray {
        origin: state.camera_origin.clone(),
        direction: &state.lower_left_corner + &state.horizontal / 2. + &state.vertical / 2.
            - &state.camera_origin,
    };

    let mut prev_hit_result = HitResult::NoHit;
    let mut closest_so_far = f64::INFINITY;

    for sphere in spheres {
        let new_hit_result = sphere.hit(&ray, 0., closest_so_far);

        // this object was a hit (and implicitly was in front of the last)
        if let HitResult::Hit {
            data: ref new_hit_data,
        } = new_hit_result
        {
            closest_so_far = new_hit_data.t;
            prev_hit_result = new_hit_result;
        }
    }

    prev_hit_result
}
