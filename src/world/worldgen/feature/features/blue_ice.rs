use shulkr::world::block::Block;

use crate::util::rng::RandomSource;
use crate::world::worldgen::feature::features::DIRECTIONS;
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util::BlockPosition;
use shulkr::util::Direction;

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPosition,
) -> bool {
    if origin.y() > region.sea_level() - 1 {
        return false;
    }

    if !region.get_block(origin).is(Block::WATER)
        && !region.get_block(origin.below()).is(Block::WATER)
    {
        return false;
    }

    let found_packed_ice = DIRECTIONS.into_iter().any(|direction| {
        direction != Direction::Down
            && region
                .get_block(origin.relative(direction))
                .is(Block::PACKED_ICE)
    });
    if !found_packed_ice {
        return false;
    }

    region.set_block(origin, Block::BLUE_ICE.default_state());

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
        let block = place_state.as_block();
        if place_state.is_air() || matches!(block, Block::WATER | Block::PACKED_ICE | Block::ICE) {
            let adjacent_blue_ice = DIRECTIONS.into_iter().any(|direction| {
                region
                    .get_block(place_pos.relative(direction))
                    .is(Block::BLUE_ICE)
            });
            if adjacent_blue_ice {
                region.set_block(place_pos, Block::BLUE_ICE.default_state());
            }
        }
    }

    true
}
