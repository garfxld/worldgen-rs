use serde::Deserialize;

use crate::registry::blocks::properties::Properties;
use crate::registry::blocks::vanilla_blocks;
use crate::util::ident::BlockState;
use crate::util::noise::normal_noise::NormalNoise;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::util::rng::legacy_random::LegacyRandomSource;
use crate::util::value_providers::int::IntProvider;
use crate::worldgen::block_predicate::HolderSet;
use crate::worldgen::feature::features::DIRECTIONS;
use crate::worldgen::feature::features::blockpos_iter::between_closed;
use crate::worldgen::feature::state_provider::BlockStateProvider;
use crate::worldgen::region::WorldGenRegion;

#[derive(Debug, Deserialize)]
pub struct GeodeConfig {
    pub blocks: GeodeBlockSettings,
    pub layers: GeodeLayerSettings,
    pub crack: GeodeCrackSettings,
    #[serde(default = "default_use_potential_placements_chance")]
    pub use_potential_placements_chance: f64,
    #[serde(default)]
    pub use_alternate_layer0_chance: f64,
    #[serde(default = "default_placements_require_layer0_alternate")]
    pub placements_require_layer0_alternate: bool,
    #[serde(default = "default_outer_wall_distance")]
    pub outer_wall_distance: IntProvider,
    #[serde(default = "default_distribution_points")]
    pub distribution_points: IntProvider,
    #[serde(default = "default_point_offset")]
    pub point_offset: IntProvider,
    #[serde(default = "default_min_gen_offset")]
    pub min_gen_offset: i32,
    #[serde(default = "default_max_gen_offset")]
    pub max_gen_offset: i32,
    #[serde(default = "default_noise_multiplier")]
    pub noise_multiplier: f64,
    pub invalid_blocks_threshold: i32,
}

#[derive(Debug, Deserialize)]
pub struct GeodeBlockSettings {
    pub filling_provider: BlockStateProvider,
    pub inner_layer_provider: BlockStateProvider,
    pub alternate_inner_layer_provider: BlockStateProvider,
    pub middle_layer_provider: BlockStateProvider,
    pub outer_layer_provider: BlockStateProvider,
    pub inner_placements: Vec<BlockState>,
    pub cannot_replace: HolderSet,
    pub invalid_blocks: HolderSet,
}

#[derive(Debug, Deserialize)]
pub struct GeodeLayerSettings {
    #[serde(default = "default_filling")]
    pub filling: f64,
    #[serde(default = "default_inner_layer")]
    pub inner_layer: f64,
    #[serde(default = "default_middle_layer")]
    pub middle_layer: f64,
    #[serde(default = "default_outer_layer")]
    pub outer_layer: f64,
}

#[derive(Debug, Deserialize)]
pub struct GeodeCrackSettings {
    #[serde(default = "default_generate_crack_chance")]
    pub generate_crack_chance: f64,
    #[serde(default = "default_base_crack_size")]
    pub base_crack_size: f64,
    #[serde(default = "default_crack_point_offset")]
    pub crack_point_offset: i32,
}

const fn default_use_potential_placements_chance() -> f64 {
    0.35
}

const fn default_placements_require_layer0_alternate() -> bool {
    true
}

const fn default_outer_wall_distance() -> IntProvider {
    IntProvider::Uniform {
        min_inclusive: 4,
        max_inclusive: 5,
    }
}

const fn default_distribution_points() -> IntProvider {
    IntProvider::Uniform {
        min_inclusive: 3,
        max_inclusive: 4,
    }
}

const fn default_point_offset() -> IntProvider {
    IntProvider::Uniform {
        min_inclusive: 1,
        max_inclusive: 2,
    }
}

const fn default_min_gen_offset() -> i32 {
    -16
}

const fn default_max_gen_offset() -> i32 {
    16
}

const fn default_noise_multiplier() -> f64 {
    0.05
}

const fn default_filling() -> f64 {
    1.7
}

const fn default_inner_layer() -> f64 {
    2.2
}

const fn default_middle_layer() -> f64 {
    3.2
}

const fn default_outer_layer() -> f64 {
    4.2
}

const fn default_generate_crack_chance() -> f64 {
    1.0
}

const fn default_base_crack_size() -> f64 {
    2.0
}

const fn default_crack_point_offset() -> i32 {
    2
}

fn dist_sqr(a: BlockPos, b: BlockPos) -> f64 {
    let dx = f64::from(a.x() - b.x());
    let dy = f64::from(a.y() - b.y());
    let dz = f64::from(a.z() - b.z());
    dx * dx + dy * dy + dz * dz
}

fn inv_sqrt(value: f64) -> f64 {
    1.0 / value.sqrt()
}

fn safe_set_block(
    region: &WorldGenRegion<'_>,
    pos: BlockPos,
    state: BlockState,
    cannot_replace: &HolderSet,
) {
    if !cannot_replace.contains_block(region.get_block(pos)) {
        region.set_block(pos, state);
    }
}

