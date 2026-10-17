use std::cell::RefCell;
use std::sync::Arc;

use shulkr::util::BlockPosition;
use shulkr::world::block::{Block, BlockState};

use crate::registry::biome::BiomeRef;
use crate::registry::block_tags;
use crate::registry::carver::CARVERS;
use crate::util::pos::ChunkPos;
use crate::util::rng::RandomSource;
use crate::util::rng::legacy_random::LegacyRandomSource;
use crate::world::chunk::carving_mask::CarvingMask;
use crate::world::chunk::heightmap::{Heightmap, HeightmapType};
use crate::world::chunk::proto::Chunk;
use crate::world::chunk::status::ChunkStatus;
use crate::world::worldgen::aquifer::FluidPicker;
use crate::world::worldgen::beardifier::Beardifier;
use crate::world::worldgen::biome::biome_manager::BiomeManager;
use crate::world::worldgen::biome::biome_source::BiomeSource;
use crate::world::worldgen::densityfunction::eval::sample_volume;
use crate::world::worldgen::densityfunction::volume::DensityVolume;
use crate::world::worldgen::noise_chunk::NoiseChunk;
use crate::world::worldgen::noise_generator_settings::{NoiseGeneratorSettings, NoiseSettings};
use crate::world::worldgen::random_state::RandomState;
use crate::world::worldgen::worldgen_context::WorldGenContext;
use rustc_hash::FxHashSet;

#[derive(Debug)]
pub struct NoiseColumn {
    pub min_y: i32,
    pub column: Vec<BlockState>,
}

impl NoiseColumn {
    pub fn get_block(&self, block_y: i32) -> BlockState {
        let index = block_y - self.min_y;
        if index >= 0 && (index as usize) < self.column.len() {
            self.column[index as usize]
        } else {
            Block::AIR.default_state()
        }
    }
}

pub struct NoiseBasedChunkGenerator {
    biome_source: Arc<BiomeSource>,
    settings: Arc<NoiseGeneratorSettings>,
    global_fluid_picker: FluidPicker,
}

impl NoiseBasedChunkGenerator {
    pub fn new(biome_source: Arc<BiomeSource>, settings: Arc<NoiseGeneratorSettings>) -> Self {
        let global_fluid_picker = FluidPicker::new(settings.sea_level, settings.default_fluid);
        Self {
            biome_source,
            settings,
            global_fluid_picker,
        }
    }

    pub const fn biome_source(&self) -> &Arc<BiomeSource> {
        &self.biome_source
    }

    fn worldgen_context(&self, chunk: &Chunk) -> WorldGenContext {
        self.generation_context(chunk.min_y(), chunk.height())
    }

    pub fn generation_context(&self, level_min_y: i32, level_height: i32) -> WorldGenContext {
        WorldGenContext::new(
            self.settings.noise_settings.min_y,
            self.settings.noise_settings.height,
            self.settings.sea_level,
            level_min_y,
            level_height,
        )
    }

    pub fn generate_carvers(
        &self,
        chunk: &Chunk,
        noise_chunk: &mut NoiseChunk<'_>,
        random_state: &RandomState,
        biome_zoom_seed: i64,
    ) {
        let biome_resolver = RefCell::new(self.biome_source.create_uncached_resolver(random_state));
        let mut random = RandomSource::Legacy(LegacyRandomSource::from_seed(0));
        let pos = chunk.pos();
        let context = self.worldgen_context(chunk);
        let protected_blocks_on_top = 7;
        let max_y = context.get_min_gen_y() + context.get_gen_depth() - 1 - protected_blocks_on_top;
        let mut mask = CarvingMask::new(context.get_min_gen_y() + 1, max_y);
        let carvers = &CARVERS;

        for dx in -8..=8 {
            for dz in -8..=8 {
                let source_pos = ChunkPos::new(pos.0.x + dx, pos.0.y + dz);
                let biome = biome_resolver.borrow_mut().get_noise_biome(
                    (source_pos.0.x * 16) >> 2,
                    0,
                    (source_pos.0.y * 16) >> 2,
                );
                for (index, key) in biome
                    .value()
                    .generation_settings()
                    .carvers()
                    .iter()
                    .enumerate()
                {
                    let carver = carvers
                        .by_key(key)
                        .unwrap_or_else(|| panic!("unknown carver {key}"));
                    random.set_large_feature_seed(
                        random_state.seed().wrapping_add(index as i64),
                        source_pos.0.x,
                        source_pos.0.y,
                    );
                    if carver.is_start_chunk(&mut random) {
                        carver.carve(&context, &mut random, pos, source_pos, &mut mask);
                    }
                }
            }
        }

        if !mask.is_empty() {
            let noise_biome = |quart_x, quart_y, quart_z| {
                biome_resolver
                    .borrow_mut()
                    .get_noise_biome(quart_x, quart_y, quart_z)
            };
            let biome_manager = BiomeManager::new(&noise_biome, biome_zoom_seed);
            self.apply_carving_mask(chunk, &mask, random_state, context, noise_chunk, &|pos| {
                biome_manager.get_biome(pos)
            });
        }
    }

