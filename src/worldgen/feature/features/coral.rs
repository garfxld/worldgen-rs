use crate::registry::REGISTRY;
use crate::registry::blocks::BlockRef;
use crate::registry::blocks::properties::Properties;
use crate::registry::blocks::vanilla_blocks;
use crate::util::direction::Direction;
use crate::util::ident::{BlockState, Identifier};
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::worldgen::region::WorldGenRegion;

const CORAL_BLOCKS: Identifier = Identifier::const_vanilla("coral_blocks");
const CORALS: Identifier = Identifier::const_vanilla("corals");
const WALL_CORALS: Identifier = Identifier::const_vanilla("wall_corals");

fn random_element_of(tag: &Identifier, random: &mut RandomSource) -> Option<BlockRef> {
    let blocks = REGISTRY.blocks.get_tag(tag)?;
    if blocks.is_empty() {
        return None;
    }
    Some(blocks[random.next_i32_bounded(blocks.len() as i32) as usize])
}

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPos,
    place_shape: impl FnOnce(&WorldGenRegion<'_>, &mut RandomSource, BlockPos, BlockState) -> bool,
) -> bool {
    let Some(coral) = random_element_of(&CORAL_BLOCKS, random) else {
        return false;
    };
    place_shape(region, random, origin, coral.default_state())
}

pub fn place_coral_block(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    pos: BlockPos,
    state: BlockState,
) -> bool {
    let above = pos.above();
    let target = region.get_block(pos);
    if !(target.get_block() == &vanilla_blocks::WATER || target.get_block().has_tag(&CORALS))
        || region.get_block(above).get_block() != &vanilla_blocks::WATER
    {
        return false;
    }

    region.set_block(pos, state);

    if random.next_f32() < 0.25 {
        if let Some(coral) = random_element_of(&CORALS, random) {
            region.set_block(above, coral.default_state());
        }
    } else if random.next_f32() < 0.05 {
        region.set_block(
            above,
            vanilla_blocks::SEA_PICKLE
                .default_state()
                .set_value(&Properties::PICKLES, (random.next_i32_bounded(4) + 1) as u8),
        );
    }

    for direction in Direction::HORIZONTAL {
        if random.next_f32() < 0.2 {
            let relative = direction.relative(pos);
            if region.get_block(relative).get_block() == &vanilla_blocks::WATER
                && let Some(coral) = random_element_of(&WALL_CORALS, random)
            {
                let mut fan_state = coral.default_state();
                if fan_state
                    .try_get_value(&Properties::HORIZONTAL_FACING)
                    .is_some()
                {
                    fan_state = fan_state.set_value(&Properties::HORIZONTAL_FACING, direction);
                }
                region.set_block(relative, fan_state);
            }
        }
    }

    true
}
