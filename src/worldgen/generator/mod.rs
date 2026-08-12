use rustc_hash::FxHashSet;

use crate::chunk::heightmap::{Heightmap, HeightmapType};
use crate::chunk::proto::{Chunk, GenerationChunk, NoisePhase};
use crate::registry::REGISTRY;
use crate::registry::biome::BiomeRef;
use crate::util::ident::{BlockStateId, Identifier};
use crate::util::pos::BlockPos;
use crate::worldgen::backend::ir::loader::{DatapackSource, LoadError};
use crate::worldgen::backend::jit::CompileError;
use crate::worldgen::biome::multi_noise::build_parameter_list;
use crate::worldgen::biome::{BiomeSourceKind, obfuscate_biome_seed};
use crate::worldgen::density::{
    Aquifer, AquiferResult, Beardifier, ColumnCache, NoiseChunk, Noises, OreVeinifier,
};
use crate::worldgen::surface::{DatapackSurfaceRules, SurfaceSystem};

mod fuzzed_biome;
mod surface;

#[derive(Debug, thiserror::Error)]
pub enum GeneratorError {
    #[error(transparent)]
    Load(#[from] LoadError),
    #[error(transparent)]
    Compile(#[from] CompileError),
}

pub struct Generator {
    biome_source: BiomeSourceKind,
    noises: Noises,
    ore_veinifier: Option<OreVeinifier>,
    default_block_id: BlockStateId,
    default_fluid_id: BlockStateId,
    aquifers_enabled: bool,
    surface_rules: DatapackSurfaceRules,
    surface_system: SurfaceSystem,
    surface_extension_biomes: SurfaceExtensionBiomes,
    /// Obfuscated seed for `BiomeManager` biome zoom fuzzing.
    biome_zoom_seed: i64,
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

fn resolve_block_state(name: &str) -> BlockStateId {
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
        })
    }

    #[must_use]
    pub const fn noises(&self) -> &Noises {
        &self.noises
    }

    #[must_use]
    pub const fn biome_source(&self) -> &BiomeSourceKind {
        &self.biome_source
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

        let mut pending_writes: Vec<(usize, usize, usize, BlockStateId)> = Vec::new();
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

                #[expect(
                    clippy::cast_sign_loss,
                    reason = "world_y - min_y is non-negative inside the chunk's Y range"
                )]
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
