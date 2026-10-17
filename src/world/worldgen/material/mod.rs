pub mod condition;
pub mod registry;
pub mod rule;
pub mod rule_context;

use std::sync::Arc;

use rustc_hash::FxHashSet;
use shulkr::util::{BlockPosition, Key};
use shulkr::world::block::{Block, BlockState};

use crate::registry::biome::{BiomeRef, should_melt_frozen_ocean_iceberg_slightly};
use crate::util::rng::name_hash::NameHash;
use crate::util::rng::{RandomFactory, RandomSource};
use crate::world::chunk::heightmap::HeightmapType;
use crate::world::chunk::proto::Chunk;
use crate::world::chunk::status::ChunkStatus;
use crate::world::worldgen::densityfunction::context::SamplerContext;
use crate::world::worldgen::densityfunction::function::{Df, java_round};
use crate::world::worldgen::densityfunction::volume::DensityVolume;
use crate::world::worldgen::noise_chunk::NoiseChunk;
use crate::world::worldgen::random_state::RandomState;
use crate::world::worldgen::synth::Noise;
use crate::world::worldgen::worldgen_context::WorldGenContext;

use rule::MaterialRule;
use rule_context::MaterialRuleContext;

const DIMENSION_MIN_Y: i32 = -2032;
const WAY_BELOW_MIN_Y: i32 = DIMENSION_MIN_Y << 4;
const CLAY_BANDS_RANDOM: &str = "minecraft:clay_bands";

fn state(path: &'static str) -> BlockState {
    BlockState::from_key(Key::const_vanilla(path))
        .unwrap_or_else(|| panic!("missing block minecraft:{path}"))
}

pub struct MaterialSystem {
    default_block: BlockState,
    sea_level: i32,
    preliminary_surface_function: Df,
    clay_bands: [BlockState; 192],
    clay_bands_offset_noise: Arc<Noise>,
    badlands_pillar_noise: Arc<Noise>,
    badlands_pillar_roof_noise: Arc<Noise>,
    badlands_surface_noise: Arc<Noise>,
    iceberg_pillar_noise: Arc<Noise>,
    iceberg_pillar_roof_noise: Arc<Noise>,
    iceberg_surface_noise: Arc<Noise>,
    noise_random: RandomFactory,
    surface_noise: Arc<Noise>,
    surface_secondary_noise: Arc<Noise>,
}

struct ChunkColumn<'a> {
    chunk: &'a Chunk,
    min_y: i32,
    max_y: i32,
    x: i32,
    z: i32,
}

impl ChunkColumn<'_> {
    fn get_block(&self, block_y: i32) -> BlockState {
        self.chunk
            .get_block_state(BlockPosition::new(self.x, block_y, self.z))
    }

    fn set_block(&self, block_y: i32, state: BlockState) {
        if block_y >= self.min_y && block_y <= self.max_y {
            let pos = BlockPosition::new(self.x, block_y, self.z);
            self.chunk.set_block_state(pos, state, ChunkStatus::Biomes);
            if state.has_fluid() {
                self.chunk.mark_pos_for_postprocessing(pos);
            }
        }
    }
}

fn is_stone(state: BlockState) -> bool {
    !state.is_air() && !state.has_fluid()
}

