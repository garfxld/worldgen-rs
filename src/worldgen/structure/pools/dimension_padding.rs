#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DimensionPadding {
    pub bottom: i32,
    pub top: i32,
}

impl DimensionPadding {
    pub const ZERO: Self = Self::of(0);

    pub const fn of(value: i32) -> Self {
        Self {
            bottom: value,
            top: value,
        }
    }

    pub const fn new(bottom: i32, top: i32) -> Self {
        Self { bottom, top }
    }
}

impl Default for DimensionPadding {
    fn default() -> Self {
        Self::ZERO
    }
}
