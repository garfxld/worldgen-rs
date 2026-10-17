use serde::Deserialize;

use crate::util::rng::RandomSource;
use crate::util::value_providers::int::IntProvider;
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util::BlockPosition;
use shulkr::world::block::Block;
use shulkr::world::block::BlockState;

#[derive(Debug, Deserialize)]
pub struct ReplaceSphereConfig {
    pub target: BlockState,
    pub state: BlockState,
    pub radius: IntProvider,
}

fn find_target(
    region: &WorldGenRegion<'_>,
    mut cursor: BlockPosition,
    target: Block,
) -> Option<BlockPosition> {
    while cursor.y() > region.min_y() + 1 {
        if region.get_block(cursor).as_block() == target {
            return Some(cursor);
        }
        cursor = cursor.below();
    }
    None
}

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPosition,
    config: &ReplaceSphereConfig,
) -> bool {
    let target = config.target.as_block();
    let clamped_origin = origin.at_y(origin.y().clamp(region.min_y() + 1, region.max_y()));
    let Some(center) = find_target(region, clamped_origin, target) else {
        return false;
    };

    let radius_x = config.radius.sample(random);
    let radius_y = config.radius.sample(random);
    let radius_z = config.radius.sample(random);
    let max_radius = radius_x.max(radius_y).max(radius_z);
    let mut replaced_any = false;

    for pos in center.within_manhattan(radius_x, radius_y, radius_z) {
        let dist = (pos.x() - center.x()).abs()
            + (pos.y() - center.y()).abs()
            + (pos.z() - center.z()).abs();
        if dist > max_radius {
            break;
        }
        if region.get_block(pos).as_block() == target {
            region.set_block(pos, config.state);
            replaced_any = true;
        }
    }

    replaced_any
}
