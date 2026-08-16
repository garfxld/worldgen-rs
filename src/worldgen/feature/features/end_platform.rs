use crate::registry::blocks::vanilla_blocks;
use crate::util::pos::BlockPos;
use crate::worldgen::region::WorldGenRegion;

pub fn create_end_platform(region: &WorldGenRegion<'_>, origin: BlockPos) {
    for dz in -2..=2 {
        for dx in -2..=2 {
            for dy in -1..3 {
                let pos = origin.offset(dx, dy, dz);
                let block = if dy == -1 {
                    &vanilla_blocks::OBSIDIAN
                } else {
                    &vanilla_blocks::AIR
                };
                if region.get_block(pos).get_block() != block {
                    region.set_block(pos, block.default_state());
                }
            }
        }
    }
}

pub fn place(region: &WorldGenRegion<'_>, origin: BlockPos) -> bool {
    create_end_platform(region, origin);
    true
}
