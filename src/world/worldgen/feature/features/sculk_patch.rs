use serde::Deserialize;
use shulkr::world::block::Block;

use crate::util::rng::RandomSource;
use crate::world::worldgen::feature::multiface;
use crate::world::worldgen::feature::sculk::SculkSpreader;
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util as shapes;
use shulkr::util::BlockPosition;

#[derive(Debug, Deserialize)]
pub struct SculkPatchConfig {
    pub charge_count: i32,
    pub amount_per_charge: i32,
    pub spread_attempts: i32,
    pub growth_rounds: i32,
    pub spread_rounds: i32,
}

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPosition,
    config: &SculkPatchConfig,
) -> bool {
    if !can_spread_from(region, origin) {
        return false;
    }

    let mut spreader = SculkSpreader::create_world_gen_spreader();
    let total_rounds = config.spread_rounds + config.growth_rounds;

    for round in 0..total_rounds {
        for _ in 0..config.charge_count {
            spreader.add_cursors(origin, config.amount_per_charge);
        }

        let spread_veins = round < config.spread_rounds;
        for _ in 0..config.spread_attempts {
            spreader.update_cursors(region, origin, random, spread_veins);
        }
        spreader.clear();
    }

    true
}

fn can_spread_from(region: &WorldGenRegion<'_>, origin: BlockPosition) -> bool {
    let start = region.get_block(origin);
    let block = start.as_block();
    if block == Block::SCULK || block == Block::SCULK_VEIN {
        return true;
    }

    let fluid = start.get_fluid_state();
    if !(start.is_air() || (block == Block::WATER && fluid.is_source())) {
        return false;
    }

    multiface::DIRECTIONS.iter().any(|&direction| {
        let pos = origin.relative(direction);
        shapes::is_offset_shape_full_block(region.get_block(pos).get_collision_shape_at(pos))
    })
}
