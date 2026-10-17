use shulkr::util::{BlockPosition, Direction};

use crate::util::rng::RandomSource;
use crate::world::worldgen::block_predicate::BlockPredicate;
use crate::world::worldgen::feature::PlacedFeature;
use crate::world::worldgen::feature::state_provider::ProviderHolder;
use crate::world::worldgen::region::WorldGenRegion;

#[derive(Debug)]
pub struct SingleBlockPillarFeature {
    pub block: ProviderHolder,
    pub can_replace: BlockPredicate,
    pub direction: Direction,
    pub chance_to_continue: f32,
    pub cap_feature: Option<&'static PlacedFeature>,
}

impl SingleBlockPillarFeature {
    pub fn place(
        &self,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        origin: BlockPosition,
    ) -> bool {
        let mut pos = origin;
        while self.can_replace.test(region, pos)
            && random.next_f32() < self.chance_to_continue
            && region.is_inside_build_height(pos.y())
        {
            let state = self.block.get_state(region, random, pos);
            region.set_block_with_flags(pos, state, 2);
            pos = pos.relative(self.direction);
        }
        pos = pos.relative(self.direction.opposite());
        if let Some(feature) = self.cap_feature {
            feature.place(region, random, pos);
        }
        true
    }
}
