use serde::Deserialize;
use shulkr::util::BlockPosition;

use crate::util::rng::RandomSource;
use crate::world::worldgen::feature::features::ore::BlockReplacement;
use crate::world::worldgen::region::WorldGenRegion;

#[derive(Debug, Clone, Deserialize)]
pub struct ReplaceBlockFeature {
    #[serde(rename = "targets")]
    pub replacements: Vec<BlockReplacement>,
}

impl ReplaceBlockFeature {
    pub fn place(
        &self,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        origin: BlockPosition,
    ) -> bool {
        for replacement in &self.replacements {
            if replacement
                .target
                .test(region.get_block(origin), origin, random)
            {
                region.set_block_with_flags(origin, replacement.state, 2);
                break;
            }
        }
        true
    }
}
