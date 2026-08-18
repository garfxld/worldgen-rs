use serde::Deserialize;

use crate::registry::blocks::vanilla_blocks;
use crate::registry::fluid::is_water_fluid;
use crate::util::direction::Direction;
use crate::util::ident::{BlockState, Identifier};
use crate::util::math::map;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::util::value_providers::FloatProvider;
use crate::util::value_providers::int::IntProvider;
use crate::worldgen::block_predicate::HolderSet;
use crate::worldgen::column::Column;
use crate::worldgen::feature::features::speleothem::{
    grow_speleothem, is_empty_or_water, is_neither_empty_nor_water, place_base_block_if_possible,
};
use crate::worldgen::region::WorldGenRegion;

const BASE_STONE_OVERWORLD: Identifier = Identifier::const_vanilla("base_stone_overworld");

#[derive(Debug, Deserialize)]
pub struct SpeleothemClusterConfig {
    pub base_block: BlockState,
    pub pointed_block: BlockState,
    pub replaceable_blocks: HolderSet,
    pub height: IntProvider,
    pub wetness: FloatProvider,
    pub density: FloatProvider,
    pub radius: IntProvider,
    #[serde(default = "default_floor_to_ceiling_search_range")]
    pub floor_to_ceiling_search_range: i32,
    #[serde(default = "default_height_deviation")]
    pub height_deviation: i32,
    #[serde(default = "default_max_distance_from_center_affecting_height_bias")]
    pub max_distance_from_center_affecting_height_bias: i32,
    #[serde(default = "default_max_distance_from_edge_affecting_chance_of_speleothem")]
    pub max_distance_from_edge_affecting_chance_of_speleothem: i32,
    #[serde(default = "default_max_stalagmite_stalactite_height_diff")]
    pub max_stalagmite_stalactite_height_diff: i32,
    pub speleothem_block_layer_thickness: IntProvider,
    #[serde(default = "default_chance_of_speleothem_at_max_distance_from_center")]
    pub chance_of_speleothem_at_max_distance_from_center: f32,
}

const fn default_floor_to_ceiling_search_range() -> i32 {
    12
}

const fn default_height_deviation() -> i32 {
    3
}

const fn default_max_distance_from_center_affecting_height_bias() -> i32 {
    8
}

const fn default_max_distance_from_edge_affecting_chance_of_speleothem() -> i32 {
    3
}

const fn default_max_stalagmite_stalactite_height_diff() -> i32 {
    1
}

const fn default_chance_of_speleothem_at_max_distance_from_center() -> f32 {
    0.1
}

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPos,
    config: &SpeleothemClusterConfig,
) -> bool {
    if !is_empty_or_water(region.get_block(origin)) {
        return false;
    }

    let height = config.height.sample(random);
    let wetness = config.wetness.sample(random);
    let density = config.density.sample(random);
    let x_radius = config.radius.sample(random);
    let z_radius = config.radius.sample(random);

    for dx in -x_radius..=x_radius {
        for dz in -z_radius..=z_radius {
            let chance = get_chance_of_stalagmite_or_stalactite(x_radius, z_radius, dx, dz, config);
            let pos = origin.offset(dx, 0, dz);
            place_column(
                region, random, pos, dx, dz, wetness, chance, height, density, config,
            );
        }
    }

    true
}

#[expect(clippy::too_many_arguments, reason = "vanilla parameter list")]
fn place_column(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    pos: BlockPos,
    dx: i32,
    dz: i32,
    chance_of_water: f32,
    chance_of_stalagmite_or_stalactite: f64,
    cluster_height: i32,
    density: f32,
    config: &SpeleothemClusterConfig,
) {
    let Some(base_column) = Column::scan(
        region,
        pos,
        config.floor_to_ceiling_search_range,
        is_empty_or_water,
        is_neither_empty_nor_water,
    ) else {
        return;
    };

    let ceiling = base_column.ceiling();
    let base_floor = base_column.floor();
    if ceiling.is_none() && base_floor.is_none() {
        return;
    }

    let want_pool = random.next_f32() < chance_of_water;
    let column = match base_floor {
        Some(floor_y) if want_pool && can_place_pool(region, pos.at_y(floor_y), config) => {
            region.set_block(pos.at_y(floor_y), vanilla_blocks::WATER.default_state());
            base_column.with_floor(Some(floor_y - 1))
        }
        _ => base_column,
    };

    let floor = column.floor();

    let want_stalactite = random.next_f64() < chance_of_stalagmite_or_stalactite;
    let stalactite_height = match ceiling {
        Some(ceiling_y) if want_stalactite && !is_lava(region, pos.at_y(ceiling_y)) => {
            let thickness = config.speleothem_block_layer_thickness.sample(random);
            replace_blocks_with_base_blocks(
                region,
                pos.at_y(ceiling_y),
                thickness,
                Direction::Up,
                config,
            );
            let max_height = match floor {
                Some(floor_y) => cluster_height.min(ceiling_y - floor_y),
                None => cluster_height,
            };
            get_speleothem_height(random, dx, dz, density, max_height, config)
        }
        _ => 0,
    };

    let want_stalagmite = random.next_f64() < chance_of_stalagmite_or_stalactite;
    let stalagmite_height = match floor {
        Some(floor_y) if want_stalagmite && !is_lava(region, pos.at_y(floor_y)) => {
            let thickness = config.speleothem_block_layer_thickness.sample(random);
            replace_blocks_with_base_blocks(
                region,
                pos.at_y(floor_y),
                thickness,
                Direction::Down,
                config,
            );
            if ceiling.is_some() {
                0.max(
                    stalactite_height
                        + random.next_i32_between(
                            -config.max_stalagmite_stalactite_height_diff,
                            config.max_stalagmite_stalactite_height_diff,
                        ),
                )
            } else {
                get_speleothem_height(random, dx, dz, density, cluster_height, config)
            }
        }
        _ => 0,
    };

    let (actual_stalactite_height, actual_stalagmite_height) = match (ceiling, floor) {
        (Some(ceiling_y), Some(floor_y))
            if ceiling_y - stalactite_height <= floor_y + stalagmite_height =>
        {
            let lowest_stalactite_bottom = (ceiling_y - stalactite_height).max(floor_y + 1);
            let highest_stalagmite_top = (floor_y + stalagmite_height).min(ceiling_y - 1);
            let actual_stalactite_bottom =
                random.next_i32_between(lowest_stalactite_bottom, highest_stalagmite_top + 1);
            let actual_stalagmite_top = actual_stalactite_bottom - 1;
            (
                ceiling_y - actual_stalactite_bottom,
                actual_stalagmite_top - floor_y,
            )
        }
        _ => (stalactite_height, stalagmite_height),
    };

    let merge_tips = random.next_bool()
        && actual_stalactite_height > 0
        && actual_stalagmite_height > 0
        && column
            .height()
            .is_some_and(|height| actual_stalactite_height + actual_stalagmite_height == height);

    if let Some(ceiling_y) = ceiling {
        grow_speleothem(
            region,
            pos.at_y(ceiling_y - 1),
            Direction::Down,
            actual_stalactite_height,
            merge_tips,
            config.base_block.get_block(),
            config.pointed_block.get_block(),
            &config.replaceable_blocks,
        );
    }

    if let Some(floor_y) = floor {
        grow_speleothem(
            region,
            pos.at_y(floor_y + 1),
            Direction::Up,
            actual_stalagmite_height,
            merge_tips,
            config.base_block.get_block(),
            config.pointed_block.get_block(),
            &config.replaceable_blocks,
        );
    }
}

