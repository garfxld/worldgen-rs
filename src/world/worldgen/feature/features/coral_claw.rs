use shulkr::util::Direction;

use crate::util::rng::RandomSource;
use crate::util::shuffle;
use crate::world::worldgen::feature::PlacedFeature;
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util::BlockPosition;

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPosition,
    feature: &'static PlacedFeature,
) -> bool {
    if !feature.place(region, random, origin) {
        return false;
    }

    let claw_direction = Direction::HORIZONTAL[random.next_i32_bounded(4) as usize];
    let n_branches = random.next_i32_bounded(2) + 2;
    let mut possible_directions = [
        claw_direction,
        claw_direction.rotate_y_clockwise(),
        claw_direction.rotate_y_counter_clockwise(),
    ];
    shuffle(&mut possible_directions, random);

    for branch_direction in possible_directions.into_iter().take(n_branches as usize) {
        let mut pos = origin.relative(branch_direction);
        let sideway_length = random.next_i32_bounded(2) + 1;

        let (inway_length, segment_direction) = if branch_direction == claw_direction {
            (random.next_i32_bounded(3) + 2, claw_direction)
        } else {
            pos = pos.above();
            let options = [branch_direction, Direction::Up];
            let segment_direction = options[random.next_i32_bounded(2) as usize];
            (random.next_i32_bounded(3) + 3, segment_direction)
        };

        for _ in 0..sideway_length {
            if !feature.place(region, random, pos) {
                break;
            }
            pos = pos.relative(segment_direction);
        }

        pos = pos.relative(segment_direction.opposite());
        pos = pos.above();

        for _ in 0..inway_length {
            pos = pos.relative(claw_direction);
            if !feature.place(region, random, pos) {
                break;
            }
            if random.next_f32() < 0.25 {
                pos = pos.above();
            }
        }
    }

    true
}
