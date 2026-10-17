use crate::world::worldgen::region::WorldGenRegion;
use serde::Deserialize;
use shulkr::util::BlockPosition;
use shulkr::world::block::Block;

#[derive(Debug, Clone, Deserialize)]
pub struct EndGatewayConfig {
    #[serde(default)]
    pub exit: Option<[i32; 3]>,
    pub exact: bool,
}

pub fn place(
    region: &WorldGenRegion<'_>,
    origin: BlockPosition,
    _config: &EndGatewayConfig,
) -> bool {
    let gateway = Block::END_GATEWAY.default_state();
    let air = Block::AIR.default_state();
    let bedrock = Block::BEDROCK.default_state();

    for x in -1..=1 {
        for y in -2..=2 {
            for z in -1..=1 {
                let pos = origin.offset(x, y, z);
                let same_x = x == 0;
                let same_y = y == 0;
                let same_z = z == 0;
                let end = y.abs() == 2;

                let state = if same_x && same_y && same_z {
                    gateway
                } else if same_y {
                    air
                } else if (end && same_x && same_z) || ((same_x || same_z) && !end) {
                    bedrock
                } else {
                    air
                };
                region.set_block(pos, state);
            }
        }
    }

    true
}