    fn apply_carving_mask(
        &self,
        chunk: &Chunk,
        mask: &CarvingMask,
        random_state: &RandomState,
        context: WorldGenContext,
        noise_chunk: &mut NoiseChunk<'_>,
        biome_getter: &dyn Fn(BlockPosition) -> BiomeRef,
    ) {
        let chunk_pos = chunk.pos();
        mask.visit(|x, z, bottom_y, top_y| {
            let mut has_grass = false;
            let world_x = chunk_pos.0.x * 16 + x;
            let world_z = chunk_pos.0.y * 16 + z;
            let mut world_y = top_y;
            while world_y >= bottom_y {
                let block_pos = BlockPosition::new(world_x, world_y, world_z);
                let block_state = chunk.get_block_state(block_pos);
                if !block_state.as_block().has_tag(&block_tags::UNCARVABLE) {
                    if block_state.is(Block::GRASS_BLOCK) || block_state.is(Block::MYCELIUM) {
                        has_grass = true;
                    }
                    if let Some(state) = noise_chunk.aquifer.compute_substance(
                        &mut noise_chunk.caching_samplers,
                        world_x,
                        world_y,
                        world_z,
                        0.0,
                    ) {
                        chunk.set_block_state(block_pos, state, ChunkStatus::Biomes);
                        if noise_chunk.aquifer.should_schedule_fluid_update() && state.has_fluid() {
                            chunk.mark_pos_for_postprocessing(block_pos);
                        }
                        if has_grass {
                            let helper_pos = BlockPosition::new(world_x, world_y - 1, world_z);
                            if chunk.get_block_state(helper_pos).is(Block::DIRT)
                                && let Some(top_material) =
                                    random_state.surface_system().top_material(
                                        &self.settings.material_rule,
                                        random_state,
                                        context,
                                        biome_getter,
                                        chunk,
                                        &mut noise_chunk.caching_samplers,
                                        helper_pos,
                                        state.has_fluid(),
                                    )
                            {
                                chunk.set_block_state(
                                    helper_pos,
                                    top_material,
                                    ChunkStatus::Biomes,
                                );
                                if top_material.has_fluid() {
                                    chunk.mark_pos_for_postprocessing(helper_pos);
                                }
                            }
                        }
                    }
                }
                world_y -= 1;
            }
        });
    }

    pub const fn generator_settings(&self) -> &Arc<NoiseGeneratorSettings> {
        &self.settings
    }

