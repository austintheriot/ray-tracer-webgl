use super::{
    ray::Ray,
    vec3::{Point, Vec3},
};

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
