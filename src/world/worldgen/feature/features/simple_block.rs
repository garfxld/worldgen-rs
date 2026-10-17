use serde::Deserialize;
use shulkr::util::Key;
use shulkr::world::block::Block;

use crate::util::rng::RandomSource;
use crate::world::worldgen::block_survival::{can_survive, is_carpet_base, is_double_plant};
use crate::world::worldgen::feature::features::HORIZONTAL_DIRECTIONS;
use crate::world::worldgen::feature::state_provider::ProviderHolder;
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util::BlockPosition;
use shulkr::util::Direction;
use shulkr::util::is_offset_face_full;
use shulkr::world::block::BlockState;
use shulkr::world::block::{DoubleBlockHalf, EnumProperty, Properties, WallSide};

const PALE_MOSS_CARPET: Key = Key::const_vanilla("pale_moss_carpet");
const WATER: Key = Key::const_vanilla("water");

#[derive(Debug, Deserialize)]
pub struct SimpleBlockConfig {
    pub to_place: ProviderHolder,
    #[serde(default)]
    pub schedule_tick: bool,
}

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPosition,
    config: &SimpleBlockConfig,
) -> bool {
    let Some(state) = config.to_place.get_optional_state(region, random, origin) else {
        return false;
    };

    if !can_survive(region, origin, state) {
        return false;
    }

    let block = state.as_block();
    if is_double_plant(block.key().path()) {
        let above = region.get_block(origin.above());
        if !above.is_air()
            && (state.get_fluid_state() != above.get_fluid_state() || !above.is_replaceable())
        {
            return false;
        }
        place_double_plant(region, state, origin);
    } else if block.key() == &PALE_MOSS_CARPET {
        place_mossy_carpet(region, origin);
    } else {
        region.set_block(origin, state);
    }

    true
}

fn place_double_plant(region: &WorldGenRegion<'_>, state: BlockState, lower_pos: BlockPosition) {
    let upper_pos = lower_pos.above();
    let lower = state.with(Properties::DOUBLE_BLOCK_HALF, DoubleBlockHalf::Lower);
    let upper = state.with(Properties::DOUBLE_BLOCK_HALF, DoubleBlockHalf::Upper);
    region.set_block(lower_pos, copy_waterlogged_from(region, lower_pos, lower));
    region.set_block(upper_pos, copy_waterlogged_from(region, upper_pos, upper));
}

fn copy_waterlogged_from(
    region: &WorldGenRegion<'_>,
    pos: BlockPosition,
    state: BlockState,
) -> BlockState {
    if state.get(Properties::WATERLOGGED).is_none() {
        return state;
    }
    state.with(Properties::WATERLOGGED, is_water_at(region, pos))
}

fn is_water_at(region: &WorldGenRegion<'_>, pos: BlockPosition) -> bool {
    region
        .get_block(pos)
        .get_fluid_state()
        .fluid()
        .has_tag(&WATER)
}

fn place_mossy_carpet(region: &WorldGenRegion<'_>, pos: BlockPosition) {
    let simple = Block::PALE_MOSS_CARPET.default_state();
    let adjusted = updated_carpet_state(region, simple, pos, true);
    region.set_block(pos, adjusted);

    let topper = create_topper_with_side_chance(region, pos);
    if !topper.is_air() {
        region.set_block(pos.above(), topper);
        let bottom = updated_carpet_state(region, adjusted, pos, true);
        region.set_block(pos, bottom);
    }
}

const fn wall_property(direction: Direction) -> EnumProperty<WallSide> {
    match direction {
        Direction::North => Properties::NORTH_WALL,
        Direction::South => Properties::SOUTH_WALL,
        Direction::West => Properties::WEST_WALL,
        _ => Properties::EAST_WALL,
    }
}

fn can_support_at_face(
    region: &WorldGenRegion<'_>,
    pos: BlockPosition,
    direction: Direction,
) -> bool {
    if direction == Direction::Up {
        return false;
    }
    let neighbor_pos = pos.relative(direction);
    let neighbor = region.get_block(neighbor_pos);
    let face = direction.opposite();
    is_offset_face_full(neighbor.get_support_shape_at(neighbor_pos), face)
        || is_offset_face_full(neighbor.get_collision_shape_at(neighbor_pos), face)
}

fn updated_carpet_state(
    region: &WorldGenRegion<'_>,
    state: BlockState,
    pos: BlockPosition,
    create_sides: bool,
) -> BlockState {
    let mut state = state;
    let create_sides = create_sides || is_carpet_base(state);
    let mut above_state = None;
    let mut below_state = None;

    for direction in HORIZONTAL_DIRECTIONS {
        let property = wall_property(direction);
        let mut side = if can_support_at_face(region, pos, direction) {
            if create_sides {
                WallSide::Low
            } else {
                state.try_get(property)
            }
        } else {
            WallSide::None
        };

        if side == WallSide::Low {
            let above = *above_state.get_or_insert_with(|| region.get_block(pos.above()));
            if above.as_block().key() == &PALE_MOSS_CARPET
                && above.try_get(property) != WallSide::None
                && !is_carpet_base(above)
            {
                side = WallSide::Tall;
            }

            if !is_carpet_base(state) {
                let below = *below_state.get_or_insert_with(|| region.get_block(pos.below()));
                if below.as_block().key() == &PALE_MOSS_CARPET
                    && below.try_get(property) == WallSide::None
                {
                    side = WallSide::None;
                }
            }
        }

        state = state.with(property, side);
    }

    state
}

fn create_topper_with_side_chance(region: &WorldGenRegion<'_>, pos: BlockPosition) -> BlockState {
    let air = Block::AIR.default_state();
    let above_pos = pos.above();
    let above_previous = region.get_block(above_pos);
    let is_carpet_above = above_previous.as_block().key() == &PALE_MOSS_CARPET;

    if (is_carpet_above && is_carpet_base(above_previous))
        || (!is_carpet_above && !above_previous.is_replaceable())
    {
        return air;
    }

    let no_base = Block::PALE_MOSS_CARPET
        .default_state()
        .with(Properties::BOTTOM, false);
    let mut above_state = updated_carpet_state(region, no_base, above_pos, true);

    for direction in HORIZONTAL_DIRECTIONS {
        let property = wall_property(direction);
        if above_state.try_get(property) != WallSide::None
            && !region.with_random(RandomSource::next_bool)
        {
            above_state = above_state.with(property, WallSide::None);
        }
    }

    if has_faces(above_state) && above_state != above_previous {
        above_state
    } else {
        air
    }
}

fn has_faces(state: BlockState) -> bool {
    if is_carpet_base(state) {
        return true;
    }
    HORIZONTAL_DIRECTIONS
        .iter()
        .any(|direction| state.try_get(wall_property(*direction)) != WallSide::None)
}
