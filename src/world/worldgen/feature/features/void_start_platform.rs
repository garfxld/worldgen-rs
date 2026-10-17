use crate::util::pos::ChunkPos;
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util::BlockPosition;
use shulkr::world::block::Block;

const PLATFORM_OFFSET: BlockPosition = BlockPosition::new(8, 3, 8);
const PLATFORM_RADIUS: i32 = 16;

const fn checkerboard_distance(xa: i32, za: i32, xb: i32, zb: i32) -> i32 {
    let dx = (xa - xb).abs();
    let dz = (za - zb).abs();
    if dx > dz { dx } else { dz }
}

pub fn place(region: &WorldGenRegion<'_>, origin: BlockPosition) -> bool {
    let platform_origin_chunk = ChunkPos::from_block_pos(PLATFORM_OFFSET);
    let current_chunk = ChunkPos::from_block_pos(origin);
    if checkerboard_distance(
        current_chunk.0.x,
        current_chunk.0.y,
        platform_origin_chunk.0.x,
        platform_origin_chunk.0.y,
    ) > 1
    {
        return true;
    }

    let platform_origin = PLATFORM_OFFSET.at_y(origin.y() + PLATFORM_OFFSET.y());
    let min_x = current_chunk.0.x * 16;
    let max_x = min_x + 15;
    let min_z = current_chunk.0.y * 16;
    let max_z = min_z + 15;

    for z in min_z..=max_z {
        for x in min_x..=max_x {
            if checkerboard_distance(platform_origin.x(), platform_origin.z(), x, z)
                <= PLATFORM_RADIUS
            {
                let pos = BlockPosition::new(x, platform_origin.y(), z);
                let block = if pos == platform_origin {
                    &Block::COBBLESTONE
                } else {
                    &Block::STONE
                };
                region.set_block(pos, block.default_state());
            }
        }
    }

    true
}
