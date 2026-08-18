use serde::Deserialize;

use crate::registry::blocks::properties::Properties;
use crate::registry::blocks::vanilla_blocks;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::worldgen::region::WorldGenRegion;

#[derive(Debug, Deserialize)]
pub struct TwistingVinesConfig {
    pub spread_width: i32,
    pub spread_height: i32,
    pub max_height: i32,
}

fn is_invalid_placement_location(region: &WorldGenRegion<'_>, pos: BlockPos) -> bool {
    if !region.get_block(pos).is_air() {
        return true;
    }
    let below = region.get_block(pos.below()).get_block();
    below != &vanilla_blocks::NETHERRACK
        && below != &vanilla_blocks::WARPED_NYLIUM
        && below != &vanilla_blocks::WARPED_WART_BLOCK
}

fn find_first_air_block_above_ground(
    region: &WorldGenRegion<'_>,
    mut pos: BlockPos,
) -> Option<BlockPos> {
    loop {
        pos = pos.below();
        if !region.is_inside_build_height(pos.y()) {
            return None;
        }
        if !region.get_block(pos).is_air() {
            break;
        }
    }
    Some(pos.above())
}

fn place_twisting_vines_column(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    mut place_pos: BlockPos,
    total_height: i32,
    min_age: i32,
    max_age: i32,
) {
    for height in 1..=total_height {
        if region.get_block(place_pos).is_air() {
            if height == total_height || !region.get_block(place_pos.above()).is_air() {
                let age = random.next_i32_between(min_age, max_age);
                region.set_block(
                    place_pos,
                    vanilla_blocks::TWISTING_VINES
                        .default_state()
                        .set_value(&Properties::AGE_25, age as u8),
                );
                break;
            }

            region.set_block(
                place_pos,
                vanilla_blocks::TWISTING_VINES_PLANT.default_state(),
            );
        }

        place_pos = place_pos.above();
    }
}

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPos,
    config: &TwistingVinesConfig,
) -> bool {
    if is_invalid_placement_location(region, origin) {
        return false;
    }

    let spread_width = config.spread_width;
    let spread_height = config.spread_height;
    let max_height = config.max_height;

    for _ in 0..(spread_width * spread_width) {
        let place_pos = origin.offset(
            random.next_i32_between(-spread_width, spread_width),
            random.next_i32_between(-spread_height, spread_height),
            random.next_i32_between(-spread_width, spread_width),
        );
        if let Some(place_pos) = find_first_air_block_above_ground(region, place_pos)
            && !is_invalid_placement_location(region, place_pos)
        {
            let mut vine_height = random.next_i32_between(1, max_height);
            if random.next_i32_bounded(6) == 0 {
                vine_height *= 2;
            }
            if random.next_i32_bounded(5) == 0 {
                vine_height = 1;
            }

            place_twisting_vines_column(region, random, place_pos, vine_height, 17, 25);
        }
    }

    true
}