impl MaterialSystem {
    pub fn new(
        random_state: &RandomState,
        default_block: BlockState,
        sea_level: i32,
        preliminary_surface_function: Df,
        noise_random: RandomFactory,
    ) -> Self {
        let clay_bands_offset_noise =
            random_state.get_or_create_noise("minecraft:clay_bands_offset");
        let clay_bands =
            generate_bands(&mut noise_random.with_hash_of(&NameHash::new(CLAY_BANDS_RANDOM)));
        Self {
            default_block,
            sea_level,
            preliminary_surface_function,
            clay_bands,
            clay_bands_offset_noise,
            surface_noise: random_state.get_or_create_noise("minecraft:surface"),
            surface_secondary_noise: random_state
                .get_or_create_noise("minecraft:surface_secondary"),
            badlands_pillar_noise: random_state.get_or_create_noise("minecraft:badlands_pillar"),
            badlands_pillar_roof_noise: random_state
                .get_or_create_noise("minecraft:badlands_pillar_roof"),
            badlands_surface_noise: random_state.get_or_create_noise("minecraft:badlands_surface"),
            iceberg_pillar_noise: random_state.get_or_create_noise("minecraft:iceberg_pillar"),
            iceberg_pillar_roof_noise: random_state
                .get_or_create_noise("minecraft:iceberg_pillar_roof"),
            iceberg_surface_noise: random_state.get_or_create_noise("minecraft:iceberg_surface"),
            noise_random,
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors MaterialSystem.buildSurface"
    )]
    pub fn build_surface(
        &self,
        random_state: &RandomState,
        biome_getter: &dyn Fn(BlockPosition) -> BiomeRef,
        generation_context: WorldGenContext,
        proto_chunk: &Chunk,
        noise_chunk: &mut NoiseChunk<'_>,
        rule_source: &MaterialRule,
        possible_biomes: Option<&FxHashSet<u16>>,
    ) {
        let chunk_pos = proto_chunk.pos();
        let min_block_x = chunk_pos.0.x * 16;
        let min_block_z = chunk_pos.0.y * 16;
        let min_y = proto_chunk.min_y();
        let max_y = proto_chunk.min_y() + proto_chunk.height() - 1;
        let highest_filled_section_index = proto_chunk.get_highest_filled_section_index();
        let max_block_y = ((min_y >> 4) + highest_filled_section_index) * 16 + 15;
        let full_volume = noise_chunk.volume;
        let narrowed_volume = DensityVolume::new(
            [
                full_volume.size_x,
                (max_block_y - full_volume.min_block_y + 1).max(1),
                full_volume.size_z,
            ],
            [
                full_volume.min_block_x,
                full_volume.min_block_y,
                full_volume.min_block_z,
            ],
        );
        let mut context = MaterialRuleContext::new(
            self,
            random_state,
            narrowed_volume,
            &mut noise_chunk.caching_samplers,
            biome_getter,
            generation_context,
            possible_biomes,
        );
        let mut rule = rule_source.compile(&mut context);
        let eroded_badlands = Key::const_vanilla("eroded_badlands");
        let frozen_ocean = Key::const_vanilla("frozen_ocean");
        let deep_frozen_ocean = Key::const_vanilla("deep_frozen_ocean");

        for x in 0..16 {
            for z in 0..16 {
                let block_x = min_block_x + x;
                let block_z = min_block_z + z;
                let starting_height =
                    proto_chunk.get_height(HeightmapType::WorldSurfaceWg, x, z) + 1;
                let column = ChunkColumn {
                    chunk: proto_chunk,
                    min_y,
                    max_y,
                    x: block_x,
                    z: block_z,
                };
                let surface_biome =
                    biome_getter(BlockPosition::new(block_x, starting_height, block_z));
                if *surface_biome.key() == eroded_badlands {
                    self.eroded_badlands_extension(
                        &column,
                        block_x,
                        block_z,
                        starting_height,
                        min_y,
                    );
                }

                let height = proto_chunk.get_height(HeightmapType::WorldSurfaceWg, x, z) + 1;
                let gradient_x = get_surface_gradient_x(proto_chunk, x, z);
                let gradient_z = get_surface_gradient_z(proto_chunk, x, z);
                context.update_xz(block_x, block_z, gradient_x, gradient_z);
                let mut stone_above_depth = 0;
                let mut water_height = i32::MIN;
                let mut next_ceiling_stone_y = i32::MAX;
                let end_y = proto_chunk.min_y();

                let mut y = height;
                while y >= end_y {
                    let old = column.get_block(y);
                    if old.is_air() {
                        stone_above_depth = 0;
                        water_height = i32::MIN;
                    } else if old.has_fluid() {
                        if water_height == i32::MIN {
                            water_height = y + 1;
                        }
                    } else {
                        if next_ceiling_stone_y >= y {
                            next_ceiling_stone_y = WAY_BELOW_MIN_Y;
                            let mut lookahead_y = y - 1;
                            while lookahead_y >= end_y - 1 {
                                let next_state = column.get_block(lookahead_y);
                                if !is_stone(next_state) {
                                    next_ceiling_stone_y = lookahead_y + 1;
                                    break;
                                }
                                lookahead_y -= 1;
                            }
                        }

                        stone_above_depth += 1;
                        let stone_below_depth = y - next_ceiling_stone_y + 1;
                        context.update_y(stone_above_depth, stone_below_depth, water_height, y);
                        if y >= min_y
                            && y <= max_y
                            && let Some(state) = rule.try_apply(&mut context, block_x, y, block_z)
                        {
                            column.set_block(y, state);
                        }
                    }
                    y -= 1;
                }

                let key = surface_biome.key();
                if *key == frozen_ocean || *key == deep_frozen_ocean {
                    let min_surface_level = context.get_min_surface_level();
                    self.frozen_ocean_extension(
                        min_surface_level,
                        surface_biome,
                        &column,
                        block_x,
                        block_z,
                        starting_height,
                    );
                }
            }
        }
    }

