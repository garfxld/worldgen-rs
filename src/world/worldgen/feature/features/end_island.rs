use crate::util::rng::RandomSource;
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util::BlockPosition;
use shulkr::world::block::Block;

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPosition,
) -> bool {
    let mut size = random.next_i32_bounded(3) as f32 + 4.0;
    let mut y = 0;

    while size > 0.5 {
        let min = (-size).floor() as i32;
        let max = size.ceil() as i32;
        for x in min..=max {
            for z in min..=max {
                if (x * x + z * z) as f32 <= (size + 1.0) * (size + 1.0) {
                    region.set_block(origin.offset(x, y, z), Block::END_STONE.default_state());
                }
            }
        }

        y -= 1;
        size -= random.next_i32_bounded(2) as f32 + 0.5;
    }

    true
}
