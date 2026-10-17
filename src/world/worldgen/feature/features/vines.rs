use crate::world::worldgen::feature::features::DIRECTIONS;
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util::BlockPosition;
use shulkr::util::Direction;
use shulkr::util::is_offset_face_full;
use shulkr::world::block::Block;
use shulkr::world::block::{BoolProperty, Properties};

pub fn place(region: &WorldGenRegion<'_>, origin: BlockPosition) -> bool {
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
        let vine = Block::VINE
            .default_state()
            .with(property_for_face(direction), true);
        region.set_block(origin, vine);
        return true;
    }

    false
}

const fn property_for_face(direction: Direction) -> BoolProperty {
    match direction {
        Direction::Up => Properties::UP,
        Direction::North => Properties::NORTH,
        Direction::South => Properties::SOUTH,
        Direction::West => Properties::WEST,
        Direction::East => Properties::EAST,
        Direction::Down => Properties::DOWN,
    }
}

fn can_attach_to(
    region: &WorldGenRegion<'_>,
    neighbor_pos: BlockPosition,
    direction_towards_neighbor: Direction,
) -> bool {
    let neighbor = region.get_block(neighbor_pos);
    let face = direction_towards_neighbor.opposite();
    is_offset_face_full(neighbor.get_support_shape_at(neighbor_pos), face)
        || is_offset_face_full(neighbor.get_collision_shape_at(neighbor_pos), face)
}
