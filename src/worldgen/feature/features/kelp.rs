use crate::chunk::heightmap::HeightmapType;
use crate::registry::REGISTRY;
use crate::registry::blocks::properties::Properties;
use crate::registry::blocks::vanilla_blocks;
use crate::util::direction::Direction;
use crate::util::ident::Identifier;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::worldgen::region::WorldGenRegion;

const CANNOT_SUPPORT_KELP: Identifier = Identifier::const_vanilla("cannot_support_kelp");

fn can_survive(region: &WorldGenRegion<'_>, pos: BlockPos) -> bool {
    let below_pos = pos.below();
    let below = region.get_block(below_pos);
    let block = below.get_block();
    if REGISTRY.blocks.is_in_tag(&block.key, &CANNOT_SUPPORT_KELP) {
        return false;
    }
    block == &vanilla_blocks::KELP
        || block == &vanilla_blocks::KELP_PLANT
        || below.is_face_sturdy_at(below_pos, Direction::Up)
}

pub fn place(region: &WorldGenRegion<'_>, random: &mut RandomSource, origin: BlockPos) -> bool {
    let y = region.get_height(HeightmapType::OceanFloor, origin.x(), origin.z());
    let mut pos = origin.at_y(y);
    if region.get_block(pos).get_block() != &vanilla_blocks::WATER {
        return false;
    }

    let height = 1 + random.next_i32_bounded(10);
    let mut placed = 0;

    for h in 0..=height {
        if region.get_block(pos).get_block() == &vanilla_blocks::WATER
            && region.get_block(pos.above()).get_block() == &vanilla_blocks::WATER
            && can_survive(region, pos)
        {
            if h == height {
                region.set_block(
                    pos,
                    vanilla_blocks::KELP
                        .default_state()
                        .set_value(&Properties::AGE_25, (random.next_i32_bounded(4) + 20) as u8),
                );
                placed += 1;
            } else {
                region.set_block(pos, vanilla_blocks::KELP_PLANT.default_state());
            }
        } else if h > 0 {
            let below = pos.below();
            if can_survive(region, below)
                && region.get_block(below.below()).get_block() != &vanilla_blocks::KELP
            {
                region.set_block(
                    below,
                    vanilla_blocks::KELP
                        .default_state()
                        .set_value(&Properties::AGE_25, (random.next_i32_bounded(4) + 20) as u8),
                );
                placed += 1;
            }
            break;
        }

        pos = pos.above();
    }

    placed > 0
}
