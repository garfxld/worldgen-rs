use serde::Deserialize;

use crate::registry::blocks::vanilla_blocks;
use crate::util::pos::BlockPos;
use crate::worldgen::region::WorldGenRegion;

#[derive(Debug, Clone, Deserialize)]
pub struct EndGatewayConfig {
    #[serde(default)]
    pub exit: Option<[i32; 3]>,
    pub exact: bool,
}

pub fn place(region: &WorldGenRegion<'_>, origin: BlockPos, _config: &EndGatewayConfig) -> bool {
    let gateway = vanilla_blocks::END_GATEWAY.default_state();
    let air = vanilla_blocks::AIR.default_state();
    let bedrock = vanilla_blocks::BEDROCK.default_state();

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
