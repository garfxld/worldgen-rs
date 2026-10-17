use serde::Deserialize;
use shulkr::util::BlockPosition;

use crate::util::rng::RandomSource;
use crate::util::value_providers::int::IntProvider;
use crate::world::worldgen::block_predicate::{BlockPredicate, HolderSet};
use crate::world::worldgen::feature::features::blockpos_iter::between_closed;
use crate::world::worldgen::feature::state_provider::ProviderHolder;
use crate::world::worldgen::region::WorldGenRegion;

#[derive(Debug, Clone, Deserialize)]
pub struct SteppedColumnClusterFeature {
    pub block: ProviderHolder,
    pub continue_through: BlockPredicate,
    pub can_replace: BlockPredicate,
    pub cannot_place_on: HolderSet,
    pub cluster_reach: IntProvider,
    pub column_count: IntProvider,
    pub column_reach: IntProvider,
    pub height: IntProvider,
}

const fn dist_manhattan(a: BlockPosition, b: BlockPosition) -> i32 {
    (a.x() - b.x()).abs() + (a.y() - b.y()).abs() + (a.z() - b.z()).abs()
}

impl SteppedColumnClusterFeature {
    pub fn place(
        &self,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        origin: BlockPosition,
    ) -> bool {
        let lava_sea_level = region.sea_level();
        if !self.can_place_at(region, origin) {
            return false;
        }
        let column_height = self.height.sample(random);
        let cluster_reach = column_height.min(self.cluster_reach.sample(random));
        let count = self.column_count.sample(random);
        let mut placed = false;
        let width = cluster_reach * 2 + 1;
        for _ in 0..count {
            let x = origin.x() - cluster_reach + random.next_i32_bounded(width);
            let y = origin.y() + random.next_i32_bounded(1);
            let z = origin.z() - cluster_reach + random.next_i32_bounded(width);
            let pos = BlockPosition::new(x, y, z);
            let blocks_to_place_y = column_height - dist_manhattan(pos, origin);
            if blocks_to_place_y >= 0 {
                let reach = self.column_reach.sample(random);
                placed |= self.place_column(
                    region,
                    random,
                    lava_sea_level,
                    pos,
                    blocks_to_place_y,
                    reach,
                );
            }
        }
        placed
    }

    fn place_column(
        &self,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        lava_sea_level: i32,
        origin: BlockPosition,
        column_height: i32,
        reach: i32,
    ) -> bool {
        let mut placed_any = false;
        for pos in between_closed(
            BlockPosition::new(origin.x() - reach, origin.y(), origin.z() - reach),
            BlockPosition::new(origin.x() + reach, origin.y(), origin.z() + reach),
        ) {
            let step_limit = dist_manhattan(pos, origin);
            let column_pos = if self.can_replace.test(region, pos) {
                self.find_surface(region, lava_sea_level, pos, step_limit)
            } else {
                self.find_air(region, pos, step_limit)
            };
            let Some(column_pos) = column_pos else {
                continue;
            };
            let mut blocks_y = column_height - step_limit / 2;
            let mut cursor = column_pos;
            while blocks_y >= 0 {
                if self.can_replace.test(region, cursor) {
                    let state = self.block.get_state(region, random, cursor);
                    region.set_block(cursor, state);
                    cursor = cursor.above();
                    placed_any = true;
                } else {
                    if !self.continue_through.test(region, cursor) {
                        break;
                    }
                    cursor = cursor.above();
                }
                blocks_y -= 1;
            }
        }
        placed_any
    }

    fn find_surface(
        &self,
        region: &WorldGenRegion<'_>,
        _lava_sea_level: i32,
        mut cursor: BlockPosition,
        mut limit: i32,
    ) -> Option<BlockPosition> {
        while cursor.y() > region.min_y() + 1 && limit > 0 {
            limit -= 1;
            if self.can_place_at(region, cursor) {
                return Some(cursor);
            }
            cursor = cursor.below();
        }
        None
    }

    fn can_place_at(&self, region: &WorldGenRegion<'_>, cursor: BlockPosition) -> bool {
        if !self.can_replace.test(region, cursor) {
            return false;
        }
        let block_state = region.get_block(cursor.below());
        !block_state.is_air() && !self.cannot_place_on.contains_block(block_state)
    }

    fn find_air(
        &self,
        region: &WorldGenRegion<'_>,
        mut cursor: BlockPosition,
        mut limit: i32,
    ) -> Option<BlockPosition> {
        while cursor.y() < region.max_y() && limit > 0 {
            limit -= 1;
            let block_state = region.get_block(cursor);
            if self.cannot_place_on.contains_block(block_state) {
                return None;
            }
            if block_state.is_air() {
                return Some(cursor);
            }
            cursor = cursor.above();
        }
        None
    }
}
