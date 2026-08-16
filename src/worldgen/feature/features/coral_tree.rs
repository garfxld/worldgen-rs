use crate::util::direction::Direction;
use crate::util::ident::BlockState;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::util::shuffle;
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
    let mut pos = origin;
    let trunk_height = random.next_i32_bounded(3) + 1;

    for _ in 0..trunk_height {
        if !coral::place_coral_block(region, random, pos, state) {
            return true;
        }
        pos = pos.above();
    }

    let trunk_top = pos;
    let n_branches = random.next_i32_bounded(3) + 2;
    let mut directions = Direction::HORIZONTAL;
    shuffle(&mut directions, random);

    for branch_direction in directions.into_iter().take(n_branches as usize) {
        let mut branch_pos = branch_direction.relative(trunk_top);
        let branch_height = random.next_i32_bounded(5) + 2;
        let mut segment_length = 0;

        for j in 0..branch_height {
            if !coral::place_coral_block(region, random, branch_pos, state) {
                break;
            }
            segment_length += 1;
            branch_pos = branch_pos.above();
            if j == 0 || (segment_length >= 2 && random.next_f32() < 0.25) {
                branch_pos = branch_direction.relative(branch_pos);
                segment_length = 0;
            }
        }
    }

    true
}
