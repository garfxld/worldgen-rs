use serde::Deserialize;
use shulkr::util::Key;

use shulkr::util::Direction;

use crate::util::rng::RandomSource;
use crate::world::worldgen::feature::features::blockpos_iter::between_closed;
use crate::world::worldgen::feature::state_provider::ProviderHolder;
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util::BlockPosition;

const DIRT_PATH: Key = Key::const_vanilla("dirt_path");

#[derive(Debug, Deserialize)]
pub struct BlockPileConfig {
    pub state_provider: ProviderHolder,
}

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPosition,
    config: &BlockPileConfig,
) -> bool {
    if origin.y() < region.min_y() + 5 {
        return false;
    }

    let xr = 2 + random.next_i32_bounded(2);
    let zr = 2 + random.next_i32_bounded(2);

    #[expect(
        clippy::if_same_then_else,
        reason = "the else branch draws an extra float, as vanilla does"
    )]
    for pos in between_closed(origin.offset(-xr, 0, -zr), origin.offset(xr, 1, zr)) {
        let xd = origin.x() - pos.x();
        let zd = origin.z() - pos.z();
        if f64::from(xd * xd + zd * zd)
            <= f64::from(random.next_f32() * 10.0 - random.next_f32() * 6.0)
        {
            try_place_block(region, pos, random, config);
        } else if random.next_f32() < 0.031 {
            try_place_block(region, pos, random, config);
        }
    }

    true
}

fn may_place_on(
    region: &WorldGenRegion<'_>,
    pos: BlockPosition,
    random: &mut RandomSource,
) -> bool {
    let below = pos.below();
    let below_state = region.get_block(below);
    if below_state.as_block().key() == &DIRT_PATH {
        return random.next_bool();
    }
    below_state.is_side_solid_at(below, Direction::Up)
}

fn try_place_block(
    region: &WorldGenRegion<'_>,
    pos: BlockPosition,
    random: &mut RandomSource,
    config: &BlockPileConfig,
) {
    if region.get_block(pos).is_air() && may_place_on(region, pos, random) {
        let state = config.state_provider.get_state(region, random, pos);
        region.set_block(pos, state);
    }
}
