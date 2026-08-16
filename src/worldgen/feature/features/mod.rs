pub mod bamboo;
pub mod basalt_columns;
pub mod basalt_pillar;
pub mod block_blob;
pub mod block_column;
pub mod block_pile;
pub mod blockpos_iter;
pub mod blue_ice;
pub mod bonus_chest;
pub mod chorus_plant;
pub mod coral;
pub mod coral_claw;
pub mod coral_mushroom;
pub mod coral_tree;
pub mod delta_feature;
pub mod desert_well;
pub mod disk;
pub mod end_gateway;
pub mod end_island;
pub mod end_platform;
pub mod end_spike;
pub mod fallen_tree;
pub mod fossil;
pub mod freeze_top_layer;
pub mod geode;
pub mod glowstone_blob;
pub mod huge_fungus;
pub mod huge_mushroom;
pub mod iceberg;
pub mod kelp;
pub mod lake;
pub mod large_dripstone;
pub mod monster_room;
pub mod multiface_growth;
pub mod nether_forest_vegetation;
pub mod netherrack_replace_blobs;
pub mod ore;
pub mod root_system;
pub mod scattered_ore;
pub mod sculk_patch;
pub mod sea_pickle;
pub mod seagrass;
pub mod simple_block;
pub mod speleothem;
pub mod speleothem_cluster;
pub mod spike;
pub mod spring;
pub mod template;
pub mod tree;
pub mod twisting_vines;
pub mod underwater_magma;
pub mod vegetation_patch;
pub mod vines;
pub mod void_start_platform;
pub mod weeping_vines;

use crate::util::direction::Direction;
use crate::util::ident::BlockState;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::worldgen::region::WorldGenRegion;

pub const DIRECTIONS: [Direction; 6] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];

pub const HORIZONTAL_DIRECTIONS: [Direction; 4] = Direction::HORIZONTAL;

fn check_neighbors(
    region: &WorldGenRegion<'_>,
    pos: BlockPos,
    predicate: impl Fn(BlockState) -> bool,
) -> bool {
    DIRECTIONS
        .iter()
        .any(|direction| predicate(region.get_block(direction.relative(pos))))
}

pub fn is_adjacent_to_air(region: &WorldGenRegion<'_>, pos: BlockPos) -> bool {
    check_neighbors(region, pos, |state| state.is_air())
}

pub fn should_skip_air_check(
    random: &mut RandomSource,
    discard_chance_on_air_exposure: f32,
) -> bool {
    if discard_chance_on_air_exposure <= 0.0 {
        return true;
    }
    if discard_chance_on_air_exposure >= 1.0 {
        return false;
    }
    random.next_f32() >= discard_chance_on_air_exposure
}

pub fn mark_above_for_post_processing(region: &WorldGenRegion<'_>, place_pos: BlockPos) {
    let mut pos = place_pos;
    for _ in 0..2 {
        pos = pos.above();
        if region.get_block(pos).is_air() {
            return;
        }
        region.mark_pos_for_postprocessing(pos);
    }
}
