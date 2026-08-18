use serde::Deserialize;

use crate::chunk::heightmap::HeightmapType;
use crate::registry::REGISTRY;
use crate::registry::blocks::properties::{DoubleBlockHalf, Properties};
use crate::registry::blocks::vanilla_blocks;
use crate::util::direction::Direction;
use crate::util::ident::Identifier;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::worldgen::region::WorldGenRegion;

const CANNOT_SUPPORT_SEAGRASS: Identifier = Identifier::const_vanilla("cannot_support_seagrass");

#[derive(Debug, Deserialize)]
pub struct SeagrassConfig {
    pub probability: f32,
}

fn can_survive(region: &WorldGenRegion<'_>, pos: BlockPos, is_tall: bool) -> bool {
    let below_pos = pos.below();
    let below = region.get_block(below_pos);
    if !below.is_face_sturdy_at(below_pos, Direction::Up)
        || REGISTRY
            .blocks
            .is_in_tag(&below.get_block().key, &CANNOT_SUPPORT_SEAGRASS)
    {
        return false;
    }
    if !is_tall {
        return true;
    }
    let fluid = region.get_block(pos).get_fluid_state();
    fluid.is_water() && fluid.is_full()
}

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPos,
    config: &SeagrassConfig,
) -> bool {
    let x = origin.x() + random.next_i32_bounded(8) - random.next_i32_bounded(8);
    let z = origin.z() + random.next_i32_bounded(8) - random.next_i32_bounded(8);
    let y = region.get_height(HeightmapType::OceanFloor, x, z);
    let grass_pos = BlockPos::new(x, y, z);

    if region.get_block(grass_pos).get_block() != &vanilla_blocks::WATER {
        return false;
    }

    let is_tall = random.next_f64() < f64::from(config.probability);
    if !can_survive(region, grass_pos, is_tall) {
        return false;
    }

    if is_tall {
        let above = grass_pos.above();
        if region.get_block(above).get_block() != &vanilla_blocks::WATER {
            return false;
        }
        region.set_block(
            grass_pos,
            vanilla_blocks::TALL_SEAGRASS
                .default_state()
                .set_value(&Properties::DOUBLE_BLOCK_HALF, DoubleBlockHalf::Lower),
        );
        region.set_block(
            above,
            vanilla_blocks::TALL_SEAGRASS
                .default_state()
                .set_value(&Properties::DOUBLE_BLOCK_HALF, DoubleBlockHalf::Upper),
        );
    } else {
        region.set_block(grass_pos, vanilla_blocks::SEAGRASS.default_state());
    }

    true
}
