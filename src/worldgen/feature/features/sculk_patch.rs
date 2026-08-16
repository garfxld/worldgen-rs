use serde::Deserialize;

use crate::registry::blocks::properties::Properties;
use crate::registry::blocks::shapes;
use crate::registry::blocks::vanilla_blocks;
use crate::util::direction::Direction;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::util::value_providers::int::IntProvider;
use crate::worldgen::feature::multiface;
use crate::worldgen::feature::sculk::SculkSpreader;
use crate::worldgen::region::WorldGenRegion;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SculkPatchConfig {
    pub charge_count: i32,
    pub amount_per_charge: i32,
    pub spread_attempts: i32,
    pub growth_rounds: i32,
    pub spread_rounds: i32,
    pub extra_rare_growths: IntProvider,
    pub catalyst_chance: f32,
}

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPos,
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

    let below = origin.below();
    if random.next_f32() <= config.catalyst_chance
        && shapes::is_offset_shape_full_block(region.get_block(below).get_collision_shape_at(below))
    {
        region.set_block(origin, vanilla_blocks::SCULK_CATALYST.default_state());
    }

    let extra_growths = config.extra_rare_growths.sample(random);
    for _ in 0..extra_growths {
        let candidate = origin.offset(
            random.next_i32_bounded(5) - 2,
            0,
            random.next_i32_bounded(5) - 2,
        );
        let below_candidate = candidate.below();
        if region.get_block(candidate).is_air()
            && region
                .get_block(below_candidate)
                .is_face_sturdy_at(below_candidate, Direction::Up)
        {
            region.set_block(
                candidate,
                vanilla_blocks::SCULK_SHRIEKER
                    .default_state()
                    .set_value(&Properties::CAN_SUMMON, true),
            );
        }
    }

    true
}

fn can_spread_from(region: &WorldGenRegion<'_>, origin: BlockPos) -> bool {
    let start = region.get_block(origin);
    let block = start.get_block();
    if block == &vanilla_blocks::SCULK || block == &vanilla_blocks::SCULK_VEIN {
        return true;
    }

    let fluid = start.get_fluid_state();
    if !(start.is_air() || (block == &vanilla_blocks::WATER && fluid.is_source())) {
        return false;
    }

    multiface::DIRECTIONS.iter().any(|&direction| {
        let pos = direction.relative(origin);
        shapes::is_offset_shape_full_block(region.get_block(pos).get_collision_shape_at(pos))
    })
}
