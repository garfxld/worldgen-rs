use serde::Deserialize;
use shulkr::util::BlockPosition;

use crate::util::rng::RandomSource;
use crate::util::value_providers::int::IntProvider;
use crate::world::worldgen::block_predicate::{BlockPredicate, HolderSet};
use crate::world::worldgen::feature::features::DIRECTIONS;
use crate::world::worldgen::feature::state_provider::ProviderHolder;
use crate::world::worldgen::region::WorldGenRegion;

#[derive(Debug, Clone, Deserialize)]
pub struct RandomNeighborSpreadFeature {
    pub block: ProviderHolder,
    pub accepted_neighbors: HolderSet,
    pub can_replace: BlockPredicate,
    pub attempts: IntProvider,
    pub xz_offset: IntProvider,
    pub y_offset: IntProvider,
}

impl RandomNeighborSpreadFeature {
    pub fn place(
        &self,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        origin: BlockPosition,
    ) -> bool {
        let state = self.block.get_state(region, random, origin);
        region.set_block_with_flags(origin, state, 2);
        let attempts = self.attempts.sample(random);
        for _ in 0..attempts {
            let dx = self.xz_offset.sample(random);
            let dy = self.y_offset.sample(random);
            let dz = self.xz_offset.sample(random);
            let place_pos = origin.offset(dx, dy, dz);
            if self.can_replace.test(region, place_pos) {
                let mut neighbours = 0;
                for direction in DIRECTIONS {
                    if self
                        .accepted_neighbors
                        .contains_block(region.get_block(place_pos.relative(direction)))
                    {
                        neighbours += 1;
                    }
                    if neighbours > 1 {
                        break;
                    }
                }
                if neighbours == 1 {
                    let state = self.block.get_state(region, random, place_pos);
                    region.set_block_with_flags(place_pos, state, 2);
                }
            }
        }
        true
    }
}
