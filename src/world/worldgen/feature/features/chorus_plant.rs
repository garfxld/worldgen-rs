use shulkr::util::Key;
use shulkr::world::block::{Block, BlockState, Properties};

use shulkr::util::Direction;

use crate::util::rng::RandomSource;
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util::BlockPosition;

const SUPPORTS_CHORUS_PLANT: Key = Key::const_vanilla("supports_chorus_plant");

fn all_neighbors_empty(
    region: &WorldGenRegion<'_>,
    pos: BlockPosition,
    ignore: Option<Direction>,
) -> bool {
    for direction in Direction::HORIZONTAL {
        if Some(direction) != ignore && !region.get_block(pos.relative(direction)).is_air() {
            return false;
        }
    }
    true
}

fn is_plant_or_flower(region: &WorldGenRegion<'_>, pos: BlockPosition) -> bool {
    let block = region.get_block(pos).as_block();
    matches!(block, Block::CHORUS_PLANT | Block::CHORUS_FLOWER)
}

fn supports_chorus_plant(region: &WorldGenRegion<'_>, pos: BlockPosition) -> bool {
    region
        .get_block(pos)
        .as_block()
        .has_tag(&SUPPORTS_CHORUS_PLANT)
}

fn state_with_connections(region: &WorldGenRegion<'_>, pos: BlockPosition) -> BlockState {
    let down = pos.below();
    let down_supported = is_plant_or_flower(region, down) || supports_chorus_plant(region, down);

    Block::CHORUS_PLANT
        .default_state()
        .with(Properties::DOWN, down_supported)
        .with(Properties::UP, is_plant_or_flower(region, pos.above()))
        .with(Properties::NORTH, is_plant_or_flower(region, pos.north()))
        .with(Properties::EAST, is_plant_or_flower(region, pos.east()))
        .with(Properties::SOUTH, is_plant_or_flower(region, pos.south()))
        .with(Properties::WEST, is_plant_or_flower(region, pos.west()))
}

fn grow_tree_recursive(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    current: BlockPosition,
    start_pos: BlockPosition,
    max_horizontal_spread: i32,
    depth: i32,
) {
    let mut height = random.next_i32_bounded(4) + 1;
    if depth == 0 {
        height += 1;
    }

    for i in 0..height {
        let target = current.above_n(i + 1);
        if !all_neighbors_empty(region, target, None) {
            return;
        }

        region.set_block(target, state_with_connections(region, target));
        let below = target.below();
        region.set_block(below, state_with_connections(region, below));
    }

    let mut placed_stem = false;
    if depth < 4 {
        let mut stems = random.next_i32_bounded(4);
        if depth == 0 {
            stems += 1;
        }

        for _ in 0..stems {
            let direction = Direction::HORIZONTAL[random.next_i32_bounded(4) as usize];
            let target = current.above_n(height).relative(direction);
            let below = target.below();
            if (target.x() - start_pos.x()).abs() < max_horizontal_spread
                && (target.z() - start_pos.z()).abs() < max_horizontal_spread
                && region.get_block(target).is_air()
                && region.get_block(below).is_air()
                && all_neighbors_empty(region, target, Some(direction.opposite()))
            {
                placed_stem = true;
                region.set_block(target, state_with_connections(region, target));
                let opposite_pos = target.relative(direction.opposite());
                region.set_block(opposite_pos, state_with_connections(region, opposite_pos));
                grow_tree_recursive(
                    region,
                    random,
                    target,
                    start_pos,
                    max_horizontal_spread,
                    depth + 1,
                );
            }
        }
    }

    if !placed_stem {
        region.set_block(
            current.above_n(height),
            Block::CHORUS_FLOWER
                .default_state()
                .with(Properties::AGE_5, 5),
        );
    }
}

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPosition,
) -> bool {
    if region.get_block(origin).is_air() && supports_chorus_plant(region, origin.below()) {
        region.set_block(origin, state_with_connections(region, origin));
        grow_tree_recursive(region, random, origin, origin, 8, 0);
        true
    } else {
        false
    }
}
