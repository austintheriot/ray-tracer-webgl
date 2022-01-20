use crate::math;
use std::f64::consts::PI;
use std::fmt::Formatter;
use std::ops::Add;
use std::ops::AddAssign;
use std::ops::Div;
use std::ops::DivAssign;
use std::ops::Mul;
use std::ops::MulAssign;
use std::ops::Neg;
use std::ops::Sub;
use std::ops::SubAssign;

/// Color and position values are stored as floats
/// Colors are actually *written* to memory as bytes (0->255)
#[derive(Clone, PartialEq)]
pub struct Vec3(pub f64, pub f64, pub f64);

impl Vec3 {
    pub fn new() -> Self {
        Vec3(0., 0., 0.)
    }

    pub fn splat(value: f64) -> Self {
        Vec3(value, value, value)
    }

    pub fn x(&self) -> f64 {
        self.0
    }
    pub fn y(&self) -> f64 {
        self.1
    }
    pub fn z(&self) -> f64 {
        self.2
    }

    pub fn r(&self) -> f64 {
        self.0
    }
    pub fn g(&self) -> f64 {
        self.1
    }
    pub fn b(&self) -> f64 {
        self.2
    }

    pub fn length(&self) -> f64 {
        self.length_squared().sqrt()
    }

    pub fn length_squared(&self) -> f64 {
        self.0.powi(2) + self.1.powi(2) + self.2.powi(2)
    }

    pub fn dot(a: &Vec3, b: &Vec3) -> f64 {
        a.0 * b.0 + a.1 * b.1 + a.2 * b.2
    }

    pub fn cross(a: &Vec3, b: &Vec3) -> Vec3 {
        Vec3(
            a.1 * b.2 - a.2 * b.1,
            a.2 * b.0 - a.0 * b.2,
            a.0 * b.1 - a.1 * b.0,
        )
    }

    pub fn normalize(self: Vec3) -> Vec3 {
        self.clone() / self.length()
    }

    /// from -1->1 to 0->1
    pub fn map_to_color_range(&self) -> Vec3 {
        (1. + self.clone()) * 0.5
    }

    pub fn random_with_range(min: f64, max: f64) -> Self {
        Vec3(
            math::random_with_range(min, max),
            math::random_with_range(min, max),
            math::random_with_range(min, max),
        )
    }

    /// INSIDE the unit sphere
    pub fn random_point_in_unit_sphere() -> Vec3 {
        let u = js_sys::Math::random();
        let v = js_sys::Math::random();
        let theta = u * 2.0 * PI;
        let phi = (2.0 * v - 1.0).acos();
        let r = (js_sys::Math::random()).cbrt();
        let sin_theta = theta.sin();
        let cos_theta = theta.cos();
        let sin_phi = phi.sin();
        let cos_phi = phi.cos();
        let x = r * sin_phi * cos_theta;
        let y = r * sin_phi * sin_theta;
        let z = r * cos_phi;
        Vec3(x, y, z)
    }

    /// along the EDGE of the unit sphere
    pub fn random_unit_vector() -> Vec3 {
        Vec3::normalize(Vec3::random_point_in_unit_sphere())
    }

    pub fn to_array(&self) -> [f32; 3] {
        [self.x() as f32, self.y() as f32, self.z() as f32]
    }

    pub fn near_zero(&self) -> bool {
        let threshold = 1e-10;
        self.x() < threshold && self.y() < threshold && self.z() < threshold
    }
}

impl Default for Vec3 {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Vec3 {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.debug_struct("Vec3")
            .field("0", &self.0)
            .field("1", &self.1)
            .field("2", &self.2)
            .finish()
    }
}

impl Neg for &Vec3 {
    type Output = Vec3;

    fn neg(self) -> Self::Output {
        Vec3(self.0 * -1., self.1 * -1., self.2 * -1.)
    }
}

impl Neg for Vec3 {
    type Output = Vec3;

    fn neg(self) -> Self::Output {
        Vec3(self.0 * -1., self.1 * -1., self.2 * -1.)
    }
}

impl AddAssign for Vec3 {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
        self.1 += rhs.1;
        self.2 += rhs.2;
    }
}

impl AddAssign<f64> for Vec3 {
    fn add_assign(&mut self, rhs: f64) {
        self.0 += rhs;
        self.1 += rhs;
        self.2 += rhs;
    }
}

impl SubAssign for Vec3 {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
        self.1 -= rhs.1;
        self.2 -= rhs.2;
    }
}

impl SubAssign<f64> for Vec3 {
    fn sub_assign(&mut self, rhs: f64) {
        self.0 -= rhs;
        self.1 -= rhs;
        self.2 -= rhs;
    }
}

impl MulAssign<f64> for Vec3 {
    fn mul_assign(&mut self, rhs: f64) {
        self.0 *= rhs;
        self.1 *= rhs;
        self.2 *= rhs;
    }
}

impl DivAssign<f64> for Vec3 {
    fn div_assign(&mut self, rhs: f64) {
        self.0 /= rhs;
        self.1 /= rhs;
        self.2 /= rhs;
    }
}

impl Add for &Vec3 {
    type Output = Vec3;

