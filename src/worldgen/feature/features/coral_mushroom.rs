use crate::util::ident::BlockState;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::worldgen::feature::features::coral;
use crate::worldgen::region::WorldGenRegion;

pub fn place(region: &WorldGenRegion<'_>, random: &mut RandomSource, origin: BlockPos) -> bool {
    coral::place(region, random, origin, place_shape)
}

fn place_shape(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPos,
    state: BlockState,
) -> bool {
    let height = random.next_i32_bounded(3) + 3;
    let width = random.next_i32_bounded(3) + 3;
    let length = random.next_i32_bounded(3) + 3;
    let sink_value = random.next_i32_bounded(3) + 1;

    for x in 0..=width {
        for y in 0..=height {
            for z in 0..=length {
                let pos = origin.offset(x, y - sink_value, z);
                let on_shell = (x != 0 && x != width || y != 0 && y != height)
                    && (z != 0 && z != length || y != 0 && y != height)
                    && (x != 0 && x != width || z != 0 && z != length)
                    && (x == 0 || x == width || y == 0 || y == height || z == 0 || z == length);
                if on_shell && random.next_f32() >= 0.1 {
                    coral::place_coral_block(region, random, pos, state);
                }
            }
        }
    }

    true
}
