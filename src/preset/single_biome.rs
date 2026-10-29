use shulkr::util::Key;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SingleBiomePreset {
    pub(crate) biome: Key,
}

impl SingleBiomePreset {
    pub const fn new(biome: Key) -> Self {
        Self { biome }
    }

    pub const fn biome(&self) -> &Key {
        &self.biome
    }
}

impl Default for SingleBiomePreset {
    fn default() -> Self {
        Self::new(Key::const_vanilla("plains"))
    }
}
