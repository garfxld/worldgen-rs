#[derive(Copy, Clone, Debug, Eq)]
#[derive_const(PartialEq)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    #[must_use]
    pub const fn is_horizontal(self) -> bool {
        matches!(self, Axis::X | Axis::Z)
    }

    #[must_use]
    pub const fn is_vertical(self) -> bool {
        matches!(self, Axis::Y)
    }

    #[must_use]
    pub const fn as_str(&self) -> &str {
        match self {
            Axis::X => "x",
            Axis::Y => "y",
            Axis::Z => "z",
        }
    }
}
