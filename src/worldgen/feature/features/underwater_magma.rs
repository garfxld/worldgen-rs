use serde::Deserialize;

use crate::registry::blocks::shapes;
use crate::registry::blocks::vanilla_blocks;
use crate::util::direction::Direction;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::worldgen::feature::features::blockpos_iter::between_closed;
use crate::worldgen::region::WorldGenRegion;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnderwaterMagmaConfig {
    pub floor_search_range: i32,
    pub placement_radius_around_floor: i32,
    pub placement_probability_per_valid_position: f32,
}

fn is_water(region: &WorldGenRegion<'_>, pos: BlockPos) -> bool {
    region.get_block(pos).get_block() == &vanilla_blocks::WATER
}

fn get_floor_y(region: &WorldGenRegion<'_>, origin: BlockPos, search_range: i32) -> Option<i32> {
    if !is_water(region, origin) {
        return None;
    }

    let mut pos = origin;
    let mut i = 1;
    while i < search_range && is_water(region, pos) {
        pos = pos.below();
        i += 1;
    }

    if is_water(region, pos) { None } else { Some(pos.y()) }
}

fn is_water_or_air(region: &WorldGenRegion<'_>, pos: BlockPos) -> bool {
    let state = region.get_block(pos);
    state.get_block() == &vanilla_blocks::WATER || state.is_air()
}

fn is_visible_from_outside(
    region: &WorldGenRegion<'_>,
    pos: BlockPos,
    covered_direction: Direction,
) -> bool {
    !shapes::is_face_full(region.get_block(pos).get_occlusion_shape(), covered_direction)
}

fn is_valid_placement(region: &WorldGenRegion<'_>, pos: BlockPos) -> bool {
    if is_water_or_air(region, pos)
        || is_visible_from_outside(region, pos.below(), Direction::Up)
    {
        return false;
    }

    for direction in Direction::HORIZONTAL {
        if is_visible_from_outside(region, direction.relative(pos), direction.opposite()) {
            return false;
        }
    }

    true
}

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPos,
    config: &UnderwaterMagmaConfig,
) -> bool {
    let Some(floor_y) = get_floor_y(region, origin, config.floor_search_range) else {
        return false;
    };

    let floor_pos = origin.at_y(floor_y);
    let r = config.placement_radius_around_floor;
    let magma = vanilla_blocks::MAGMA_BLOCK.default_state();
    let mut placed = 0;

    for pos in between_closed(floor_pos.offset(-r, -r, -r), floor_pos.offset(r, r, r)) {
        if random.next_f32() < config.placement_probability_per_valid_position
            && is_valid_placement(region, pos)
        {
            region.set_block(pos, magma);
            placed += 1;
        }
    }

    placed > 0
}
