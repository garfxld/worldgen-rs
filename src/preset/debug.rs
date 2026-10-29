#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct DebugPreset;

impl DebugPreset {
    pub const fn new() -> Self {
        Self
    }
}
