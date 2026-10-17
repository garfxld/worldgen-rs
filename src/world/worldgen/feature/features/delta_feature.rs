use serde::Deserialize;

use crate::util::rng::RandomSource;
use crate::util::value_providers::int::IntProvider;
use crate::world::worldgen::feature::features::DIRECTIONS;
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util::BlockPosition;
use shulkr::util::Direction;
use shulkr::world::block::Block;
use shulkr::world::block::BlockState;

#[derive(Debug, Deserialize)]
pub struct DeltaConfig {
    pub contents: BlockState,
    pub rim: BlockState,
    pub size: IntProvider,
    pub rim_size: IntProvider,
}

fn cannot_replace(block: Block) -> bool {
    block == Block::BEDROCK
        || block == Block::NETHER_BRICKS
        || block == Block::NETHER_BRICK_FENCE
        || block == Block::NETHER_BRICK_STAIRS
        || block == Block::NETHER_WART
        || block == Block::CHEST
        || block == Block::SPAWNER
}

fn is_clear(region: &WorldGenRegion<'_>, pos: BlockPosition, config: &DeltaConfig) -> bool {
    let state = region.get_block(pos);
    if state.as_block() == config.contents.as_block() {
        return false;
    }
    if cannot_replace(state.as_block()) {
        return false;
    }

    for direction in DIRECTIONS {
        let is_air = region.get_block(pos.relative(direction)).is_air();
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
    origin: BlockPosition,
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
