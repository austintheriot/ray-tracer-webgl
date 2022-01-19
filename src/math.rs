pub fn random_with_range(min: f64, max: f64) -> f64 {
    min + (max - min) * js_sys::Math::random()
}
