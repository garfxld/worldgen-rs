use serde::Deserialize;
use shulkr::util::BlockPosition;

use crate::util::rng::RandomSource;
use crate::util::value_providers::int::IntProvider;
use crate::world::worldgen::block_predicate::BlockPredicate;
use crate::world::worldgen::feature::state_provider::ProviderHolder;
use crate::world::worldgen::region::WorldGenRegion;

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectedRandomPatchySquare {
    pub block: ProviderHolder,
    pub project_through: BlockPredicate,
    pub size: IntProvider,
    pub max_projection_height: i32,
}

impl ProjectedRandomPatchySquare {
    pub fn place(
        &self,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        pos: BlockPosition,
    ) -> bool {
        let size = self.size.sample(random);
        let bound = size * size + 1;
        for dx in -size..=size {
            for dz in -size..=size {
                let probability = dx.abs() * dz.abs();
                if random.next_i32_bounded(bound) < bound - probability {
                    let mut base_pos = pos.offset(dx, 0, dz);
                    let mut drop = self.max_projection_height;
                    while self.project_through.test(region, base_pos.below()) {
                        base_pos = base_pos.below();
                        drop -= 1;
                        if drop <= 0 {
                            break;
                        }
                    }
                    if let Some(state) = self.block.get_optional_state(region, random, base_pos) {
                        region.set_block_with_flags(base_pos, state, 2);
                    }
                }
            }
        }
        true
    }
}
