use std::sync::OnceLock;

use rustc_hash::{FxHashMap, FxHashSet};

use crate::chunk::heightmap::{Heightmap, HeightmapType};
use crate::chunk::proto::{Chunk, GenerationChunk, NoisePhase};
use crate::registry::REGISTRY;
use crate::registry::biome::BiomeRef;
use crate::registry::blocks::vanilla_blocks;
use crate::util::ident::{BlockState, Identifier};
use crate::util::pos::{BlockPos, ChunkPos};
use crate::util::rng::RandomSource;
use crate::util::rng::name_hash::NameHash;
use crate::worldgen::backend::ir::loader::{DatapackError, DatapackSource};
use crate::worldgen::backend::jit::CompileError;
use crate::worldgen::biome::multi_noise::build_parameter_list;
use crate::worldgen::biome::{BiomeSourceKind, obfuscate_biome_seed};
use crate::worldgen::density::{
    Aquifer, AquiferResult, Beardifier, ColumnCache, NoiseChunk, Noises, OreVeinifier,
};
use crate::worldgen::feature::features;
use crate::worldgen::feature::sorter::{StepFeatureData, build_features_per_step};
use crate::worldgen::structure::structure::Structure;
use crate::worldgen::structure::structure_set::StructureSet;
use crate::worldgen::surface::{DatapackSurfaceRules, SurfaceSystem};

const WORLDGEN_REGION_RANDOM: &str = "minecraft:worldgen_region_random";
pub const GENERATION_STEPS: usize = 11;

mod carver;
mod feature;
mod structure;
pub mod fuzzed_biome;
mod surface;

