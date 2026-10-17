use serde::Deserialize;
use shulkr::util::Key;
use shulkr::world::block::{BambooLeaves, Block, BlockState, Properties};

use crate::world::chunk::heightmap::HeightmapType;

use crate::util::rng::RandomSource;
use crate::world::worldgen::block_survival;
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util::BlockPosition;

const BENEATH_BAMBOO_PODZOL_REPLACEABLE: Key =
    Key::const_vanilla("beneath_bamboo_podzol_replaceable");

#[derive(Debug, Deserialize)]
pub struct BambooConfig {
    pub probability: f32,
}

fn bamboo_trunk() -> BlockState {
    Block::BAMBOO
        .default_state()
        .with(Properties::AGE_1, 1)
        .with(Properties::BAMBOO_LEAVES, BambooLeaves::None)
        .with(Properties::STAGE, 0)
}

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPosition,
    config: &BambooConfig,
) -> bool {
    let mut bamboo_pos = origin;
    if !region.get_block(bamboo_pos).is_air() {
        return false;
    }

    let trunk = bamboo_trunk();
    if block_survival::can_survive(region, bamboo_pos, trunk) {
        let height = random.next_i32_bounded(12) + 5;
        if random.next_f32() < config.probability {
            let r = random.next_i32_bounded(4) + 1;
            for xx in origin.x() - r..=origin.x() + r {
                for zz in origin.z() - r..=origin.z() + r {
                    let xd = xx - origin.x();
                    let zd = zz - origin.z();
                    if xd * xd + zd * zd <= r * r {
                        let podzol_y = region.get_height(HeightmapType::WorldSurface, xx, zz) - 1;
                        let podzol_pos = BlockPosition::new(xx, podzol_y, zz);
                        if region
                            .get_block(podzol_pos)
                            .as_block()
                            .has_tag(&BENEATH_BAMBOO_PODZOL_REPLACEABLE)
                        {
                            region.set_block(podzol_pos, Block::PODZOL.default_state());
                        }
                    }
                }
            }
        }

        let mut i = 0;
        while i < height && region.get_block(bamboo_pos).is_air() {
            region.set_block(bamboo_pos, trunk);
            bamboo_pos = bamboo_pos.above();
            i += 1;
        }

        if bamboo_pos.y() - origin.y() >= 3 {
            region.set_block(
                bamboo_pos,
                trunk
                    .with(Properties::BAMBOO_LEAVES, BambooLeaves::Large)
                    .with(Properties::STAGE, 1),
            );
            bamboo_pos = bamboo_pos.below();
            region.set_block(
                bamboo_pos,
                trunk
                    .with(Properties::BAMBOO_LEAVES, BambooLeaves::Large)
                    .with(Properties::AGE_1, 1),
            );
            bamboo_pos = bamboo_pos.below();
            region.set_block(
                bamboo_pos,
                trunk
                    .with(Properties::BAMBOO_LEAVES, BambooLeaves::Small)
                    .with(Properties::AGE_1, 1),
            );
        }
    }

    true
}
