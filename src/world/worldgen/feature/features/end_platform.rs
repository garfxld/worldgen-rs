use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util::BlockPosition;
use shulkr::world::block::Block;

pub fn create_end_platform(region: &WorldGenRegion<'_>, origin: BlockPosition) {
    for dz in -2..=2 {
        for dx in -2..=2 {
            for dy in -1..3 {
                let pos = origin.offset(dx, dy, dz);
                let block = if dy == -1 {
                    Block::OBSIDIAN
                } else {
                    Block::AIR
                };
                if region.get_block(pos).as_block() != block {
                    region.set_block(pos, block.default_state());
                }
            }
        }
    }
}

pub fn place(region: &WorldGenRegion<'_>, origin: BlockPosition) -> bool {
    create_end_platform(region, origin);
    true
}
