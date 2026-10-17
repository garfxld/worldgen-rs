use serde::Deserialize;
use shulkr::util::BlockPosition;
use shulkr::world::block::BlockState;

use crate::world::worldgen::region::WorldGenRegion;

#[derive(Debug, Clone, Deserialize)]
pub struct FillLayerFeature {
    pub height: i32,
    pub state: BlockState,
}

impl FillLayerFeature {
    pub fn place(&self, region: &WorldGenRegion<'_>, origin: BlockPosition) -> bool {
        for dx in 0..16 {
            for dz in 0..16 {
                let pos = BlockPosition::new(
                    origin.x() + dx,
                    region.min_y() + self.height,
                    origin.z() + dz,
                );
                if region.get_block(pos).is_air() {
                    region.set_block_with_flags(pos, self.state, 2);
                }
            }
        }
        true
    }
}
