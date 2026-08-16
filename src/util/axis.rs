#[derive(Copy, Clone, Debug, Eq, serde::Deserialize)]
#[derive_const(PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    pub const fn is_horizontal(self) -> bool {
        matches!(self, Axis::X | Axis::Z)
    }

    pub const fn is_vertical(self) -> bool {
        matches!(self, Axis::Y)
    }

    pub const fn as_str(&self) -> &str {
        match self {
            Axis::X => "x",
            Axis::Y => "y",
            Axis::Z => "z",
        }
    }
}
