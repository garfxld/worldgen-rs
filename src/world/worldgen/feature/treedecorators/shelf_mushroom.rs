use serde::Deserialize;
use shulkr::util::{BlockPosition, Direction};
use shulkr::world::block::{Block, Properties};

use crate::util::rng::RandomSource;
use crate::world::worldgen::feature::treedecorators::TreeDecoratorContext;

#[derive(Debug, Deserialize)]
pub struct ShelfMushroomDecorator {
    pub probability: f32,
}

impl ShelfMushroomDecorator {
    pub fn place(&self, context: &mut TreeDecoratorContext<'_, '_>) {
        if context.random.next_f32() >= self.probability {
            return;
        }
        if context.logs.is_empty() {
            return;
        }
        let logs = context.logs.clone();
        if is_fallen_log(&logs) {
            place_on_fallen_log(context, &logs);
        } else {
            place_on_standing_tree(context, &logs);
        }
    }
}

fn place_on_standing_tree(context: &mut TreeDecoratorContext<'_, '_>, logs: &[BlockPosition]) {
    let first = Direction::HORIZONTAL[context.random.next_i32_bounded(4) as usize];
    let directions = [first, first.rotate_y_clockwise()];
    let tree_base_y = logs[0].y();
    for &log_pos in logs {
        let dy = log_pos.y() - tree_base_y;
        if !(1..=4).contains(&dy) {
            continue;
        }
        for facing in directions {
            if context.random.next_f32() <= 0.25
                && try_place_on_standing_tree(context, log_pos, facing)
            {
                break;
            }
        }
    }
}

fn place_on_fallen_log(context: &mut TreeDecoratorContext<'_, '_>, logs: &[BlockPosition]) {
    let directions = perpendicular_to_fallen_log(logs);
    for &log_pos in logs {
        for facing in directions {
            if context.random.next_f32() <= 0.25 {
                try_place_on_fallen_tree(context, log_pos, facing);
            }
        }
    }
}

fn try_place_on_standing_tree(
    context: &mut TreeDecoratorContext<'_, '_>,
    log_pos: BlockPosition,
    facing: Direction,
) -> bool {
    let mushroom_pos = mushroom_pos_for(log_pos, facing);
    if !is_block_replaceable_with_shelf_mushroom(context, mushroom_pos) {
        return false;
    }
    if has_shelf_mushroom_at(context, mushroom_pos.below()) {
        return false;
    }
    place_mushroom(context, mushroom_pos, facing);
    true
}

fn try_place_on_fallen_tree(
    context: &mut TreeDecoratorContext<'_, '_>,
    log_pos: BlockPosition,
    facing: Direction,
) {
    let mushroom_pos = mushroom_pos_for(log_pos, facing);
    if is_block_replaceable_with_shelf_mushroom(context, mushroom_pos)
        && !has_horizontally_adjacent_shelf_mushroom(context, mushroom_pos)
        && !has_horizontally_adjacent_shelf_mushroom(context, log_pos)
    {
        place_mushroom(context, mushroom_pos, facing);
    }
}

const fn is_fallen_log(logs: &[BlockPosition]) -> bool {
    logs[0].y() == logs[logs.len() - 1].y()
}

const fn perpendicular_to_fallen_log(logs: &[BlockPosition]) -> [Direction; 2] {
    let first = logs[0];
    let last = logs[logs.len() - 1];
    if first.x() != last.x() {
        [Direction::North, Direction::South]
    } else {
        [Direction::East, Direction::West]
    }
}

fn mushroom_pos_for(log_pos: BlockPosition, facing: Direction) -> BlockPosition {
    log_pos.relative(facing)
}

fn place_mushroom(
    context: &mut TreeDecoratorContext<'_, '_>,
    pos: BlockPosition,
    facing: Direction,
) {
    let age = next_age(context.random);
    let state = Block::SHELF_MUSHROOM
        .default_state()
        .with(Properties::AGE_1, age)
        .with(Properties::HORIZONTAL_FACING, facing);
    context.set_block(pos, state);
}

fn next_age(random: &mut RandomSource) -> u8 {
    random.next_i32_bounded(2) as u8
}

fn has_shelf_mushroom_at(context: &TreeDecoratorContext<'_, '_>, pos: BlockPosition) -> bool {
    context.region.get_block(pos).is(Block::SHELF_MUSHROOM)
}

fn has_horizontally_adjacent_shelf_mushroom(
    context: &TreeDecoratorContext<'_, '_>,
    pos: BlockPosition,
) -> bool {
    Direction::HORIZONTAL
        .into_iter()
        .any(|direction| has_shelf_mushroom_at(context, pos.relative(direction)))
}

fn is_block_replaceable_with_shelf_mushroom(
    context: &TreeDecoratorContext<'_, '_>,
    pos: BlockPosition,
) -> bool {
    context.is_replaceable(pos) && !context.is_water_or_water_nearby(pos)
}
