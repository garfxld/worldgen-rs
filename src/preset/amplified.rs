#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct AmplifiedPreset;

impl AmplifiedPreset {
    pub const fn new() -> Self {
        Self
    }
}
