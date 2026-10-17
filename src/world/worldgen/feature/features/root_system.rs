use crate::util::rng::RandomSource;
use crate::world::chunk::heightmap::HeightmapType;
use crate::world::worldgen::block_predicate::{BlockPredicate, HolderSet};
use crate::world::worldgen::block_survival;
use crate::world::worldgen::feature::PlacedFeature;
use crate::world::worldgen::feature::state_provider::ProviderHolder;
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util::BlockPosition;
use shulkr::util::Direction;
use shulkr::util::Key;
use shulkr::world::block::BlockState;

const WATER: Key = Key::const_vanilla("water");
const LAVA: Key = Key::const_vanilla("lava");

#[derive(Debug)]
pub struct RootSystemConfig {
    pub feature: &'static PlacedFeature,
    pub required_vertical_space_for_tree: i32,
    pub level_test_distance: i32,
    pub max_level_deviation: i32,
    pub root_radius: i32,
    pub root_replaceable: HolderSet,
    pub root_state_provider: ProviderHolder,
    pub root_placement_attempts: i32,
    pub root_column_max_height: i32,
    pub hanging_root_radius: i32,
    pub hanging_roots_vertical_span: i32,
    pub hanging_root_state_provider: ProviderHolder,
    pub hanging_root_placement_attempts: i32,
    pub allowed_vertical_water_for_tree: i32,
    pub allowed_tree_position: BlockPredicate,
}

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPosition,
    config: &RootSystemConfig,
) -> bool {
    if !region.get_block(origin).is_air() {
        return false;
    }

    if place_dirt_and_tree(region, config, random, origin) {
        place_roots(region, config, random, origin);
    }

    true
}

fn is_allowed_tree_space(
    state: BlockState,
    blocks_above_origin: i32,
    allowed_vertical_water_height: i32,
) -> bool {
    if state.is_air() {
        return true;
    }

    let blocks_above_ground = blocks_above_origin + 1;
    blocks_above_ground <= allowed_vertical_water_height
        && state.get_fluid_state().fluid().has_tag(&WATER)
}

fn space_for_tree(
    region: &WorldGenRegion<'_>,
    config: &RootSystemConfig,
    pos: BlockPosition,
) -> bool {
    let mut column_up_pos = pos;
    for i in 1..=config.required_vertical_space_for_tree {
        column_up_pos = column_up_pos.above();
        let state = region.get_block(column_up_pos);
        if !is_allowed_tree_space(state, i, config.allowed_vertical_water_for_tree) {
            return false;
        }
    }

    if config.level_test_distance > 0 {
        for direction in Direction::HORIZONTAL {
            let corner_pos = pos.relative_n(direction, config.level_test_distance);
            let below = region.get_block(corner_pos.below_n(config.max_level_deviation));
            let above = region.get_block(corner_pos.above_n(config.max_level_deviation));
            if below.is_air() || !above.is_air() {
                return false;
            }
        }
    }

    true
}

fn place_dirt_and_tree(
    region: &WorldGenRegion<'_>,
    config: &RootSystemConfig,
    random: &mut RandomSource,
    origin: BlockPosition,
) -> bool {
    let mut working_pos = origin;
    for y in 0..config.root_column_max_height {
        working_pos = working_pos.above();
        if region.get_height(
            HeightmapType::WorldSurface,
            working_pos.x(),
            working_pos.z(),
        ) < working_pos.y()
        {
            return false;
        }

        if config.allowed_tree_position.test(region, working_pos)
            && space_for_tree(region, config, working_pos)
        {
            let below_pos = working_pos.below();
            let below_state = region.get_block(below_pos);
            if below_state.get_fluid_state().fluid().has_tag(&LAVA) || !below_state.is_solid() {
                return false;
            }

            if config.feature.place(region, random, working_pos) {
                place_dirt(region, config, random, origin, origin.y() + y);
                return true;
            }
        }
    }

    false
}

fn place_dirt(
    region: &WorldGenRegion<'_>,
    config: &RootSystemConfig,
    random: &mut RandomSource,
    origin: BlockPosition,
    target_height: i32,
) {
    for y in origin.y()..target_height {
        place_rooted_dirt(region, config, random, origin.at_y(y));
    }
}

fn place_rooted_dirt(
    region: &WorldGenRegion<'_>,
    config: &RootSystemConfig,
    random: &mut RandomSource,
    working_pos: BlockPosition,
) {
    let root_radius = config.root_radius;

    for _ in 0..config.root_placement_attempts {
        let pos = working_pos.offset(
            random.next_i32_bounded(root_radius) - random.next_i32_bounded(root_radius),
            0,
            random.next_i32_bounded(root_radius) - random.next_i32_bounded(root_radius),
        );
        if config
            .root_replaceable
            .contains_block(region.get_block(pos))
        {
            let state = config.root_state_provider.get_state(region, random, pos);
            region.set_block(pos, state);
        }
    }
}

fn place_roots(
    region: &WorldGenRegion<'_>,
    config: &RootSystemConfig,
    random: &mut RandomSource,
    origin: BlockPosition,
) {
    let root_radius = config.hanging_root_radius;
    let vertical_span = config.hanging_roots_vertical_span;

    for _ in 0..config.hanging_root_placement_attempts {
        let working_pos = origin.offset(
            random.next_i32_bounded(root_radius) - random.next_i32_bounded(root_radius),
            random.next_i32_bounded(vertical_span) - random.next_i32_bounded(vertical_span),
            random.next_i32_bounded(root_radius) - random.next_i32_bounded(root_radius),
        );
        if region.get_block(working_pos).is_air() {
            let target_state =
                config
                    .hanging_root_state_provider
                    .get_state(region, random, working_pos);
            if block_survival::can_survive(region, working_pos, target_state)
                && region
                    .get_block(working_pos.above())
                    .is_side_solid_at(working_pos, Direction::Down)
            {
                region.set_block(working_pos, target_state);
            }
        }
    }
}