    pub fn get_surface_depth(&self, block_x: i32, block_z: i32) -> i32 {
        let noise_value = f64::from(self.surface_noise.get(
            f64::from(block_x),
            0.0,
            f64::from(block_z),
        ));
        (noise_value * 2.75 + 3.0 + self.noise_random.at(block_x, 0, block_z).next_f64() * 0.25)
            as i32
    }

    pub fn get_surface_secondary(&self, block_x: i32, block_z: i32) -> f64 {
        f64::from(
            self.surface_secondary_noise
                .get(f64::from(block_x), 0.0, f64::from(block_z)),
        )
    }

    pub const fn get_sea_level(&self) -> i32 {
        self.sea_level
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors MaterialSystem.topMaterial"
    )]
    pub fn top_material(
        &self,
        rule_source: &MaterialRule,
        random_state: &RandomState,
        worldgen_context: WorldGenContext,
        biome_getter: &dyn Fn(BlockPosition) -> BiomeRef,
        chunk: &Chunk,
        density_samplers: &mut SamplerContext<'_>,
        pos: BlockPosition,
        under_fluid: bool,
    ) -> Option<BlockState> {
        let volume = DensityVolume::new([1, 1, 1], [pos.x(), pos.y(), pos.z()]);
        let mut context = MaterialRuleContext::new(
            self,
            random_state,
            volume,
            density_samplers,
            biome_getter,
            worldgen_context,
            None,
        );
        let mut rule = rule_source.compile(&mut context);
        let block_x = pos.x();
        let block_y = pos.y();
        let block_z = pos.z();
        let gradient_x = get_surface_gradient_x(chunk, pos.x() & 15, pos.z() & 15);
        let gradient_z = get_surface_gradient_z(chunk, pos.x() & 15, pos.z() & 15);
        context.update_xz(block_x, block_z, gradient_x, gradient_z);
        context.update_y(
            1,
            1,
            if under_fluid { block_y + 1 } else { i32::MIN },
            block_y,
        );
        rule.try_apply(&mut context, block_x, block_y, block_z)
    }

    fn eroded_badlands_extension(
        &self,
        column: &ChunkColumn<'_>,
        block_x: i32,
        block_z: i32,
        height: i32,
        min_y: i32,
    ) {
        let x = f64::from(block_x);
        let z = f64::from(block_z);
        let pillar_buffer = (f64::from(self.badlands_surface_noise.get(x, 0.0, z)) * 8.25)
            .abs()
            .min(f64::from(
                self.badlands_pillar_noise.get(x * 0.2, 0.0, z * 0.2) * 15.0,
            ));
        if pillar_buffer <= 0.0 {
            return;
        }
        let pillar_floor =
            (f64::from(self.badlands_pillar_roof_noise.get(x * 0.75, 0.0, z * 0.75)) * 1.5).abs();
        let extension_top =
            64.0 + (pillar_buffer * pillar_buffer * 2.5).min((pillar_floor * 50.0).ceil() + 24.0);
        let start_y = extension_top.floor() as i32;
        if height > start_y {
            return;
        }
        let mut y = start_y;
        while y >= min_y {
            let old_state = column.get_block(y);
            if old_state.is(self.default_block.as_block()) {
                break;
            }
            if old_state.is(Block::WATER) {
                return;
            }
            y -= 1;
        }
        let mut y = start_y;
        while y >= min_y && column.get_block(y).is_air() {
            column.set_block(y, self.default_block);
            y -= 1;
        }
    }

    fn frozen_ocean_extension(
        &self,
        min_surface_level: i32,
        surface_biome: BiomeRef,
        column: &ChunkColumn<'_>,
        block_x: i32,
        block_z: i32,
        height: i32,
    ) {
        let x = f64::from(block_x);
        let z = f64::from(block_z);
        let iceberg = (f64::from(self.iceberg_surface_noise.get(x, 0.0, z)) * 8.25)
            .abs()
            .min(f64::from(
                self.iceberg_pillar_noise.get(x * 1.28, 0.0, z * 1.28) * 15.0,
            ));
        if iceberg <= 1.8 {
            return;
        }
        let iceberg_roof =
            (f64::from(self.iceberg_pillar_roof_noise.get(x * 1.17, 0.0, z * 1.17)) * 1.5).abs();
        let mut top = (iceberg * iceberg * 1.2).min((iceberg_roof * 40.0).ceil() + 14.0);
        if should_melt_frozen_ocean_iceberg_slightly(
            surface_biome.value(),
            BlockPosition::new(block_x, self.sea_level, block_z),
            self.sea_level,
        ) {
            top -= 2.0;
        }
        if top <= 2.0 {
            return;
        }
        let extension_bottom = f64::from(self.sea_level) - top - 7.0;
        top += f64::from(self.sea_level);
        let extension_top = top;
        let mut random = self.noise_random.at(block_x, 0, block_z);
        let max_snow_depth = 2 + random.next_i32_bounded(4);
        let min_snow_height = self.sea_level + 18 + random.next_i32_bounded(10);
        let mut snow_depth = 0;
        let snow_block = Block::SNOW_BLOCK.default_state();
        let packed_ice = Block::PACKED_ICE.default_state();

        let mut y = height.max(extension_top as i32 + 1);
        while y >= min_surface_level {
            if column.get_block(y).is_air() && y < extension_top as i32 && random.next_f64() > 0.01
                || column.get_block(y).is(Block::WATER)
                    && y > extension_bottom as i32
                    && y < self.sea_level
                    && random.next_f64() > 0.15
            {
                if snow_depth <= max_snow_depth && y > min_snow_height {
                    column.set_block(y, snow_block);
                    snow_depth += 1;
                } else {
                    column.set_block(y, packed_ice);
                }
            }
            y -= 1;
        }
    }

    pub fn get_band(&self, world_x: i32, y: i32, world_z: i32) -> BlockState {
        let offset = java_round(
            self.clay_bands_offset_noise
                .get(f64::from(world_x), 0.0, f64::from(world_z))
                * 4.0,
        );
        let len = self.clay_bands.len() as i32;
        self.clay_bands[((y + offset + len) % len) as usize]
    }

    pub const fn preliminary_surface_function(&self) -> &Df {
        &self.preliminary_surface_function
    }
}