    pub fn create_noise_chunk<'a>(
        &self,
        random_state: &RandomState,
        beardifier: &'a Beardifier,
        chunk_min_block_x: i32,
        chunk_min_block_z: i32,
        noise_settings: NoiseSettings,
    ) -> NoiseChunk<'a> {
        NoiseChunk::new(
            random_state,
            Some(beardifier),
            &self.settings,
            self.global_fluid_picker,
            Self::chunk_volume(chunk_min_block_x, chunk_min_block_z, noise_settings),
        )
    }

    fn chunk_volume(
        chunk_min_block_x: i32,
        chunk_min_block_z: i32,
        noise_settings: NoiseSettings,
    ) -> DensityVolume {
        DensityVolume::new(
            [16, noise_settings.height, 16],
            [chunk_min_block_x, noise_settings.min_y, chunk_min_block_z],
        )
    }

    pub fn get_base_height(
        &self,
        x: i32,
        z: i32,
        heightmap: HeightmapType,
        level_min_y: i32,
        level_height: i32,
        random_state: &RandomState,
    ) -> i32 {
        self.iterate_noise_column(
            level_min_y,
            level_height,
            random_state,
            x,
            z,
            None,
            Some(&|state| heightmap.is_opaque(state)),
        )
        .unwrap_or(level_min_y)
    }

    pub fn get_base_column(
        &self,
        x: i32,
        z: i32,
        level_min_y: i32,
        level_height: i32,
        random_state: &RandomState,
    ) -> NoiseColumn {
        let mut column = None;
        self.iterate_noise_column(
            level_min_y,
            level_height,
            random_state,
            x,
            z,
            Some(&mut column),
            None,
        );
        column.unwrap_or(NoiseColumn {
            min_y: level_min_y,
            column: Vec::new(),
        })
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors NoiseBasedChunkGenerator.iterateNoiseColumn"
    )]
    fn iterate_noise_column(
        &self,
        level_min_y: i32,
        level_height: i32,
        random_state: &RandomState,
        block_x: i32,
        block_z: i32,
        column_reference: Option<&mut Option<NoiseColumn>>,
        tester: Option<&dyn Fn(BlockState) -> bool>,
    ) -> Option<i32> {
        let noise_settings = self
            .settings
            .noise_settings
            .clamp_to_height_accessor(level_min_y, level_min_y + level_height - 1);
        if noise_settings.height <= 0 {
            return None;
        }

        let volume = DensityVolume::new(
            [1, noise_settings.height, 1],
            [block_x, noise_settings.min_y, block_z],
        );
        let mut write_to = column_reference.map(|reference| {
            reference.insert(NoiseColumn {
                min_y: volume.min_block_y,
                column: vec![BlockState::default(); volume.size_y as usize],
            })
        });

        let mut noise_chunk = NoiseChunk::new(
            random_state,
            None,
            &self.settings,
            self.global_fluid_picker,
            volume,
        );
        let default_state = self.settings.default_block;
        let final_density = random_state.get_sampler(&self.settings.noise_router.final_density);
        let mut density_buffer = vec![0.0f32; volume.size()];
        sample_volume(
            &final_density,
            &mut noise_chunk.caching_samplers,
            &mut density_buffer,
            &volume,
        );
        for y in (0..volume.size_y).rev() {
            let density = density_buffer[volume.index_unchecked(0, y, 0)];
            let block_y = volume.block_y(y);
            let state = noise_chunk
                .aquifer
                .compute_substance(
                    &mut noise_chunk.caching_samplers,
                    block_x,
                    block_y,
                    block_z,
                    f64::from(density),
                )
                .unwrap_or(default_state);
            if let Some(column) = write_to.as_mut() {
                column.column[y as usize] = state;
            }
            if tester.is_some_and(|tester| tester(state)) {
                return Some(block_y + 1);
            }
        }
        None
    }

    pub fn build_surface(
        &self,
        chunk: &Chunk,
        noise_chunk: &mut NoiseChunk<'_>,
        random_state: &RandomState,
        biome_getter: &dyn Fn(BlockPosition) -> BiomeRef,
        possible_biomes: Option<&FxHashSet<u16>>,
    ) {
        let context = self.worldgen_context(chunk);
        random_state.surface_system().build_surface(
            random_state,
            biome_getter,
            context,
            chunk,
            noise_chunk,
            &self.settings.material_rule,
            possible_biomes,
        );
    }

    pub fn do_fill(
        &self,
        random_state: &RandomState,
        noise_chunk: &mut NoiseChunk<'_>,
        chunk: &Chunk,
    ) {
        let chunk_min_y = chunk.min_y();
        let mut ocean_floor = Heightmap::new(HeightmapType::OceanFloorWg, chunk_min_y);
        let mut world_surface = Heightmap::new(HeightmapType::WorldSurfaceWg, chunk_min_y);
        let volume = noise_chunk.volume;
        let final_density = random_state.get_sampler(&self.settings.noise_router.final_density);
        let mut density_buffer = vec![0.0f32; volume.size()];
        sample_volume(
            &final_density,
            &mut noise_chunk.caching_samplers,
            &mut density_buffer,
            &volume,
        );
        let default_block = self.settings.default_block;
        let air = Block::AIR.default_state();
        let mut writes = Vec::with_capacity(volume.size_y as usize);
        for z in 0..volume.size_z {
            let block_z = volume.block_z(z);
            for x in 0..volume.size_x {
                let block_x = volume.block_x(x);
                writes.clear();
                for y in (0..volume.size_y).rev() {
                    let block_y = volume.block_y(y);
                    let density = density_buffer[volume.index_unchecked(x, y, z)];
                    let state = noise_chunk
                        .aquifer
                        .compute_substance(
                            &mut noise_chunk.caching_samplers,
                            block_x,
                            block_y,
                            block_z,
                            f64::from(density),
                        )
                        .unwrap_or(default_block);
                    if state != air {
                        writes.push((
                            x as usize,
                            (block_y - chunk_min_y) as usize,
                            z as usize,
                            state,
                        ));
                        ocean_floor.update_for_initial_fill(x as usize, block_y, z as usize, state);
                        world_surface
                            .update_for_initial_fill(x as usize, block_y, z as usize, state);
                        if noise_chunk.aquifer.should_schedule_fluid_update() && state.has_fluid() {
                            chunk.mark_pos_for_postprocessing(BlockPosition::new(
                                block_x, block_y, block_z,
                            ));
                        }
                    }
                }
                chunk.write_block_batch_for_generation(&writes);
            }
        }
        chunk.sections().recalculate_counts();
        chunk.replace_noise_heightmaps(ocean_floor, world_surface);
    }
}