fn can_cluster_grow_at_state(state: BlockState) -> bool {
    state.is_air()
        || (state.get_block() == &vanilla_blocks::WATER && state.get_fluid_state().is_full())
}

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPos,
    config: &GeodeConfig,
) -> bool {
    let min_gen_offset = config.min_gen_offset;
    let max_gen_offset = config.max_gen_offset;
    let mut points = Vec::new();
    let num_points = config.distribution_points.sample(random);
    let mut noise_random =
        RandomSource::Legacy(LegacyRandomSource::from_seed(region.seed() as u64));
    let noise = NormalNoise::create_from_random(&mut noise_random, -4, &[1.0]);
    let mut crack_points = Vec::new();
    let crack_size_adjustment = f64::from(num_points) / f64::from(config.outer_wall_distance.max());

    let layers = &config.layers;
    let blocks = &config.blocks;
    let crack = &config.crack;

    let inner_air = 1.0 / layers.filling.sqrt();
    let innermost_block_layer = 1.0 / (layers.inner_layer + crack_size_adjustment).sqrt();
    let inner_crust = 1.0 / (layers.middle_layer + crack_size_adjustment).sqrt();
    let outer_crust = 1.0 / (layers.outer_layer + crack_size_adjustment).sqrt();
    let crack_size = 1.0
        / (crack.base_crack_size
            + random.next_f64() / 2.0
            + if num_points > 3 {
                crack_size_adjustment
            } else {
                0.0
            })
        .sqrt();
    let should_generate_crack = f64::from(random.next_f32()) < crack.generate_crack_chance;
    let mut num_invalid_points = 0;

    for _ in 0..num_points {
        let x = config.outer_wall_distance.sample(random);
        let y = config.outer_wall_distance.sample(random);
        let z = config.outer_wall_distance.sample(random);
        let pos = origin.offset(x, y, z);
        let state = region.get_block(pos);
        if state.is_air() || blocks.invalid_blocks.contains_block(state) {
            num_invalid_points += 1;
            if num_invalid_points > config.invalid_blocks_threshold {
                return false;
            }
        }

        points.push((pos, config.point_offset.sample(random)));
    }

    if should_generate_crack {
        let offset_index = random.next_i32_bounded(4);
        let crack_offset = num_points * 2 + 1;
        let (dx, dz) = match offset_index {
            0 => (crack_offset, 0),
            1 => (0, crack_offset),
            2 => (crack_offset, crack_offset),
            _ => (0, 0),
        };
        crack_points.push(origin.offset(dx, 7, dz));
        crack_points.push(origin.offset(dx, 5, dz));
        crack_points.push(origin.offset(dx, 1, dz));
    }

    let mut potential_crystal_placements = Vec::new();
    let cannot_replace = &blocks.cannot_replace;

    for point_inside in between_closed(
        origin.offset(min_gen_offset, min_gen_offset, min_gen_offset),
        origin.offset(max_gen_offset, max_gen_offset, max_gen_offset),
    ) {
        let noise_offset = noise.get_value(
            f64::from(point_inside.x()),
            f64::from(point_inside.y()),
            f64::from(point_inside.z()),
        ) * config.noise_multiplier;
        let mut dist_sum_shell = 0.0;
        let mut dist_sum_crack = 0.0;

        for (point, offset) in &points {
            dist_sum_shell +=
                inv_sqrt(dist_sqr(point_inside, *point) + f64::from(*offset)) + noise_offset;
        }

        for point in &crack_points {
            dist_sum_crack +=
                inv_sqrt(dist_sqr(point_inside, *point) + f64::from(crack.crack_point_offset))
                    + noise_offset;
        }

        if dist_sum_shell < outer_crust {
            continue;
        }

        if should_generate_crack && dist_sum_crack >= crack_size && dist_sum_shell < inner_air {
            safe_set_block(
                region,
                point_inside,
                vanilla_blocks::AIR.default_state(),
                cannot_replace,
            );
        } else if dist_sum_shell >= inner_air {
            let state = blocks
                .filling_provider
                .get_state(region, random, point_inside);
            safe_set_block(region, point_inside, state, cannot_replace);
        } else if dist_sum_shell >= innermost_block_layer {
            let use_alternate_layer =
                f64::from(random.next_f32()) < config.use_alternate_layer0_chance;
            let state = if use_alternate_layer {
                blocks
                    .alternate_inner_layer_provider
                    .get_state(region, random, point_inside)
            } else {
                blocks
                    .inner_layer_provider
                    .get_state(region, random, point_inside)
            };
            safe_set_block(region, point_inside, state, cannot_replace);

            if (!config.placements_require_layer0_alternate || use_alternate_layer)
                && f64::from(random.next_f32()) < config.use_potential_placements_chance
            {
                potential_crystal_placements.push(point_inside);
            }
        } else if dist_sum_shell >= inner_crust {
            let state = blocks
                .middle_layer_provider
                .get_state(region, random, point_inside);
            safe_set_block(region, point_inside, state, cannot_replace);
        } else if dist_sum_shell >= outer_crust {
            let state = blocks
                .outer_layer_provider
                .get_state(region, random, point_inside);
            safe_set_block(region, point_inside, state, cannot_replace);
        }
    }

    let inner_placements = &blocks.inner_placements;
    for crystal_pos in potential_crystal_placements {
        if inner_placements.is_empty() {
            break;
        }

        let mut state =
            inner_placements[random.next_i32_bounded(inner_placements.len() as i32) as usize];

        for direction in DIRECTIONS {
            if state.try_get_value(&Properties::FACING).is_some() {
                state = state.set_value(&Properties::FACING, direction);
            }

            let place_pos = direction.relative(crystal_pos);
            let place_state = region.get_block(place_pos);
            if state.try_get_value(&Properties::WATERLOGGED).is_some() {
                state = state.set_value(
                    &Properties::WATERLOGGED,
                    place_state.get_fluid_state().is_source(),
                );
            }

            if can_cluster_grow_at_state(place_state) {
                safe_set_block(region, place_pos, state, cannot_replace);
                break;
            }
        }
    }

    true
}
