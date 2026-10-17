use serde::Deserialize;

use crate::util::rng::RandomSource;
use crate::util::value_providers::int::IntProvider;
use crate::world::worldgen::block_predicate::BlockPredicate;
use crate::world::worldgen::feature::features::blockpos_iter::between_closed;
use crate::world::worldgen::feature::features::mark_above_for_post_processing;
use crate::world::worldgen::feature::state_provider::ProviderHolder;
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util::BlockPosition;

#[derive(Debug, Deserialize)]
pub struct DiskConfig {
    pub state_provider: ProviderHolder,
    pub target: BlockPredicate,
    pub radius: IntProvider,
    pub half_height: i32,
}

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPosition,
    config: &DiskConfig,
) -> bool {
    let mut placed_any = false;
    let top = origin.y() + config.half_height;
    let bottom = origin.y() - config.half_height - 1;
    let radius = config.radius.sample(random);

    for column in between_closed(
        origin.offset(-radius, 0, -radius),
        origin.offset(radius, 0, radius),
    ) {
        let xd = column.x() - origin.x();
        let zd = column.z() - origin.z();
        if xd * xd + zd * zd <= radius * radius {
            placed_any |= place_column(config, region, random, top, bottom, column);
        }
    }

    placed_any
}

fn place_column(
    config: &DiskConfig,
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    top: i32,
    bottom: i32,
    column: BlockPosition,
) -> bool {
    let mut placed_any = false;
    let mut placed_above = false;

    for y in ((bottom + 1)..=top).rev() {
        let pos = column.at_y(y);
        if config.target.test(region, pos) {
            if let Some(state) = config
                .state_provider
                .get_optional_state(region, random, pos)
            {
                region.set_block(pos, state);
                if !placed_above {
                    mark_above_for_post_processing(region, pos);
                }
                placed_any = true;
                placed_above = true;
            }
        } else {
            placed_above = false;
        }
    }

    placed_any
}
