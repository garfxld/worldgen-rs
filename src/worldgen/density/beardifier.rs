pub struct Beardifier;

impl Beardifier {
    #[must_use]
    pub const fn empty() -> Self {
        Self
    }

    #[must_use]
    pub const fn compute(&self, _block_x: i32, _block_y: i32, _block_z: i32) -> f64 {
        0.0
    }
}