    fn add(self, rhs: Self) -> Self::Output {
        Vec3(self.0 + rhs.0, self.1 + rhs.1, self.2 + rhs.2)
    }
}

impl Add for Vec3 {
    type Output = Vec3;

    fn add(self, rhs: Self) -> Self::Output {
        Vec3(self.0 + rhs.0, self.1 + rhs.1, self.2 + rhs.2)
    }
}

impl Add<Vec3> for &Vec3 {
    type Output = Vec3;

    fn add(self, rhs: Vec3) -> Self::Output {
        Vec3(self.0 + rhs.0, self.1 + rhs.1, self.2 + rhs.2)
    }
}

impl Add<f64> for &Vec3 {
    type Output = Vec3;

    fn add(self, rhs: f64) -> Self::Output {
        Vec3(self.0 + rhs, self.1 + rhs, self.2 + rhs)
    }
}

impl Add<Vec3> for f64 {
    type Output = Vec3;

    fn add(self, rhs: Vec3) -> Self::Output {
        Vec3(self + rhs.0, self + rhs.1, self + rhs.2)
    }
}

impl Add<&Vec3> for f64 {
    type Output = Vec3;

    fn add(self, rhs: &Vec3) -> Self::Output {
        Vec3(self + rhs.0, self + rhs.1, self + rhs.2)
    }
}

impl Sub for &Vec3 {
    type Output = Vec3;

    fn sub(self, rhs: Self) -> Self::Output {
        Vec3(self.0 - rhs.0, self.1 - rhs.1, self.2 - rhs.2)
    }
}

impl Sub for Vec3 {
    type Output = Vec3;

    fn sub(self, rhs: Self) -> Self::Output {
        Vec3(self.0 - rhs.0, self.1 - rhs.1, self.2 - rhs.2)
    }
}

impl Sub<&Vec3> for Vec3 {
    type Output = Vec3;

    fn sub(self, rhs: &Self) -> Self::Output {
        Vec3(self.0 - rhs.0, self.1 - rhs.1, self.2 - rhs.2)
    }
}

impl Sub<Vec3> for &Vec3 {
    type Output = Vec3;

    fn sub(self, rhs: Vec3) -> Self::Output {
        Vec3(self.0 - rhs.0, self.1 - rhs.1, self.2 - rhs.2)
    }
}

impl Sub<f64> for &Vec3 {
    type Output = Vec3;

    fn sub(self, rhs: f64) -> Self::Output {
        Vec3(self.0 - rhs, self.1 - rhs, self.2 - rhs)
    }
}

impl Sub<f64> for Vec3 {
    type Output = Vec3;

    fn sub(self, rhs: f64) -> Self::Output {
        Vec3(self.0 - rhs, self.1 - rhs, self.2 - rhs)
    }
}

impl Mul for &Vec3 {
    type Output = Vec3;

    fn mul(self, rhs: Self) -> Self::Output {
        Vec3(self.0 * rhs.0, self.1 * rhs.1, self.2 * rhs.2)
    }
}

impl Mul for Vec3 {
    type Output = Vec3;

    fn mul(self, rhs: Self) -> Self::Output {
        Vec3(self.0 * rhs.0, self.1 * rhs.1, self.2 * rhs.2)
    }
}

impl Mul<f64> for &Vec3 {
    type Output = Vec3;

    fn mul(self, rhs: f64) -> Self::Output {
        Vec3(self.0 * rhs, self.1 * rhs, self.2 * rhs)
    }
}

impl Mul<f64> for Vec3 {
    type Output = Vec3;

    fn mul(self, rhs: f64) -> Self::Output {
        Vec3(self.0 * rhs, self.1 * rhs, self.2 * rhs)
    }
}

impl Mul<&Vec3> for f64 {
    type Output = Vec3;

    fn mul(self, rhs: &Vec3) -> Self::Output {
        Vec3(self * rhs.0, self * rhs.1, self * rhs.2)
    }
}

impl Mul<Vec3> for f64 {
    type Output = Vec3;

    fn mul(self, rhs: Vec3) -> Self::Output {
        Vec3(self * rhs.0, self * rhs.1, self * rhs.2)
    }
}

impl Div for &Vec3 {
    type Output = Vec3;

    fn div(self, rhs: Self) -> Self::Output {
        Vec3(self.0 / rhs.0, self.1 / rhs.1, self.2 / rhs.2)
    }
}

impl Div for Vec3 {
    type Output = Vec3;

    fn div(self, rhs: Self) -> Self::Output {
        Vec3(self.0 / rhs.0, self.1 / rhs.1, self.2 / rhs.2)
    }
}

impl Div<f64> for &Vec3 {
    type Output = Vec3;

    fn div(self, rhs: f64) -> Self::Output {
        Vec3(self.0 / rhs, self.1 / rhs, self.2 / rhs)
    }
}

impl Div<f64> for Vec3 {
    type Output = Vec3;

    fn div(self, rhs: f64) -> Self::Output {
        Vec3(self.0 / rhs, self.1 / rhs, self.2 / rhs)
    }
}

pub use Vec3 as Color;
pub use Vec3 as Point;
