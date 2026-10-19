pub mod aquifer;
pub mod backend;
pub mod beardifier;
pub mod biome;
pub mod block_predicate;
pub mod block_survival;
pub mod carver;
pub mod column;
pub mod densityfunction;
pub mod dimension;
pub mod feature;
pub mod generator;
pub mod material;
pub mod noise_based_chunk_generator;
pub mod noise_chunk;
pub mod noise_generator_settings;
pub mod noise_spawn_finder;
pub mod placement;
pub mod random_state;
pub mod region;
pub mod scheduler;
pub mod structure;
pub mod synth;
pub mod worldgen_context;

use std::result::Result as StdResult;
use std::sync::{Arc, Once};
use std::time::Instant;

use shulkr::registry::freeze;
use shulkr::util::Key;

use crate::datapack::{self, Datapack};
use crate::error::Error;
use crate::registry;
use crate::util::pos::ChunkPos;
use crate::world::chunk::status::ChunkStatus;
use crate::world::chunk::view::ChunkView;
use crate::world::worldgen::generator::Generator;
use crate::world::worldgen::scheduler::{ChunkStore, chebyshev_neighbors};

pub type Result<T> = StdResult<T, Error>;

#[derive(Debug, Clone)]
pub struct WorldgenSettings {
    pub seed: i64,
    pub dimension: Key,
    pub datapack: Datapack,
}

impl WorldgenSettings {
    pub fn new(seed: i64, dimension: Key) -> Self {
        Self {
            seed,
            dimension,
            datapack: Datapack::builtin(),
        }
    }

    #[must_use]
    pub fn with_datapack(mut self, datapack: Datapack) -> Self {
        self.datapack = datapack;
        self
    }
}

#[derive(Clone)]
pub struct Worldgen {
    store: Arc<ChunkStore>,
}

pub(crate) fn ensure_registry_initialized(datapack: &Datapack) -> Result<()> {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let start = Instant::now();
        let _ = datapack::set_active(datapack.clone());
        registry::load(datapack::active()).expect("datapack registries should load");
        // Freeze inside the `Once` so no caller can read a registry mid-freeze;
        // generation reads them constantly, and frozen reads take no lock.
        freeze();
        tracing::debug!("registries loaded in {:?}", start.elapsed());
    });

    if !datapack::active().same_layers(datapack) {
        return Err(Error::Generator(
            "a different datapack is already active in this process; registries are process-wide"
                .to_owned(),
        ));
    }
    Ok(())
}

impl Worldgen {
    pub(crate) fn new(settings: WorldgenSettings) -> Result<Self> {
        ensure_registry_initialized(&settings.datapack)?;

        let spec = dimension::resolve(&settings.datapack, &settings.dimension)
            .map_err(|e| Error::Generator(e.to_string()))?;
        let seed = settings.seed as u64;
        let generator = Generator::new(&settings.datapack, spec, seed)
            .map_err(|e| Error::Generator(e.to_string()))?;

        Ok(Self {
            store: Arc::new(ChunkStore::new(generator)),
        })
    }

    pub fn generate<T>(
        &self,
        pos: ChunkPos,
        status: ChunkStatus,
        read: impl FnOnce(&ChunkView<'_>) -> T,
    ) -> Result<T> {
        if status > ChunkStatus::Features {
            return Err(Error::Unsupported(status));
        }

        if status == ChunkStatus::Features {
            let neighbors: Vec<ChunkPos> = chebyshev_neighbors(pos, 1).collect();
            self.store.generate_to(&neighbors, status);
        } else {
            self.store.generate_to(&[pos], status);
        }

        let slot = self.store.get(pos).ok_or(Error::NotReady(pos))?;
        let chunk = slot.chunk().ok_or(Error::NotReady(pos))?;
        Ok(read(&ChunkView::new(chunk)))
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "tests assert on the Result directly")]
mod tests {
    use super::*;
    use shulkr::world::block;

    #[test]
    fn generates_nether_surface_up_to_its_solid_ceiling() {
        let worldgen =
            Worldgen::new(WorldgenSettings::new(7, Key::const_vanilla("the_nether"))).unwrap();

        let pos = ChunkPos::new(0, 0);
        let (height, ceiling, above_noise) = worldgen
            .generate(pos, ChunkStatus::Terrain, |view| {
                (
                    view.height(),
                    view.get_relative_block(0, 127, 0),
                    view.get_relative_block(0, (view.height() - 1) as usize, 0),
                )
            })
            .unwrap();

        assert_eq!(height, 256);
        assert!(ceiling.is_some_and(|state| state.is(block::Block::BEDROCK)));
        assert!(above_noise.is_none_or(|state| state.is_air()));
    }

    #[test]
    fn generates_overworld_surface_and_rejects_unsupported_status() {
        let worldgen =
            Worldgen::new(WorldgenSettings::new(42, Key::const_vanilla("overworld"))).unwrap();

        let pos = ChunkPos::new(0, 0);
        let top_block = worldgen
            .generate(pos, ChunkStatus::Terrain, |view| {
                assert_eq!(view.pos(), pos);
                view.get_relative_block(0, (view.height() - 1) as usize, 0)
            })
            .unwrap();
        assert!(top_block.is_some());

        let err = worldgen
            .generate(pos, ChunkStatus::Full, |_| ())
            .unwrap_err();
        assert!(matches!(err, Error::Unsupported(ChunkStatus::Full)));
    }

    #[test]
    fn rejects_unsupported_dimension() {
        let result = Worldgen::new(WorldgenSettings::new(
            0,
            Key::const_vanilla("some_modded_dim"),
        ));
        assert!(matches!(result, Err(Error::Generator(_))));
    }
}
