pub mod backend;
pub mod biome;
pub mod block_predicate;
pub mod block_survival;
pub mod carver;
pub mod density;
pub mod feature;
pub mod generator;
pub mod json;
pub mod placement;
pub mod region;
pub mod scheduler;
pub mod surface;

use std::path::{Path, PathBuf};
use std::result::Result as StdResult;
use std::sync::Once;

use crate::chunk::status::ChunkStatus;
use crate::chunk::view::ChunkView;
use crate::error::Error;
use crate::registry::{REGISTRY, Registries};
use crate::util::ident::Identifier;
use crate::util::pos::ChunkPos;
use crate::worldgen::backend::ir::loader::DatapackSource;
use crate::worldgen::generator::{BiomeSourceDimension, Generator};
use crate::worldgen::scheduler::ChunkStore;

pub type Result<T> = StdResult<T, Error>;

#[derive(Debug, Clone)]
pub struct WorldgenSettings {
    pub seed: i64,
    pub dimension: Identifier,
}

pub struct Worldgen {
    store: ChunkStore,
}

fn ensure_registry_initialized() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let mut registries =
            Registries::new_vanilla().expect("bundled vanilla datapack should load");
        registries.freeze();

        let _ = REGISTRY.init(registries);
    });
}

fn dimension_info(
    dimension: &Identifier,
) -> Result<(&'static str, &'static str, BiomeSourceDimension)> {
    match (dimension.namespace.as_ref(), dimension.path.as_ref()) {
        ("minecraft", "overworld") => {
            Ok(("overworld", "Overworld", BiomeSourceDimension::Overworld))
        }
        ("minecraft", "the_nether") => Ok(("nether", "Nether", BiomeSourceDimension::Nether)),
        ("minecraft", "the_end") => Ok(("end", "End", BiomeSourceDimension::End)),
        _ => Err(Error::Generator(format!(
            "unsupported dimension {dimension:?} (datapack-defined dimensions aren't wired up yet)"
        ))),
    }
}

impl Worldgen {
    pub fn new(settings: WorldgenSettings) -> Result<Self> {
        ensure_registry_initialized();

        let (dimension, prefix, biome_source_kind) = dimension_info(&settings.dimension)?;
        let source = DatapackSource::new(vanilla_datapack_root());
        let seed = settings.seed as u64;
        let generator = Generator::new(&source, dimension, prefix, biome_source_kind, seed)
            .map_err(|e| Error::Generator(e.to_string()))?;

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

        self.store.generate_to(&[pos], status);

        let slot = self.store.get(pos).ok_or(Error::NotReady(pos))?;
        let chunk = slot.chunk().ok_or(Error::NotReady(pos))?;
        Ok(read(&ChunkView::new(chunk)))
    }
}

fn vanilla_datapack_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/datapack/worldgen")
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "test asserts on the Result directly")]
mod tests {
    use super::*;

    #[test]
    fn generates_overworld_surface_and_rejects_unsupported_status() {
        let worldgen = Worldgen::new(WorldgenSettings {
            seed: 42,
            dimension: Identifier::const_vanilla("overworld"),
        })
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
        let result = Worldgen::new(WorldgenSettings {
            seed: 0,
            dimension: Identifier::const_vanilla("some_modded_dim"),
        });
        assert!(matches!(result, Err(Error::Generator(_))));
    }
}
