use serde::Deserialize;

use crate::registry::REGISTRY;
use crate::util::ident::Identifier;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::worldgen::block_survival;
use crate::worldgen::feature::state_provider::BlockStateProvider;
use crate::worldgen::region::WorldGenRegion;

const NYLIUM: Identifier = Identifier::const_vanilla("nylium");

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetherForestVegetationConfig {
    pub state_provider: BlockStateProvider,
    pub spread_width: i32,
    pub spread_height: i32,
}

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPos,
    config: &NetherForestVegetationConfig,
) -> bool {
    let below = region.get_block(origin.below());
    if !REGISTRY.blocks.is_in_tag(&below.get_block().key, &NYLIUM) {
        return false;
    }

    if origin.y() < region.min_y() + 1 || origin.y() + 1 > region.max_y() {
        return false;
    }

    let mut placed = 0;
    for _ in 0..(config.spread_width * config.spread_width) {
        let final_pos = origin.offset(
            random.next_i32_bounded(config.spread_width)
                - random.next_i32_bounded(config.spread_width),
            random.next_i32_bounded(config.spread_height)
                - random.next_i32_bounded(config.spread_height),
            random.next_i32_bounded(config.spread_width)
                - random.next_i32_bounded(config.spread_width),
        );
        let state = config.state_provider.get_state(region, random, final_pos);
        if region.get_block(final_pos).is_air()
            && final_pos.y() > region.min_y()
            && block_survival::can_survive(region, final_pos, state)
        {
            region.set_block(final_pos, state);
            placed += 1;
        }
    }

    placed > 0
}