fn is_lava(region: &WorldGenRegion<'_>, pos: BlockPos) -> bool {
    region.get_block(pos).get_block() == &vanilla_blocks::LAVA
}

fn replace_blocks_with_base_blocks(
    region: &WorldGenRegion<'_>,
    first_pos: BlockPos,
    max_count: i32,
    direction: Direction,
    config: &SpeleothemClusterConfig,
) {
    let mut pos = first_pos;
    for _ in 0..max_count {
        if !place_base_block_if_possible(
            region,
            pos,
            config.base_block.get_block(),
            &config.replaceable_blocks,
        ) {
            return;
        }
        pos = direction.relative(pos);
    }
}

fn get_speleothem_height(
    random: &mut RandomSource,
    dx: i32,
    dz: i32,
    density: f32,
    max_height: i32,
    config: &SpeleothemClusterConfig,
) -> i32 {
    if random.next_f32() > density {
        return 0;
    }

    let distance_from_center = dx.abs() + dz.abs();
    let height_mean = clamped_map(
        f64::from(distance_from_center),
        0.0,
        f64::from(config.max_distance_from_center_affecting_height_bias),
        f64::from(max_height) / 2.0,
        0.0,
    ) as f32;
    let sample = height_mean + config.height_deviation as f32 * random.next_gaussian() as f32;
    sample.clamp(0.0, max_height as f32) as i32
}

fn can_place_pool(
    region: &WorldGenRegion<'_>,
    pos: BlockPos,
    config: &SpeleothemClusterConfig,
) -> bool {
    let state = region.get_block(pos);
    let block = state.get_block();
    if block == &vanilla_blocks::WATER
        || block == config.base_block.get_block()
        || block == config.pointed_block.get_block()
    {
        return false;
    }

    if is_water_fluid(region.get_block(pos.above()).get_fluid_state().fluid_id) {
        return false;
    }

    for direction in Direction::HORIZONTAL {
        if !can_be_adjacent_to_water(region, direction.relative(pos)) {
            return false;
        }
    }

    can_be_adjacent_to_water(region, pos.below())
}

fn can_be_adjacent_to_water(region: &WorldGenRegion<'_>, pos: BlockPos) -> bool {
    let state = region.get_block(pos);
    state.get_block().has_tag(&BASE_STONE_OVERWORLD)
        || is_water_fluid(state.get_fluid_state().fluid_id)
}

fn get_chance_of_stalagmite_or_stalactite(
    x_radius: i32,
    z_radius: i32,
    dx: i32,
    dz: i32,
    config: &SpeleothemClusterConfig,
) -> f64 {
    let x_distance_from_edge = x_radius - dx.abs();
    let z_distance_from_edge = z_radius - dz.abs();
    let distance_from_edge = x_distance_from_edge.min(z_distance_from_edge);
    f64::from(clamped_map_f32(
        distance_from_edge as f32,
        0.0,
        config.max_distance_from_edge_affecting_chance_of_speleothem as f32,
        config.chance_of_speleothem_at_max_distance_from_center,
        1.0,
    ))
}

fn clamped_map(value: f64, from_min: f64, from_max: f64, to_min: f64, to_max: f64) -> f64 {
    if value < from_min {
        to_min
    } else if value > from_max {
        to_max
    } else {
        map(value, from_min, from_max, to_min, to_max)
    }
}

fn clamped_map_f32(value: f32, from_min: f32, from_max: f32, to_min: f32, to_max: f32) -> f32 {
    if value < from_min {
        to_min
    } else if value > from_max {
        to_max
    } else {
        to_min + ((value - from_min) / (from_max - from_min)) * (to_max - to_min)
    }
}
