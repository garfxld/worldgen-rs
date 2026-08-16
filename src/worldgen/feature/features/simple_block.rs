use serde::Deserialize;

use crate::registry::REGISTRY;
use crate::registry::blocks::properties::{DoubleBlockHalf, EnumProperty, Properties, WallSide};
use crate::registry::blocks::shapes::is_offset_face_full;
use crate::registry::blocks::vanilla_blocks;
use crate::util::direction::Direction;
use crate::util::ident::{BlockState, Identifier};
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::worldgen::block_survival::{can_survive, is_carpet_base, is_double_plant};
use crate::worldgen::feature::state_provider::BlockStateProvider;
use crate::worldgen::feature::features::HORIZONTAL_DIRECTIONS;
use crate::worldgen::region::WorldGenRegion;

const PALE_MOSS_CARPET: Identifier = Identifier::const_vanilla("pale_moss_carpet");
const WATER: Identifier = Identifier::const_vanilla("water");

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SimpleBlockConfig {
    pub to_place: BlockStateProvider,
    #[serde(default)]
    pub schedule_tick: bool,
}

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPos,
    config: &SimpleBlockConfig,
) -> bool {
    let Some(state) = config.to_place.get_optional_state(region, random, origin) else {
        return false;
    };

    if !can_survive(region, origin, state) {
        return false;
    }

    let block = state.get_block();
    if is_double_plant(block.key.path.as_ref()) {
        if !region.get_block(origin.above()).is_air() {
            return false;
        }
        place_double_plant(region, state, origin);
    } else if block.key == PALE_MOSS_CARPET {
        place_mossy_carpet(region, origin);
    } else {
        region.set_block(origin, state);
    }

    true
}

fn place_double_plant(region: &WorldGenRegion<'_>, state: BlockState, lower_pos: BlockPos) {
    let upper_pos = lower_pos.above();
    let lower = state.set_value(&Properties::DOUBLE_BLOCK_HALF, DoubleBlockHalf::Lower);
    let upper = state.set_value(&Properties::DOUBLE_BLOCK_HALF, DoubleBlockHalf::Upper);
    region.set_block(lower_pos, copy_waterlogged_from(region, lower_pos, lower));
    region.set_block(upper_pos, copy_waterlogged_from(region, upper_pos, upper));
}

fn copy_waterlogged_from(
    region: &WorldGenRegion<'_>,
    pos: BlockPos,
    state: BlockState,
) -> BlockState {
    if state.try_get_value(&Properties::WATERLOGGED).is_none() {
        return state;
    }
    state.set_value(&Properties::WATERLOGGED, is_water_at(region, pos))
}

fn is_water_at(region: &WorldGenRegion<'_>, pos: BlockPos) -> bool {
    REGISTRY.fluids.is_in_tag(
        &region.get_block(pos).get_fluid_state().fluid_id.key,
        &WATER,
    )
}

fn place_mossy_carpet(region: &WorldGenRegion<'_>, pos: BlockPos) {
    let simple = REGISTRY
        .blocks
        .get_default_state_id(&vanilla_blocks::PALE_MOSS_CARPET);
    let adjusted = updated_carpet_state(region, simple, pos, true);
    region.set_block(pos, adjusted);

    let topper = create_topper_with_side_chance(region, pos);
    if !topper.is_air() {
        region.set_block(pos.above(), topper);
        let bottom = updated_carpet_state(region, adjusted, pos, true);
        region.set_block(pos, bottom);
    }
}

const fn wall_property(direction: Direction) -> &'static EnumProperty<WallSide> {
    match direction {
        Direction::North => &Properties::NORTH_WALL,
        Direction::South => &Properties::SOUTH_WALL,
        Direction::West => &Properties::WEST_WALL,
        _ => &Properties::EAST_WALL,
    }
}

fn can_support_at_face(region: &WorldGenRegion<'_>, pos: BlockPos, direction: Direction) -> bool {
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
    pos: BlockPos,
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
                state.get_value(property)
            }
        } else {
            WallSide::None
        };

        if side == WallSide::Low {
            let above = *above_state.get_or_insert_with(|| region.get_block(pos.above()));
            if above.get_block().key == PALE_MOSS_CARPET
                && above.get_value(property) != WallSide::None
                && !is_carpet_base(above)
            {
                side = WallSide::Tall;
            }

            if !is_carpet_base(state) {
                let below = *below_state.get_or_insert_with(|| region.get_block(pos.below()));
                if below.get_block().key == PALE_MOSS_CARPET
                    && below.get_value(property) == WallSide::None
                {
                    side = WallSide::None;
                }
            }
        }

        state = state.set_value(property, side);
    }

    state
}

fn create_topper_with_side_chance(region: &WorldGenRegion<'_>, pos: BlockPos) -> BlockState {
    let air = REGISTRY.blocks.get_default_state_id(&vanilla_blocks::AIR);
    let above_pos = pos.above();
    let above_previous = region.get_block(above_pos);
    let is_carpet_above = above_previous.get_block().key == PALE_MOSS_CARPET;

    if (is_carpet_above && is_carpet_base(above_previous))
        || (!is_carpet_above && !above_previous.is_replaceable())
    {
        return air;
    }

    let no_base = REGISTRY
        .blocks
        .get_default_state_id(&vanilla_blocks::PALE_MOSS_CARPET)
        .set_value(&Properties::BOTTOM, false);
    let mut above_state = updated_carpet_state(region, no_base, above_pos, true);

    for direction in HORIZONTAL_DIRECTIONS {
        let property = wall_property(direction);
        if above_state.get_value(property) != WallSide::None
            && !region.with_random(RandomSource::next_bool)
        {
            above_state = above_state.set_value(property, WallSide::None);
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
        .any(|direction| state.get_value(wall_property(*direction)) != WallSide::None)
}
