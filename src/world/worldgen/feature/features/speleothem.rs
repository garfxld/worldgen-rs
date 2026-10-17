use std::f32::consts::TAU;

use serde::Deserialize;

use crate::registry::fluid::is_water_fluid;
use crate::util::math;
use crate::util::rng::RandomSource;
use crate::world::worldgen::block_predicate::HolderSet;
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util::BlockPosition;
use shulkr::util::Direction;
use shulkr::world::block::Block;
use shulkr::world::block::BlockState;
use shulkr::world::block::{Properties, SpeleothemThickness};

pub fn is_empty_or_water(state: BlockState) -> bool {
    state.is_air() || state.is(Block::WATER)
}

pub fn is_neither_empty_nor_water(state: BlockState) -> bool {
    !state.is_air() && !state.is(Block::WATER)
}

pub fn is_base(state: BlockState, base_block: Block, replaceable_blocks: &HolderSet) -> bool {
    state.as_block() == base_block || replaceable_blocks.contains_block(state)
}

pub fn is_base_or_lava(
    state: BlockState,
    base_block: Block,
    replaceable_blocks: &HolderSet,
) -> bool {
    is_base(state, base_block, replaceable_blocks) || state.is(Block::LAVA)
}

pub fn is_empty_or_water_or_lava(state: BlockState) -> bool {
    state.is_air() || state.is(Block::WATER) || state.is(Block::LAVA)
}

pub fn get_speleothem_height(
    mut xz_distance_from_center: f64,
    speleothem_radius: f64,
    scale: f64,
    bluntness: f64,
) -> f64 {
    if xz_distance_from_center < bluntness {
        xz_distance_from_center = bluntness;
    }

    const CUTOFF: f64 = 0.384;

    let r = xz_distance_from_center / speleothem_radius * CUTOFF;
    let part1 = 0.75 * r.powf(1.333_333_333_333_333_3);
    let part2 = r.powf(0.666_666_666_666_666_6);
    let part3 = 0.333_333_333_333_333_3 * r.ln();
    let height_relative_to_max_radius = (scale * (part1 - part2 - part3)).max(0.0);

    height_relative_to_max_radius / CUTOFF * speleothem_radius
}

pub fn is_circle_mostly_embedded_in_stone(
    region: &WorldGenRegion<'_>,
    center: BlockPosition,
    xz_radius: i32,
) -> bool {
    if is_empty_or_water_or_lava(region.get_block(center)) {
        return false;
    }

    let angle_increment = 6.0 / xz_radius as f32;
    let mut angle = 0.0;
    while angle < TAU {
        let dx = (math::cos(f64::from(angle)) * xz_radius as f32) as i32;
        let dz = (math::sin(f64::from(angle)) * xz_radius as f32) as i32;
        if is_empty_or_water_or_lava(region.get_block(center.offset(dx, 0, dz))) {
            return false;
        }
        angle += angle_increment;
    }

    true
}

fn create_pointed_block(
    direction: Direction,
    thickness: SpeleothemThickness,
    pointed_block: Block,
) -> BlockState {
    pointed_block
        .default_state()
        .with(Properties::VERTICAL_DIRECTION, direction)
        .with(Properties::SPELEOTHEM_THICKNESS, thickness)
}

pub fn build_base_to_tip_column(
    direction: Direction,
    total_length: i32,
    merged_tip: bool,
    pointed_block: Block,
    mut consumer: impl FnMut(BlockState),
) {
    if total_length >= 3 {
        consumer(create_pointed_block(
            direction,
            SpeleothemThickness::Base,
            pointed_block,
        ));

        for _ in 0..total_length - 3 {
            consumer(create_pointed_block(
                direction,
                SpeleothemThickness::Middle,
                pointed_block,
            ));
        }
    }

    if total_length >= 2 {
        consumer(create_pointed_block(
            direction,
            SpeleothemThickness::Frustum,
            pointed_block,
        ));
    }

    if total_length >= 1 {
        consumer(create_pointed_block(
            direction,
            if merged_tip {
                SpeleothemThickness::TipMerge
            } else {
                SpeleothemThickness::Tip
            },
            pointed_block,
        ));
    }
}