fn get_surface_gradient_x(proto_chunk: &Chunk, x: i32, z: i32) -> i32 {
    proto_chunk.get_height(HeightmapType::WorldSurfaceWg, (x + 1).min(15), z)
        - proto_chunk.get_height(HeightmapType::WorldSurfaceWg, (x - 1).max(0), z)
}

fn get_surface_gradient_z(proto_chunk: &Chunk, x: i32, z: i32) -> i32 {
    proto_chunk.get_height(HeightmapType::WorldSurfaceWg, x, (z + 1).min(15))
        - proto_chunk.get_height(HeightmapType::WorldSurfaceWg, x, (z - 1).max(0))
}

fn generate_bands(random: &mut RandomSource) -> [BlockState; 192] {
    let terracotta = state("terracotta");
    let mut clay_bands = [terracotta; 192];
    let len = clay_bands.len() as i32;

    let mut i = 0;
    while i < len {
        i += random.next_i32_bounded(5) + 1;
        if i < len {
            clay_bands[i as usize] = state("orange_terracotta");
        }
        i += 1;
    }

    make_bands(random, &mut clay_bands, 1, state("yellow_terracotta"));
    make_bands(random, &mut clay_bands, 2, state("brown_terracotta"));
    make_bands(random, &mut clay_bands, 1, state("red_terracotta"));
    let white_band_count = random.next_i32_between(9, 15);
    let white = state("white_terracotta");
    let light_gray = state("light_gray_terracotta");
    let mut i = 0;
    let mut start = 0;
    while i < white_band_count && start < len {
        clay_bands[start as usize] = white;
        if start - 1 > 0 && random.next_bool() {
            clay_bands[(start - 1) as usize] = light_gray;
        }
        if start + 1 < len && random.next_bool() {
            clay_bands[(start + 1) as usize] = light_gray;
        }
        i += 1;
        start += random.next_i32_bounded(16) + 4;
    }
    clay_bands
}

fn make_bands(
    random: &mut RandomSource,
    clay_bands: &mut [BlockState; 192],
    base_width: i32,
    state: BlockState,
) {
    let band_count = random.next_i32_between(6, 15);
    let len = clay_bands.len() as i32;
    for _ in 0..band_count {
        let width = base_width + random.next_i32_bounded(3);
        let start = random.next_i32_bounded(len);
        let mut p = 0;
        while start + p < len && p < width {
            clay_bands[(start + p) as usize] = state;
            p += 1;
        }
    }
}
