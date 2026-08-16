use crate::registry::blocks::vanilla_blocks;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::worldgen::region::WorldGenRegion;

pub fn place(region: &WorldGenRegion<'_>, random: &mut RandomSource, origin: BlockPos) -> bool {
    let mut size = random.next_i32_bounded(3) as f32 + 4.0;
    let mut y = 0;

    while size > 0.5 {
        let min = (-size).floor() as i32;
        let max = size.ceil() as i32;
        for x in min..=max {
            for z in min..=max {
                if (x * x + z * z) as f32 <= (size + 1.0) * (size + 1.0) {
                    region.set_block(
                        origin.offset(x, y, z),
                        vanilla_blocks::END_STONE.default_state(),
                    );
                }
            }
        }

        y -= 1;
        size -= random.next_i32_bounded(2) as f32 + 0.5;
    }

    true
}
