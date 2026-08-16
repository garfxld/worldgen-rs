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
    if !coral::place_coral_block(region, random, origin, state) {
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
        let mut pos = branch_direction.relative(origin);
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
            if !coral::place_coral_block(region, random, pos, state) {
                break;
            }
            pos = segment_direction.relative(pos);
        }

        pos = segment_direction.opposite().relative(pos);
        pos = pos.above();

        for _ in 0..inway_length {
            pos = claw_direction.relative(pos);
            if !coral::place_coral_block(region, random, pos, state) {
                break;
            }
            if random.next_f32() < 0.25 {
                pos = pos.above();
            }
        }
    }

    true
}
