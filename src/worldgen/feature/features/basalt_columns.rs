use serde::Deserialize;

use crate::registry::blocks::BlockRef;
use crate::registry::blocks::vanilla_blocks;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::util::value_providers::int::IntProvider;
use crate::worldgen::region::WorldGenRegion;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColumnConfig {
    pub reach: IntProvider,
    pub height: IntProvider,
}

fn cannot_place_on(block: BlockRef) -> bool {
    block == &vanilla_blocks::LAVA
        || block == &vanilla_blocks::BEDROCK
        || block == &vanilla_blocks::MAGMA_BLOCK
        || block == &vanilla_blocks::SOUL_SAND
        || block == &vanilla_blocks::NETHER_BRICKS
        || block == &vanilla_blocks::NETHER_BRICK_FENCE
        || block == &vanilla_blocks::NETHER_BRICK_STAIRS
        || block == &vanilla_blocks::NETHER_WART
        || block == &vanilla_blocks::CHEST
        || block == &vanilla_blocks::SPAWNER
}

fn is_air_or_lava_ocean(region: &WorldGenRegion<'_>, lava_sea_level: i32, pos: BlockPos) -> bool {
    let state = region.get_block(pos);
    state.is_air() || (state.get_block() == &vanilla_blocks::LAVA && pos.y() <= lava_sea_level)
}

fn can_place_at(region: &WorldGenRegion<'_>, lava_sea_level: i32, pos: BlockPos) -> bool {
    if !is_air_or_lava_ocean(region, lava_sea_level, pos) {
        return false;
    }
    let below = region.get_block(pos.below());
    !below.is_air() && !cannot_place_on(below.get_block())
}

fn find_surface(
    region: &WorldGenRegion<'_>,
    lava_sea_level: i32,
    mut pos: BlockPos,
    mut limit: i32,
) -> Option<BlockPos> {
    while pos.y() > region.min_y() + 1 && limit > 0 {
        limit -= 1;
        if can_place_at(region, lava_sea_level, pos) {
            return Some(pos);
        }
        pos = pos.below();
    }
    None
}

fn find_air(region: &WorldGenRegion<'_>, mut pos: BlockPos, mut limit: i32) -> Option<BlockPos> {
    while pos.y() <= region.max_y() && limit > 0 {
        limit -= 1;
        let state = region.get_block(pos);
        if cannot_place_on(state.get_block()) {
            return None;
        }
        if state.is_air() {
            return Some(pos);
        }
        pos = pos.above();
    }
    None
}

fn place_column(
    region: &WorldGenRegion<'_>,
    lava_sea_level: i32,
    origin: BlockPos,
    column_height: i32,
    reach: i32,
) -> bool {
    let basalt = vanilla_blocks::BASALT.default_state();
    let mut placed_any = false;

    for x in origin.x() - reach..=origin.x() + reach {
        for z in origin.z() - reach..=origin.z() + reach {
            let pos = BlockPos::new(x, origin.y(), z);
            let step_limit = (x - origin.x()).abs() + (z - origin.z()).abs();
            let column_pos = if is_air_or_lava_ocean(region, lava_sea_level, pos) {
                find_surface(region, lava_sea_level, pos, step_limit)
            } else {
                find_air(region, pos, step_limit)
            };
            let Some(column_pos) = column_pos else {
                continue;
            };

            let mut blocks_y = column_height - step_limit / 2;
            let mut cursor = column_pos;
            while blocks_y >= 0 {
                if is_air_or_lava_ocean(region, lava_sea_level, cursor) {
                    region.set_block(cursor, basalt);
                    cursor = cursor.above();
                    placed_any = true;
                } else {
                    if region.get_block(cursor).get_block() != &vanilla_blocks::BASALT {
                        break;
                    }
                    cursor = cursor.above();
                }
                blocks_y -= 1;
            }
        }
    }

    placed_any
}

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPos,
    config: &ColumnConfig,
) -> bool {
    let lava_sea_level = region.sea_level();
    if !can_place_at(region, lava_sea_level, origin) {
        return false;
    }

    let column_height = config.height.sample(random);
    let generate_clustered = random.next_f32() < 0.9;
    let reach_bound = if generate_clustered { 5 } else { 8 };
    let reach = column_height.min(reach_bound);
    let count = if generate_clustered { 50 } else { 15 };
    let width = 2 * reach + 1;
    let mut placed = false;

    for _ in 0..count {
        let x = origin.x() - reach + random.next_i32_bounded(width);
        let _ = random.next_i32_bounded(1);
        let z = origin.z() - reach + random.next_i32_bounded(width);
        let pos = BlockPos::new(x, origin.y(), z);
        let blocks_to_place_y =
            column_height - ((x - origin.x()).abs() + (z - origin.z()).abs());
        if blocks_to_place_y >= 0 {
            placed |= place_column(
                region,
                lava_sea_level,
                pos,
                blocks_to_place_y,
                config.reach.sample(random),
            );
        }
    }

    placed
}