#[expect(clippy::too_many_arguments, reason = "vanilla parameter list")]
pub fn grow_speleothem(
    region: &WorldGenRegion<'_>,
    start_pos: BlockPosition,
    tip_direction: Direction,
    height: i32,
    merged_tip: bool,
    base_block: Block,
    pointed_block: Block,
    replaceable_blocks: &HolderSet,
) {
    let check_pos = start_pos.relative(tip_direction.opposite());
    if !is_base(region.get_block(check_pos), base_block, replaceable_blocks) {
        return;
    }

    let mut pos = start_pos;
    build_base_to_tip_column(tip_direction, height, merged_tip, pointed_block, |state| {
        let state = if state.as_block() == pointed_block {
            let waterlogged = is_water_fluid(region.get_block(pos).get_fluid_state().fluid());
            state.with(Properties::WATERLOGGED, waterlogged)
        } else {
            state
        };

        region.set_block(pos, state);
        pos = pos.relative(tip_direction);
    });
}

pub fn place_base_block_if_possible(
    region: &WorldGenRegion<'_>,
    pos: BlockPosition,
    base_block: Block,
    replaceable_blocks: &HolderSet,
) -> bool {
    if replaceable_blocks.contains_block(region.get_block(pos)) {
        region.set_block(pos, base_block.default_state());
        true
    } else {
        false
    }
}

#[derive(Debug, Deserialize)]
pub struct SpeleothemConfig {
    pub base_block: BlockState,
    pub pointed_block: BlockState,
    pub replaceable_blocks: HolderSet,
    #[serde(default = "default_chance_of_taller_generation")]
    pub chance_of_taller_generation: f32,
    #[serde(default = "default_chance_of_directional_spread")]
    pub chance_of_directional_spread: f32,
    #[serde(default = "default_chance_of_spread_radius2")]
    pub chance_of_spread_radius2: f32,
    #[serde(default = "default_chance_of_spread_radius3")]
    pub chance_of_spread_radius3: f32,
}

const fn default_chance_of_taller_generation() -> f32 {
    0.2
}

const fn default_chance_of_directional_spread() -> f32 {
    0.7
}

const fn default_chance_of_spread_radius2() -> f32 {
    0.5
}

const fn default_chance_of_spread_radius3() -> f32 {
    0.5
}

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPosition,
    config: &SpeleothemConfig,
) -> bool {
    let base_block = config.base_block.as_block();
    let pointed_block = config.pointed_block.as_block();

    let Some(tip_direction) = get_tip_direction(region, origin, random, base_block, config) else {
        return false;
    };

    let root_pos = origin.relative(tip_direction.opposite());
    create_patch_of_base_blocks(region, random, root_pos, base_block, config);

    let height = if random.next_f32() < config.chance_of_taller_generation
        && is_empty_or_water(region.get_block(origin.relative(tip_direction)))
    {
        2
    } else {
        1
    };
    grow_speleothem(
        region,
        origin,
        tip_direction,
        height,
        false,
        base_block,
        pointed_block,
        &config.replaceable_blocks,
    );
    true
}

fn get_tip_direction(
    region: &WorldGenRegion<'_>,
    pos: BlockPosition,
    random: &mut RandomSource,
    base_block: Block,
    config: &SpeleothemConfig,
) -> Option<Direction> {
    let can_place_above = is_base(
        region.get_block(pos.above()),
        base_block,
        &config.replaceable_blocks,
    );
    let can_place_below = is_base(
        region.get_block(pos.below()),
        base_block,
        &config.replaceable_blocks,
    );

    if can_place_above && can_place_below {
        Some(if random.next_bool() {
            Direction::Down
        } else {
            Direction::Up
        })
    } else if can_place_above {
        Some(Direction::Down)
    } else if can_place_below {
        Some(Direction::Up)
    } else {
        None
    }
}

fn create_patch_of_base_blocks(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    pos: BlockPosition,
    base_block: Block,
    config: &SpeleothemConfig,
) {
    place_base_block_if_possible(region, pos, base_block, &config.replaceable_blocks);

    for direction in Direction::HORIZONTAL {
        if random.next_f32() > config.chance_of_directional_spread {
            continue;
        }
        let pos1 = pos.relative(direction);
        place_base_block_if_possible(region, pos1, base_block, &config.replaceable_blocks);
        if random.next_f32() > config.chance_of_spread_radius2 {
            continue;
        }
        let pos2 = pos1.relative(random_direction(random));
        place_base_block_if_possible(region, pos2, base_block, &config.replaceable_blocks);
        if random.next_f32() > config.chance_of_spread_radius3 {
            continue;
        }
        let pos3 = pos2.relative(random_direction(random));
        place_base_block_if_possible(region, pos3, base_block, &config.replaceable_blocks);
    }
}

fn random_direction(random: &mut RandomSource) -> Direction {
    Direction::ALL[random.next_i32_bounded(Direction::ALL.len() as i32) as usize]
}
