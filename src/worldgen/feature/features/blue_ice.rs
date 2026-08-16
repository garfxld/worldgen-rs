use crate::registry::blocks::vanilla_blocks;
use crate::util::direction::Direction;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::worldgen::feature::features::DIRECTIONS;
use crate::worldgen::region::WorldGenRegion;

pub fn place(region: &WorldGenRegion<'_>, random: &mut RandomSource, origin: BlockPos) -> bool {
    if origin.y() > region.sea_level() - 1 {
        return false;
    }

    if region.get_block(origin).get_block() != &vanilla_blocks::WATER
        && region.get_block(origin.below()).get_block() != &vanilla_blocks::WATER
    {
        return false;
    }

    let found_packed_ice = DIRECTIONS.into_iter().any(|direction| {
        direction != Direction::Down
            && region.get_block(origin.relative(direction)).get_block()
                == &vanilla_blocks::PACKED_ICE
    });
    if !found_packed_ice {
        return false;
    }

    region.set_block(origin, vanilla_blocks::BLUE_ICE.default_state());

    for _ in 0..200 {
        let y_off = random.next_i32_bounded(5) - random.next_i32_bounded(6);
        let mut xz_diff = 3;
        if y_off < 2 {
            xz_diff += y_off / 2;
        }

        if xz_diff < 1 {
            continue;
        }

        let place_pos = origin.offset(
            random.next_i32_bounded(xz_diff) - random.next_i32_bounded(xz_diff),
            y_off,
            random.next_i32_bounded(xz_diff) - random.next_i32_bounded(xz_diff),
        );
        let place_state = region.get_block(place_pos);
        let block = place_state.get_block();
        if place_state.is_air()
            || block == &vanilla_blocks::WATER
            || block == &vanilla_blocks::PACKED_ICE
            || block == &vanilla_blocks::ICE
        {
            let adjacent_blue_ice = DIRECTIONS.into_iter().any(|direction| {
                region.get_block(place_pos.relative(direction)).get_block()
                    == &vanilla_blocks::BLUE_ICE
            });
            if adjacent_blue_ice {
                region.set_block(place_pos, vanilla_blocks::BLUE_ICE.default_state());
            }
        }
    }

    true
}
