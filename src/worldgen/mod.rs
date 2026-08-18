pub mod backend;
pub mod biome;
pub mod block_predicate;
pub mod block_survival;
pub mod carver;
pub mod column;
pub mod density;
pub mod dimension;
pub mod feature;
pub mod generator;
pub mod placement;
pub mod region;
pub mod scheduler;
pub mod structure;
pub mod surface;

use std::result::Result as StdResult;
use std::sync::Once;
use std::time::Instant;

use crate::chunk::status::ChunkStatus;
use crate::chunk::view::ChunkView;
use crate::datapack::{self, Datapack};
use crate::error::Error;
use crate::registry::{REGISTRY, Registries};
use crate::util::ident::Identifier;
use crate::util::pos::ChunkPos;
use crate::worldgen::backend::ir::loader::DatapackSource;
use crate::worldgen::generator::Generator;
use crate::worldgen::scheduler::{ChunkStore, chebyshev_neighbors};

pub type Result<T> = StdResult<T, Error>;

#[derive(Debug, Clone)]
pub struct WorldgenSettings {
    pub seed: i64,
    pub dimension: Identifier,
    pub datapack: Datapack,
}

impl WorldgenSettings {
    pub fn new(seed: i64, dimension: Identifier) -> Self {
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

pub struct Worldgen {
    store: ChunkStore,
}

fn ensure_registry_initialized(datapack: &Datapack) -> Result<()> {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let start = Instant::now();
        let _ = datapack::set_active(datapack.clone());
        let mut registries = Registries::new_with_datapack(datapack::active())
            .expect("datapack registries should load");
        registries.freeze();

        let _ = REGISTRY.init(registries);
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
    pub fn new(settings: WorldgenSettings) -> Result<Self> {
        ensure_registry_initialized(&settings.datapack)?;

        let source = DatapackSource::new(settings.datapack.clone());
        let spec = dimension::resolve(&source, &settings.dimension)
            .map_err(|e| Error::Generator(e.to_string()))?;
        let seed = settings.seed as u64;
        let generator =
            Generator::new(&source, &spec, seed).map_err(|e| Error::Generator(e.to_string()))?;

        Ok(Self {
            store: ChunkStore::new(generator),
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
#[expect(clippy::unwrap_used, reason = "test asserts on the Result directly")]
mod tests {
    use super::*;

    #[test]
    fn generates_nether_surface_up_to_its_solid_ceiling() {
        let worldgen = Worldgen::new(WorldgenSettings::new(
            7,
            Identifier::const_vanilla("the_nether"),
        ))
        .unwrap();

        let pos = ChunkPos::new(0, 0);
        let (height, ceiling) = worldgen
            .generate(pos, ChunkStatus::Surface, |view| {
                (
                    view.height(),
                    view.get_relative_block(0, (view.height() - 1) as usize, 0),
                )
            })
            .unwrap();

        assert_eq!(height, 128);
        assert!(
            ceiling.is_some_and(|state| !state.is_air()),
            "the nether's top layer is bedrock, so the surface pass must handle a full column"
        );
    }

    #[test]
    fn generates_overworld_surface_and_rejects_unsupported_status() {
        let worldgen = Worldgen::new(WorldgenSettings::new(
            42,
            Identifier::const_vanilla("overworld"),
        ))
        .unwrap();

        let pos = ChunkPos::new(0, 0);
        let top_block = worldgen
            .generate(pos, ChunkStatus::Surface, |view| {
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
            Identifier::const_vanilla("some_modded_dim"),
        ));
        assert!(matches!(result, Err(Error::Generator(_))));
    }
}
