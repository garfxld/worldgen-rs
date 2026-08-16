use crate::registry::REGISTRY;
use crate::registry::blocks::properties::Properties;
use crate::registry::blocks::vanilla_blocks;
use crate::util::direction::Direction;
use crate::util::ident::{BlockState, Identifier};
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::worldgen::region::WorldGenRegion;

const SUPPORTS_CHORUS_PLANT: Identifier = Identifier::const_vanilla("supports_chorus_plant");

fn all_neighbors_empty(
    region: &WorldGenRegion<'_>,
    pos: BlockPos,
    ignore: Option<Direction>,
) -> bool {
    for direction in Direction::HORIZONTAL {
        if Some(direction) != ignore && !region.get_block(direction.relative(pos)).is_air() {
            return false;
        }
    }
    true
}

fn is_plant_or_flower(region: &WorldGenRegion<'_>, pos: BlockPos) -> bool {
    let block = region.get_block(pos).get_block();
    block == &vanilla_blocks::CHORUS_PLANT || block == &vanilla_blocks::CHORUS_FLOWER
}

fn supports_chorus_plant(region: &WorldGenRegion<'_>, pos: BlockPos) -> bool {
    REGISTRY
        .blocks
        .is_in_tag(&region.get_block(pos).get_block().key, &SUPPORTS_CHORUS_PLANT)
}

fn state_with_connections(region: &WorldGenRegion<'_>, pos: BlockPos) -> BlockState {
    let down = pos.below();
    let down_supported = is_plant_or_flower(region, down) || supports_chorus_plant(region, down);

    vanilla_blocks::CHORUS_PLANT
        .default_state()
        .set_value(&Properties::DOWN, down_supported)
        .set_value(&Properties::UP, is_plant_or_flower(region, pos.above()))
        .set_value(&Properties::NORTH, is_plant_or_flower(region, pos.north()))
        .set_value(&Properties::EAST, is_plant_or_flower(region, pos.east()))
        .set_value(&Properties::SOUTH, is_plant_or_flower(region, pos.south()))
        .set_value(&Properties::WEST, is_plant_or_flower(region, pos.west()))
}

fn grow_tree_recursive(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    current: BlockPos,
    start_pos: BlockPos,
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
            let target = direction.relative(current.above_n(height));
            let below = target.below();
            if (target.x() - start_pos.x()).abs() < max_horizontal_spread
                && (target.z() - start_pos.z()).abs() < max_horizontal_spread
                && region.get_block(target).is_air()
                && region.get_block(below).is_air()
                && all_neighbors_empty(region, target, Some(direction.opposite()))
            {
                placed_stem = true;
                region.set_block(target, state_with_connections(region, target));
                let opposite_pos = direction.opposite().relative(target);
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
            vanilla_blocks::CHORUS_FLOWER
                .default_state()
                .set_value(&Properties::AGE_5, 5),
        );
    }
}

pub fn place(region: &WorldGenRegion<'_>, random: &mut RandomSource, origin: BlockPos) -> bool {
    if region.get_block(origin).is_air() && supports_chorus_plant(region, origin.below()) {
        region.set_block(origin, state_with_connections(region, origin));
        grow_tree_recursive(region, random, origin, origin, 8, 0);
        true
    } else {
        false
    }
}
