use crate::util::rng::RandomSource;
use crate::util::shuffle;
use crate::world::worldgen::feature::PlacedFeature;
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util::BlockPosition;
use shulkr::util::Direction;

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPosition,
    feature: &'static PlacedFeature,
) -> bool {
    let mut pos = origin;
    let trunk_height = random.next_i32_bounded(3) + 1;

    for _ in 0..trunk_height {
        if !feature.place(region, random, pos) {
            return true;
        }
        pos = pos.above();
    }

    let trunk_top = pos;
    let n_branches = random.next_i32_bounded(3) + 2;
    let mut directions = Direction::HORIZONTAL;
    shuffle(&mut directions, random);

    for branch_direction in directions.into_iter().take(n_branches as usize) {
        let mut branch_pos = trunk_top.relative(branch_direction);
        let branch_height = random.next_i32_bounded(5) + 2;
        let mut segment_length = 0;

        for j in 0..branch_height {
            if !feature.place(region, random, branch_pos) {
                break;
            }
            segment_length += 1;
            branch_pos = branch_pos.above();
            if j == 0 || (segment_length >= 2 && random.next_f32() < 0.25) {
                branch_pos = branch_pos.relative(branch_direction);
                segment_length = 0;
            }
        }
    }

    true
}
