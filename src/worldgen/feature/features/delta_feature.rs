use serde::Deserialize;

use crate::registry::blocks::BlockRef;
use crate::registry::blocks::vanilla_blocks;
use crate::util::direction::Direction;
use crate::util::ident::BlockState;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::util::value_providers::int::IntProvider;
use crate::worldgen::feature::features::DIRECTIONS;
use crate::worldgen::region::WorldGenRegion;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeltaConfig {
    pub contents: BlockState,
    pub rim: BlockState,
    pub size: IntProvider,
    pub rim_size: IntProvider,
}

fn cannot_replace(block: BlockRef) -> bool {
    block == &vanilla_blocks::BEDROCK
        || block == &vanilla_blocks::NETHER_BRICKS
        || block == &vanilla_blocks::NETHER_BRICK_FENCE
        || block == &vanilla_blocks::NETHER_BRICK_STAIRS
        || block == &vanilla_blocks::NETHER_WART
        || block == &vanilla_blocks::CHEST
        || block == &vanilla_blocks::SPAWNER
}

fn is_clear(region: &WorldGenRegion<'_>, pos: BlockPos, config: &DeltaConfig) -> bool {
    let state = region.get_block(pos);
    if state.get_block() == config.contents.get_block() {
        return false;
    }
    if cannot_replace(state.get_block()) {
        return false;
    }

    for direction in DIRECTIONS {
        let is_air = region.get_block(direction.relative(pos)).is_air();
        let is_up = direction == Direction::Up;
        if (is_air && !is_up) || (!is_air && is_up) {
            return false;
        }
    }

    true
}

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPos,
    config: &DeltaConfig,
) -> bool {
    let mut any_placed = false;
    let spawn_rim = random.next_f64() < 0.9;
    let rim_x = if spawn_rim {
        config.rim_size.sample(random)
    } else {
        0
    };
    let rim_z = if spawn_rim {
        config.rim_size.sample(random)
    } else {
        0
    };
    let has_rim = spawn_rim && rim_x != 0 && rim_z != 0;
    let radius_x = config.size.sample(random);
    let radius_z = config.size.sample(random);
    let radius_limit = radius_x.max(radius_z);

    for pos in origin.within_manhattan(radius_x, 0, radius_z) {
        let dist = (pos.x() - origin.x()).abs() + (pos.z() - origin.z()).abs();
        if dist > radius_limit {
            break;
        }

        if is_clear(region, pos, config) {
            if has_rim {
                any_placed = true;
                region.set_block(pos, config.rim);
            }

            let offset_pos = pos.offset(rim_x, 0, rim_z);
            if is_clear(region, offset_pos, config) {
                any_placed = true;
                region.set_block(offset_pos, config.contents);
            }
        }
    }

    any_placed
}
