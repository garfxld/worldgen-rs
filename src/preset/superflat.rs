use shulkr::util::Key;
use shulkr::world::block::BlockState;

#[derive(Debug, Clone, PartialEq)]
pub struct SuperflatPreset {
    pub(crate) source: FlatSource,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FlatSource {
    Default,
    Custom(FlatSettings),
    Preset(Key),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlatSettings {
    pub layers: Vec<(BlockState, u32)>,
    pub biome: Key,
    pub features: bool,
    pub lakes: bool,
    pub structure_overrides: Option<Vec<Key>>,
}

impl SuperflatPreset {
    pub const fn new() -> Self {
        Self {
            source: FlatSource::Default,
        }
    }

    pub fn builder() -> SuperflatPresetBuilder {
        SuperflatPresetBuilder::default()
    }

    pub const fn from_preset(key: Key) -> Self {
        Self {
            source: FlatSource::Preset(key),
        }
    }

    pub const fn classic_flat() -> Self {
        Self::from_preset(Key::const_vanilla("classic_flat"))
    }

    pub const fn tunnelers_dream() -> Self {
        Self::from_preset(Key::const_vanilla("tunnelers_dream"))
    }

    pub const fn water_world() -> Self {
        Self::from_preset(Key::const_vanilla("water_world"))
    }

    pub const fn overworld() -> Self {
        Self::from_preset(Key::const_vanilla("overworld"))
    }

    pub const fn snowy_kingdom() -> Self {
        Self::from_preset(Key::const_vanilla("snowy_kingdom"))
    }

    pub const fn bottomless_pit() -> Self {
        Self::from_preset(Key::const_vanilla("bottomless_pit"))
    }

    pub const fn desert() -> Self {
        Self::from_preset(Key::const_vanilla("desert"))
    }

    pub const fn redstone_ready() -> Self {
        Self::from_preset(Key::const_vanilla("redstone_ready"))
    }

    pub const fn the_void() -> Self {
        Self::from_preset(Key::const_vanilla("the_void"))
    }
}

impl Default for SuperflatPreset {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct SuperflatPresetBuilder {
    settings: FlatSettings,
}

impl Default for SuperflatPresetBuilder {
    fn default() -> Self {
        Self {
            settings: FlatSettings {
                layers: Vec::new(),
                biome: Key::const_vanilla("plains"),
                features: false,
                lakes: false,
                structure_overrides: None,
            },
        }
    }
}

impl SuperflatPresetBuilder {
    #[must_use]
    pub fn layer(mut self, block: impl Into<BlockState>, height: u32) -> Self {
        self.settings.layers.push((block.into(), height));
        self
    }

    #[must_use]
    pub fn biome(mut self, biome: Key) -> Self {
        self.settings.biome = biome;
        self
    }

    #[must_use]
    pub const fn features(mut self, features: bool) -> Self {
        self.settings.features = features;
        self
    }

    #[must_use]
    pub const fn lakes(mut self, lakes: bool) -> Self {
        self.settings.lakes = lakes;
        self
    }

    #[must_use]
    pub fn structures(mut self, structure_sets: impl IntoIterator<Item = Key>) -> Self {
        self.settings.structure_overrides = Some(structure_sets.into_iter().collect());
        self
    }

    pub fn build(self) -> SuperflatPreset {
        SuperflatPreset {
            source: FlatSource::Custom(self.settings),
        }
    }
}
