use serde::Deserialize;
use shulkr::world::block::BlockState;

use crate::util::rng::RandomSource;
use crate::world::worldgen::block_predicate::BlockPredicate;
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util::BlockPosition;

#[derive(Debug, Deserialize)]
pub struct BlockBlobConfig {
    pub state: BlockState,
    pub can_place_on: BlockPredicate,
}

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPosition,
    config: &BlockBlobConfig,
) -> bool {
    let mut origin = origin;
    while origin.y() > region.min_y() + 3 && !config.can_place_on.test(region, origin.below()) {
        origin = origin.below();
    }

    if origin.y() <= region.min_y() + 3 {
        return false;
    }

    for _ in 0..3 {
        let xr = random.next_i32_bounded(2);
        let yr = random.next_i32_bounded(2);
        let zr = random.next_i32_bounded(2);
        let tr = (xr + yr + zr) as f32 * 0.333 + 0.5;

        for y in origin.y() - yr..=origin.y() + yr {
            for z in origin.z() - zr..=origin.z() + zr {
                for x in origin.x() - xr..=origin.x() + xr {
                    let (dx, dy, dz) = (x - origin.x(), y - origin.y(), z - origin.z());
                    let dist_sqr = (dx * dx + dy * dy + dz * dz) as f32;
                    if dist_sqr <= tr * tr {
                        region.set_block(BlockPosition::new(x, y, z), config.state);
                    }
                }
            }
        }

        origin = BlockPosition::new(
            origin.x() - 1 + random.next_i32_bounded(2),
            origin.y() - random.next_i32_bounded(2),
            origin.z() - 1 + random.next_i32_bounded(2),
        );
    }

    true
}
