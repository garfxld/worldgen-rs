#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct LargeBiomesPreset;

impl LargeBiomesPreset {
    pub const fn new() -> Self {
        Self
    }
}
