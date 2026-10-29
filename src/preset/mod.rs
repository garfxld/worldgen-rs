mod amplified;
mod datapack;
mod debug;
mod default;
mod large_biomes;
mod single_biome;
mod superflat;

pub use amplified::AmplifiedPreset;
pub use datapack::DatapackPreset;
pub use debug::DebugPreset;
pub use default::DefaultPreset;
pub use large_biomes::LargeBiomesPreset;
pub use single_biome::SingleBiomePreset;
pub(crate) use superflat::FlatSource;
pub use superflat::{FlatSettings, SuperflatPreset, SuperflatPresetBuilder};

use shulkr::util::Key;

#[derive(Debug, Clone, PartialEq)]
pub enum WorldPreset {
    Default(DefaultPreset),
    Amplified(AmplifiedPreset),
    LargeBiomes(LargeBiomesPreset),
    SingleBiome(SingleBiomePreset),
    Superflat(SuperflatPreset),
    Debug(DebugPreset),
    Datapack(DatapackPreset),
}

impl Default for WorldPreset {
    fn default() -> Self {
        Self::Default(DefaultPreset::new())
    }
}

impl WorldPreset {
    pub(crate) fn key(&self) -> Option<Key> {
        match self {
            Self::Default(_) => Some(Key::const_vanilla("normal")),
            Self::Amplified(_) => Some(Key::const_vanilla("amplified")),
            Self::LargeBiomes(_) => Some(Key::const_vanilla("large_biomes")),
            Self::Debug(_) => Some(Key::const_vanilla("debug_all_block_states")),
            Self::Datapack(preset) => Some(preset.key().clone()),
            Self::SingleBiome(_) | Self::Superflat(_) => None,
        }
    }
}

macro_rules! into_preset {
    ($($ty:ident => $variant:ident),* $(,)?) => {
        $(impl From<$ty> for WorldPreset {
            fn from(preset: $ty) -> Self {
                Self::$variant(preset)
            }
        })*
    };
}

into_preset! {
    DefaultPreset => Default,
    AmplifiedPreset => Amplified,
    LargeBiomesPreset => LargeBiomes,
    SingleBiomePreset => SingleBiome,
    SuperflatPreset => Superflat,
    DebugPreset => Debug,
    DatapackPreset => Datapack,
}
