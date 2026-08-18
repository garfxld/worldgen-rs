use serde::Deserialize;

use crate::registry::blocks::properties::Properties;
use crate::util::direction::Direction;
use crate::util::hash_set::JavaHashSet;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::util::value_providers::int::IntProvider;
use crate::worldgen::feature::features::mark_above_for_post_processing;
use crate::worldgen::feature::features::tree;
use crate::worldgen::feature::state_provider::BlockStateProvider;
use crate::worldgen::feature::treedecorators::{TreeDecorator, TreeDecoratorContext};
use crate::worldgen::region::WorldGenRegion;

#[derive(Debug, Deserialize)]
pub struct FallenTreeConfig {
    pub trunk_provider: BlockStateProvider,
    pub log_length: IntProvider,
    pub stump_decorators: Vec<TreeDecorator>,
    pub log_decorators: Vec<TreeDecorator>,
}

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPos,
    config: &FallenTreeConfig,
) -> bool {
    place_stump(region, random, origin, config);

    let direction = Direction::HORIZONTAL[random.next_i32_bounded(4) as usize];
    let log_length = config.log_length.sample(random) - 2;
    let log_start = set_ground_height(
        region,
        origin.relative_n(direction, 2 + random.next_i32_bounded(2)),
    );

    if can_place_entire_fallen_log(region, log_length, log_start, direction) {
        place_fallen_log(region, random, log_length, log_start, direction, config);
    }
    true
}

fn set_ground_height(region: &WorldGenRegion<'_>, pos: BlockPos) -> BlockPos {
    let mut pos = pos.above();
    for _ in 0..6 {
        if may_place_on(region, pos) {
            return pos;
        }
        pos = pos.below();
    }
    pos
}

fn may_place_on(region: &WorldGenRegion<'_>, pos: BlockPos) -> bool {
    tree::valid_tree_pos(region, pos) && is_over_solid_ground(region, pos)
}

fn is_over_solid_ground(region: &WorldGenRegion<'_>, pos: BlockPos) -> bool {
    region
        .get_block(pos.below())
        .is_face_sturdy_at(pos, Direction::Up)
}

fn can_place_entire_fallen_log(
    region: &WorldGenRegion<'_>,
    log_length: i32,
    log_start: BlockPos,
    direction: Direction,
) -> bool {
    let mut pos = log_start;
    let mut gap_in_ground = 0;
    for _ in 0..log_length {
        if !tree::valid_tree_pos(region, pos) {
            return false;
        }
        if is_over_solid_ground(region, pos) {
            gap_in_ground = 0;
        } else {
            gap_in_ground += 1;
            if gap_in_ground > 2 {
                return false;
            }
        }
        pos = pos.relative(direction);
    }
    true
}

fn place_log_block(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    pos: BlockPos,
    config: &FallenTreeConfig,
    axis: Option<Direction>,
) {
    let mut state = config.trunk_provider.get_state(region, random, pos);
    if let Some(direction) = axis
        && state.try_get_value(&Properties::AXIS).is_some()
    {
        state = state.set_value(&Properties::AXIS, direction.axis());
    }
    region.set_block(pos, state);
    mark_above_for_post_processing(region, pos);
}

fn place_stump(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPos,
    config: &FallenTreeConfig,
) {
    place_log_block(region, random, origin, config, None);
    let stump: JavaHashSet = std::iter::once(origin).collect();
    decorate(region, random, &stump, &config.stump_decorators);
}

fn place_fallen_log(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    log_length: i32,
    log_start: BlockPos,
    direction: Direction,
    config: &FallenTreeConfig,
) {
    let mut pos = log_start;
    let mut fallen_log = JavaHashSet::new();
    for _ in 0..log_length {
        place_log_block(region, random, pos, config, Some(direction));
        fallen_log.insert(pos);
        pos = pos.relative(direction);
    }
    decorate(region, random, &fallen_log, &config.log_decorators);
}

fn decorate(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    logs: &JavaHashSet,
    decorators: &[TreeDecorator],
) {
    if decorators.is_empty() {
        return;
    }
    let empty = JavaHashSet::new();
    let mut decorations = JavaHashSet::new();
    let mut context =
        TreeDecoratorContext::new(region, random, logs, &empty, &empty, &mut decorations);
    for decorator in decorators {
        decorator.place(&mut context);
    }
}