#[derive(Debug, thiserror::Error)]
pub enum GeneratorError {
    #[error(transparent)]
    Datapack(#[from] DatapackError),
    #[error(transparent)]
    Compile(#[from] CompileError),
}

pub struct Generator {
    biome_source: BiomeSourceKind,
    noises: Noises,
    ore_veinifier: Option<OreVeinifier>,
    default_block_id: BlockState,
    default_fluid_id: BlockState,
    aquifers_enabled: bool,
    surface_rules: DatapackSurfaceRules,
    surface_system: SurfaceSystem,
    surface_extension_biomes: SurfaceExtensionBiomes,
    biome_zoom_seed: i64,
    seed: i64,
    sea_level: i32,
    features_per_step: Vec<StepFeatureData>,
    possible_biome_ids: FxHashSet<u16>,
    possible_biomes: Vec<BiomeRef>,
    possible_structure_sets: OnceLock<Vec<&'static StructureSet>>,
    structures_by_step: OnceLock<Vec<Vec<(Identifier, &'static Structure)>>>,
    ring_positions: OnceLock<FxHashMap<Identifier, Vec<ChunkPos>>>,
}

#[derive(Clone, Copy)]
struct SurfaceExtensionBiomes {
    eroded_badlands: Option<BiomeRef>,
    frozen_ocean: Option<BiomeRef>,
    deep_frozen_ocean: Option<BiomeRef>,
}

impl SurfaceExtensionBiomes {
    fn from_possible(possible_biomes: &FxHashSet<Identifier>) -> Self {
        let lookup = |path: &str| {
            let key: Identifier = format!("minecraft:{path}")
                .parse()
                .expect("valid identifier");
            possible_biomes
                .contains(&key)
                .then(|| REGISTRY.biomes.by_key(&key))
                .flatten()
        };
        Self {
            eroded_badlands: lookup("eroded_badlands"),
            frozen_ocean: lookup("frozen_ocean"),
            deep_frozen_ocean: lookup("deep_frozen_ocean"),
        }
    }

    const fn needs_surface_biome(self) -> bool {
        self.eroded_badlands.is_some()
            || self.frozen_ocean.is_some()
            || self.deep_frozen_ocean.is_some()
    }
}

fn resolve_block_state(name: &str) -> BlockState {
    let id: Identifier = name
        .parse()
        .unwrap_or_else(|e| panic!("invalid block id {name}: {e}"));
    let block = REGISTRY
        .blocks
        .by_key(&id)
        .unwrap_or_else(|| panic!("unknown block {name}"));
    REGISTRY.blocks.get_default_state_id(block)
}

impl Generator {
    pub fn new(
        source: &DatapackSource,
        dimension: &str,
        prefix: &str,
        biome_source_kind: BiomeSourceDimension,
        seed: u64,
    ) -> Result<Self, GeneratorError> {
        let registry = source.density_function_registry()?;
        let (program, settings) = source.program(dimension, prefix, &registry)?;
        let params = source.noise_parameters()?;
        let noises = Noises::new(&program, &settings, &params, seed)?;

        let biome_source = match biome_source_kind {
            BiomeSourceDimension::Overworld => {
                let noises = Noises::new(&program, &settings, &params, seed)?;
                let parameters =
                    build_parameter_list(&source.multi_noise_biome_parameters(dimension)?);
                BiomeSourceKind::overworld(seed, noises, parameters)
            }
            BiomeSourceDimension::Nether => {
                let noises = Noises::new(&program, &settings, &params, seed)?;
                let parameters =
                    build_parameter_list(&source.multi_noise_biome_parameters(dimension)?);
                BiomeSourceKind::nether(seed, noises, parameters)
            }
            BiomeSourceDimension::End => BiomeSourceKind::end(seed),
        };

        let noises_sea_level = noises.sea_level();
        let ore_veinifier = settings.ore_veins_enabled.then(OreVeinifier::new);
        let default_block_id = resolve_block_state(&settings.default_block.name);
        let default_fluid_id = resolve_block_state(&settings.default_fluid.name);

        let surface_rules = DatapackSurfaceRules::new(
            settings.surface_rule.as_ref(),
            noises.min_y(),
            noises.height(),
        );
        let surface_system = SurfaceSystem::new(
            noises.random(),
            &params,
            surface_rules.noise_ids(),
            surface_rules.gradient_ids(),
            default_block_id,
            noises.sea_level(),
        );
        let surface_extension_biomes =
            SurfaceExtensionBiomes::from_possible(&biome_source.possible_biomes());
        let biome_zoom_seed = obfuscate_biome_seed(seed as i64);

        let possible_biome_refs = biome_source.possible_biome_refs();
        let possible_biome_ids = possible_biome_refs
            .iter()
            .map(|biome| biome.id() as u16)
            .collect();
        let features_per_step = build_features_per_step(&possible_biome_refs, |biome| {
            features().features_for_biome(*biome)
        });

        Ok(Self {
            biome_source,
            noises,
            ore_veinifier,
            default_block_id,
            default_fluid_id,
            aquifers_enabled: settings.aquifers_enabled,
            surface_rules,
            surface_system,
            surface_extension_biomes,
            biome_zoom_seed,
            seed: seed as i64,
            sea_level: noises_sea_level,
            features_per_step,
            possible_biome_ids,
            possible_biomes: possible_biome_refs,
            possible_structure_sets: OnceLock::new(),
            structures_by_step: OnceLock::new(),
            ring_positions: OnceLock::new(),
        })
    }

    pub const fn noises(&self) -> &Noises {
        &self.noises
    }

    pub const fn biome_source(&self) -> &BiomeSourceKind {
        &self.biome_source
    }

    pub const fn sea_level(&self) -> i32 {
        self.sea_level
    }

    pub const fn biome_zoom_seed(&self) -> i64 {
        self.biome_zoom_seed
    }

    pub const fn seed(&self) -> i64 {
        self.seed
    }

    pub fn get_base_height(&self, x: i32, z: i32, heightmap: HeightmapType) -> i32 {
        let noises = &self.noises;
        let min_y = noises.min_y();
        let cell_width = noises.cell_width();

        let mut noise_chunk = NoiseChunk::new_column(x, z, noises);
        let mut column_cache = ColumnCache::new(noises);
        column_cache.init_grid(
            x.div_euclid(cell_width) * cell_width,
            z.div_euclid(cell_width) * cell_width,
            noises,
        );

        let mut aquifer = Aquifer::new_sized(
            x.div_euclid(cell_width) * cell_width,
            z.div_euclid(cell_width) * cell_width,
            cell_width,
            cell_width,
            min_y,
            noises.height(),
            noises.aquifer_random(),
            self.aquifers_enabled,
            self.default_fluid_id,
            noises,
            ColumnCache::new(noises),
        );

        let default_block_id = self.default_block_id;
        noise_chunk
            .iterate_column(noises, &mut column_cache, x, z, |world_y, density, _, _| {
                let state = match aquifer.compute_substance(noises, x, world_y, z, density) {
                    AquiferResult::Solid => default_block_id,
                    AquiferResult::Fluid(state) => state,
                    AquiferResult::Air => REGISTRY.blocks.get_default_state_id(&vanilla_blocks::AIR),
                };
                heightmap.is_opaque(state)
            })
            .unwrap_or(min_y)
    }

    pub fn get_first_free_height(&self, x: i32, z: i32, heightmap: HeightmapType) -> i32 {
        self.get_base_height(x, z, heightmap)
    }

    pub fn get_first_occupied_height(&self, x: i32, z: i32, heightmap: HeightmapType) -> i32 {
        self.get_base_height(x, z, heightmap) - 1
    }

    pub fn region_random(&self, pos: ChunkPos) -> RandomSource {
        self.noises
            .random()
            .with_hash_of(&NameHash::new(WORLDGEN_REGION_RANDOM))
            .next_positional()
            .at(pos.0.x * 16, 0, pos.0.y * 16)
    }

    pub fn create_biomes(&self, chunk: &Chunk) {
        let pos = chunk.pos();
        let min_y = chunk.min_y();
        let section_count = chunk.sections().sections.len();
        let chunk_x = pos.0.x;
        let chunk_z = pos.0.y;

        let mut sampler = self.biome_source.chunk_sampler();
        sampler.init_grid(chunk_x * 16, chunk_z * 16);

        for section_index in 0..section_count {
            let section_y = (min_y / 16) + section_index as i32;
            let section = &chunk.sections().sections[section_index];
            let mut section_guard = section.write();

            for local_quart_x in 0..4i32 {
                let quart_x = chunk_x * 4 + local_quart_x;

                for local_quart_y in 0..4i32 {
                    let quart_y = section_y * 4 + local_quart_y;

                    for local_quart_z in 0..4i32 {
                        let quart_z = chunk_z * 4 + local_quart_z;

                        let biome = sampler.sample(quart_x, quart_y, quart_z);
                        let biome_id = biome.id() as u16;

                        section_guard.biomes.set(
                            local_quart_x as usize,
                            local_quart_y as usize,
                            local_quart_z as usize,
                            biome_id,
                        );
                    }
                }
            }
        }
    }

    pub fn fill_from_noise(
        &self,
        chunk: GenerationChunk<'_, NoisePhase>,
        beardifier: Option<&Beardifier>,
    ) {
        let pos = chunk.pos();
        let chunk_min_x = pos.0.x * 16;
        let chunk_min_z = pos.0.y * 16;

        let noises = &self.noises;
        let min_y = noises.min_y();
        let height = noises.height();

        let mut noise_chunk = NoiseChunk::new(chunk_min_x, chunk_min_z, noises);

        let mut column_cache = ColumnCache::new(noises);
        column_cache.init_grid(chunk_min_x, chunk_min_z, noises);

        let default_block_id = self.default_block_id;
        let ore_veinifier = &self.ore_veinifier;
        let mut aquifer = Aquifer::new(
            chunk_min_x,
            chunk_min_z,
            min_y,
            height,
            noises.aquifer_random(),
            self.aquifers_enabled,
            self.default_fluid_id,
            noises,
            ColumnCache::new(noises),
        );

        let mut pending_writes: Vec<(usize, usize, usize, BlockState)> = Vec::new();
        let mut prev_x: usize = usize::MAX;
        let mut prev_z: usize = usize::MAX;
        let mut ocean_floor_wg = Heightmap::new(HeightmapType::OceanFloorWg, min_y);
        let mut world_surface_wg = Heightmap::new(HeightmapType::WorldSurfaceWg, min_y);

        noise_chunk.fill(
            noises,
            &mut column_cache,
            beardifier,
            |local_x, world_y, local_z, density, _interpolated, _cache| {
                if local_x != prev_x || local_z != prev_z {
                    if !pending_writes.is_empty() {
                        chunk.write_block_batch(&pending_writes);
                        pending_writes.clear();
                    }
                    prev_x = local_x;
                    prev_z = local_z;
                }

                let relative_y = (world_y - min_y) as usize;
                let world_x = chunk_min_x + local_x as i32;
                let world_z = chunk_min_z + local_z as i32;

                match aquifer.compute_substance(noises, world_x, world_y, world_z, density) {
                    AquiferResult::Solid => {
                        let block = ore_veinifier
                            .as_ref()
                            .and_then(|ov| ov.compute_interpolated(world_x, world_y, world_z))
                            .unwrap_or(default_block_id);
                        pending_writes.push((local_x, relative_y, local_z, block));
                        ocean_floor_wg.update_for_initial_fill(local_x, world_y, local_z, block);
                        world_surface_wg.update_for_initial_fill(local_x, world_y, local_z, block);
                    }
                    AquiferResult::Fluid(id) => {
                        pending_writes.push((local_x, relative_y, local_z, id));
                        ocean_floor_wg.update_for_initial_fill(local_x, world_y, local_z, id);
                        world_surface_wg.update_for_initial_fill(local_x, world_y, local_z, id);
                        if aquifer.should_schedule_fluid_update() {
                            chunk.mark_pos_for_postprocessing(BlockPos::new(
                                world_x, world_y, world_z,
                            ));
                        }
                    }
                    AquiferResult::Air => {}
                }
            },
        );

        if !pending_writes.is_empty() {
            chunk.write_block_batch(&pending_writes);
        }

        chunk.replace_noise_heightmaps(ocean_floor_wg, world_surface_wg);
    }
}

pub enum BiomeSourceDimension {
    Overworld,
    Nether,
    End,
}
