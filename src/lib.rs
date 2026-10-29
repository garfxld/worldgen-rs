#![feature(portable_simd, const_cmp, const_trait_impl)]

pub mod datapack;
pub mod error;
pub mod preset;
pub mod registry;
pub mod util;
pub mod world;

pub use error::Error;
pub use preset::{
    AmplifiedPreset, DatapackPreset, DebugPreset, DefaultPreset, LargeBiomesPreset,
    SingleBiomePreset, SuperflatPreset, SuperflatPresetBuilder, WorldPreset,
};
use shulkr::util::Key;
pub use util::pos::ChunkPos;
pub use world::chunk::status::ChunkStatus;
pub use world::chunk::view::ChunkView;
pub use world::worldgen::{Result, Worldgen, WorldgenSettings};

use crate::datapack::Datapack;

pub struct WorldgenBuilder {
    seed: i64,
    dimension: Key,
    datapack: Datapack,
    preset: WorldPreset,
}

impl WorldgenBuilder {
    pub fn new() -> Self {
        Self {
            seed: 0,
            dimension: Key::vanilla("overworld"),
            datapack: Datapack::builtin(),
            preset: WorldPreset::default(),
        }
    }

    #[must_use]
    pub const fn with_seed(mut self, seed: i64) -> Self {
        self.seed = seed;
        self
    }

    #[must_use]
    pub fn with_dimension(mut self, dimension: Key) -> Self {
        self.dimension = dimension;
        self
    }

    #[must_use]
    pub fn with_datapack(mut self, datapack: Datapack) -> Self {
        self.datapack = datapack;
        self
    }

    #[must_use]
    pub fn with_preset(mut self, preset: impl Into<WorldPreset>) -> Self {
        self.preset = preset.into();
        self
    }

    pub fn build(self) -> Result<Worldgen> {
        Worldgen::new(WorldgenSettings {
            seed: self.seed,
            dimension: self.dimension,
            datapack: self.datapack,
            preset: self.preset,
        })
    }
}

impl Default for WorldgenBuilder {
    fn default() -> Self {
        Self::new()
    }
}
