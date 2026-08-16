use crate::registry::REGISTRY;
use crate::registry::blocks::properties::{BoolProperty, Properties};
use crate::registry::blocks::shapes::is_offset_face_full;
use crate::registry::blocks::vanilla_blocks;
use crate::util::direction::Direction;
use crate::util::pos::BlockPos;
use crate::worldgen::feature::features::DIRECTIONS;
use crate::worldgen::region::WorldGenRegion;

pub fn place(region: &WorldGenRegion<'_>, origin: BlockPos) -> bool {
    if !region.get_block(origin).is_air() {
        return false;
    }

    for direction in DIRECTIONS {
        if direction == Direction::Down {
            continue;
        }
        if !can_attach_to(region, origin.relative(direction), direction) {
            continue;
        }
        let vine = REGISTRY
            .blocks
            .get_default_state_id(&vanilla_blocks::VINE)
            .set_value(property_for_face(direction), true);
        region.set_block(origin, vine);
        return true;
    }

    false
}

const fn property_for_face(direction: Direction) -> &'static BoolProperty {
    match direction {
        Direction::Up => &Properties::UP,
        Direction::North => &Properties::NORTH,
        Direction::South => &Properties::SOUTH,
        Direction::West => &Properties::WEST,
        Direction::East => &Properties::EAST,
        Direction::Down => &Properties::DOWN,
    }
}

fn can_attach_to(
    region: &WorldGenRegion<'_>,
    neighbor_pos: BlockPos,
    direction_towards_neighbor: Direction,
) -> bool {
    let neighbor = region.get_block(neighbor_pos);
    let face = direction_towards_neighbor.opposite();
    is_offset_face_full(neighbor.get_support_shape_at(neighbor_pos), face)
        || is_offset_face_full(neighbor.get_collision_shape_at(neighbor_pos), face)
}
