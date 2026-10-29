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
use crate::preset::WorldPreset;
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
    pub preset: WorldPreset,
}

impl WorldgenSettings {
    pub fn new(seed: i64, dimension: Key) -> Self {
        Self {
            seed,
            dimension,
            datapack: Datapack::builtin(),
            preset: WorldPreset::default(),
        }
    }

    #[must_use]
    pub fn with_preset(mut self, preset: impl Into<WorldPreset>) -> Self {
        self.preset = preset.into();
        self
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

        let spec = dimension::resolve_with_preset(
            &settings.datapack,
            &settings.dimension,
            &settings.preset,
        )
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
    use crate::preset::{
        AmplifiedPreset, DebugPreset, DefaultPreset, LargeBiomesPreset, SingleBiomePreset,
        SuperflatPreset,
    };
    use rustc_hash::FxHashSet;
    use shulkr::registry::registries;
    use shulkr::util::BlockPosition;
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

    fn overworld_with(preset: impl Into<WorldPreset>) -> Worldgen {
        Worldgen::new(
            WorldgenSettings::new(42, Key::const_vanilla("overworld")).with_preset(preset),
        )
        .unwrap()
    }

    #[test]
    fn classic_flat_stacks_its_layers_from_the_bottom() {
        let worldgen = overworld_with(SuperflatPreset::new());
        let (column, biomes) = worldgen
            .generate(ChunkPos::new(40, -40), ChunkStatus::Features, |view| {
                let column: Vec<_> = (0..5)
                    .map(|y| view.get_relative_block(3, y, 3).unwrap())
                    .collect();
                let biomes: FxHashSet<u16> = view.biomes().map(|(_, biome)| biome).collect();
                (column, biomes)
            })
            .unwrap();

        assert!(column[0].is(block::Block::BEDROCK));
        assert!(column[1].is(block::Block::DIRT));
        assert!(column[2].is(block::Block::DIRT));
        assert!(column[3].is(block::Block::GRASS_BLOCK));
        assert!(column[4].is_air());
        let plains = registries::BIOME
            .by_key(&Key::const_vanilla("plains"))
            .unwrap()
            .id();
        assert_eq!(biomes, FxHashSet::from_iter([plains]));
    }

    #[test]
    fn flat_presets_place_non_solid_layers_as_features() {
        let worldgen = overworld_with(SuperflatPreset::from_preset(Key::const_vanilla(
            "snowy_kingdom",
        )));
        let (grass, snow) = worldgen
            .generate(ChunkPos::new(-30, 12), ChunkStatus::Features, |view| {
                (
                    view.get_relative_block(8, 63, 8).unwrap(),
                    view.get_relative_block(8, 64, 8).unwrap(),
                )
            })
            .unwrap();
        assert!(grass.is(block::Block::GRASS_BLOCK));
        assert!(snow.is(block::Block::SNOW));
    }

    #[test]
    fn every_vanilla_flat_preset_generates_its_top_layer() {
        for (preset, top_y, top) in [
            (
                SuperflatPreset::classic_flat(),
                3,
                block::Block::GRASS_BLOCK,
            ),
            (
                SuperflatPreset::tunnelers_dream(),
                236,
                block::Block::GRASS_BLOCK,
            ),
            (SuperflatPreset::water_world(), 169, block::Block::WATER),
            (SuperflatPreset::overworld(), 63, block::Block::GRASS_BLOCK),
            (SuperflatPreset::snowy_kingdom(), 64, block::Block::SNOW),
            (
                SuperflatPreset::bottomless_pit(),
                5,
                block::Block::GRASS_BLOCK,
            ),
            (SuperflatPreset::desert(), 63, block::Block::SAND),
            (
                SuperflatPreset::redstone_ready(),
                119,
                block::Block::SANDSTONE,
            ),
            (SuperflatPreset::the_void(), 0, block::Block::AIR),
        ] {
            let worldgen = overworld_with(preset.clone());
            // Far from the void's starting platform; snow comes from a fill-layer feature.
            let (state, above) = worldgen
                .generate(ChunkPos::new(-37, 91), ChunkStatus::Features, |view| {
                    (
                        view.get_relative_block(9, top_y, 9).unwrap(),
                        view.get_relative_block(9, top_y + 1, 9).unwrap(),
                    )
                })
                .unwrap();
            assert!(
                state.is(top),
                "{preset:?}: expected {top:?} at {top_y}, got {state:?}"
            );
            assert!(
                top == block::Block::AIR || !above.is(top),
                "{preset:?}: the layers continue above {top_y}"
            );
        }
    }

    #[test]
    fn the_void_has_its_start_platform() {
        let worldgen = overworld_with(SuperflatPreset::the_void());
        let at = |x: i32, y: i32, z: i32| {
            worldgen
                .generate(
                    ChunkPos::new(x >> 4, z >> 4),
                    ChunkStatus::Features,
                    |view| view.get_block_state(BlockPosition::new(x, y, z)),
                )
                .unwrap()
        };
        // A 33x33 stone square around (8, 8), three blocks above the bottom of the world,
        // with cobblestone in the middle.
        assert!(at(8, -61, 8).is(block::Block::COBBLESTONE));
        for (x, z) in [(-8, -8), (24, 24), (-8, 24), (0, 0), (20, 3)] {
            assert!(at(x, -61, z).is(block::Block::STONE), "({x}, {z})");
        }
        for (x, z) in [(-9, 8), (25, 8), (8, 25), (40, 40)] {
            assert!(at(x, -61, z).is_air(), "({x}, {z})");
        }
        assert!(at(8, -60, 8).is_air());
    }

    #[test]
    fn flat_builder_layers_and_height_limit() {
        let worldgen = overworld_with(
            SuperflatPreset::builder()
                .layer(block::Block::STONE, 10)
                .layer(block::Block::SAND, 1)
                .build(),
        );
        let (top, above) = worldgen
            .generate(ChunkPos::new(5, 5), ChunkStatus::Terrain, |view| {
                (
                    view.get_relative_block(0, 10, 0).unwrap(),
                    view.get_relative_block(0, 11, 0).unwrap(),
                )
            })
            .unwrap();
        assert!(top.is(block::Block::SAND));
        assert!(above.is_air());

        let too_tall = Worldgen::new(
            WorldgenSettings::new(42, Key::const_vanilla("overworld")).with_preset(
                SuperflatPreset::builder()
                    .layer(block::Block::STONE, 5000)
                    .build(),
            ),
        );
        assert!(too_tall.is_err());
    }

    #[test]
    fn debug_world_lays_out_block_states_over_a_barrier_floor() {
        let worldgen = overworld_with(DebugPreset);
        let (barrier, first, second, gap) = worldgen
            .generate(ChunkPos::new(0, 0), ChunkStatus::Features, |view| {
                let at = |x, y, z| view.get_block_state(BlockPosition::new(x, y, z));
                (at(1, 60, 1), at(1, 70, 1), at(1, 70, 3), at(2, 70, 2))
            })
            .unwrap();
        assert!(barrier.is(block::Block::BARRIER));
        assert_eq!(first.state_id(), 0);
        assert_eq!(second.state_id(), 1);
        assert!(gap.is_air());
    }

    #[test]
    fn single_biome_fills_every_column_with_that_biome() {
        let worldgen = overworld_with(SingleBiomePreset::new(Key::const_vanilla("cherry_grove")));
        let biomes: FxHashSet<u16> = worldgen
            .generate(ChunkPos::new(7, -3), ChunkStatus::Biomes, |view| {
                view.biomes().map(|(_, biome)| biome).collect()
            })
            .unwrap();
        let cherry = registries::BIOME
            .by_key(&Key::const_vanilla("cherry_grove"))
            .unwrap()
            .id();
        assert_eq!(biomes, FxHashSet::from_iter([cherry]));

        let unknown = Worldgen::new(
            WorldgenSettings::new(42, Key::const_vanilla("overworld"))
                .with_preset(SingleBiomePreset::new(Key::const_vanilla("not_a_biome"))),
        );
        assert!(unknown.is_err());
    }

    #[test]
    fn presets_resolve_to_vanillas_world_preset_files() {
        let pack = Datapack::builtin();
        let overworld = Key::const_vanilla("overworld");
        for (preset, settings) in [
            (DefaultPreset.into(), "overworld"),
            (AmplifiedPreset.into(), "amplified"),
            (LargeBiomesPreset.into(), "large_biomes"),
        ] {
            let spec = dimension::resolve_with_preset(&pack, &overworld, &preset).unwrap();
            assert_eq!(spec.noise_settings, Key::const_vanilla(settings));
        }
        // Presets only change the overworld.
        let nether = dimension::resolve_with_preset(
            &pack,
            &Key::const_vanilla("the_nether"),
            &SuperflatPreset::new().into(),
        )
        .unwrap();
        assert_eq!(nether.noise_settings, Key::const_vanilla("nether"));
    }

    #[test]
    fn amplified_generates_taller_terrain_than_normal() {
        let surface = |preset: WorldPreset| {
            let worldgen = overworld_with(preset);
            let mut highest = i32::MIN;
            for x in 0..3 {
                for z in 0..3 {
                    let top = worldgen
                        .generate(ChunkPos::new(x * 8, z * 8), ChunkStatus::Terrain, |view| {
                            (0..view.height() as usize)
                                .rev()
                                .find(|&y| !view.get_relative_block(8, y, 8).unwrap().is_air())
                                .map_or(i32::MIN, |y| view.min_y() + y as i32)
                        })
                        .unwrap();
                    highest = highest.max(top);
                }
            }
            highest
        };
        assert!(surface(AmplifiedPreset.into()) > surface(DefaultPreset.into()));
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
