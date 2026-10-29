#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct DefaultPreset;

impl DefaultPreset {
    pub const fn new() -> Self {
        Self
    }
}
