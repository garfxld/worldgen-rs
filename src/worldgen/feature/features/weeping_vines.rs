use crate::registry::blocks::properties::Properties;
use crate::registry::blocks::vanilla_blocks;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::worldgen::feature::features::DIRECTIONS;
use crate::worldgen::region::WorldGenRegion;

pub fn place_weeping_vines_column(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    mut place_pos: BlockPos,
    total_height: i32,
    min_age: i32,
    max_age: i32,
) {
    for height in 0..=total_height {
        if region.get_block(place_pos).is_air() {
            if height == total_height || !region.get_block(place_pos.below()).is_air() {
                let age = random.next_i32_between(min_age, max_age);
                region.set_block(
                    place_pos,
                    vanilla_blocks::WEEPING_VINES
                        .default_state()
                        .set_value(&Properties::AGE_25, age as u8),
                );
                break;
            }

            region.set_block(place_pos, vanilla_blocks::WEEPING_VINES_PLANT.default_state());
        }

        place_pos = place_pos.below();
    }
}

fn place_roof_nether_wart(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPos,
) {
    region.set_block(origin, vanilla_blocks::NETHER_WART_BLOCK.default_state());

    for _ in 0..200 {
        let place_pos = origin.offset(
            random.next_i32_bounded(6) - random.next_i32_bounded(6),
            random.next_i32_bounded(2) - random.next_i32_bounded(5),
            random.next_i32_bounded(6) - random.next_i32_bounded(6),
        );
        if region.get_block(place_pos).is_air() {
            let mut neighbours = 0;
            for direction in DIRECTIONS {
                let neighbour = region.get_block(direction.relative(place_pos)).get_block();
                if neighbour == &vanilla_blocks::NETHERRACK
                    || neighbour == &vanilla_blocks::NETHER_WART_BLOCK
                {
                    neighbours += 1;
                }
                if neighbours > 1 {
                    break;
                }
            }

            if neighbours == 1 {
                region.set_block(place_pos, vanilla_blocks::NETHER_WART_BLOCK.default_state());
            }
        }
    }
}

fn place_roof_weeping_vines(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPos,
) {
    for _ in 0..100 {
        let place_pos = origin.offset(
            random.next_i32_bounded(8) - random.next_i32_bounded(8),
            random.next_i32_bounded(2) - random.next_i32_bounded(7),
            random.next_i32_bounded(8) - random.next_i32_bounded(8),
        );
        if region.get_block(place_pos).is_air() {
            let above = region.get_block(place_pos.above()).get_block();
            if above == &vanilla_blocks::NETHERRACK || above == &vanilla_blocks::NETHER_WART_BLOCK {
                let mut vine_height = random.next_i32_between(1, 8);
                if random.next_i32_bounded(6) == 0 {
                    vine_height *= 2;
                }
                if random.next_i32_bounded(5) == 0 {
                    vine_height = 1;
                }

                place_weeping_vines_column(region, random, place_pos, vine_height, 17, 25);
            }
        }
    }
}

pub fn place(region: &WorldGenRegion<'_>, random: &mut RandomSource, origin: BlockPos) -> bool {
    if !region.get_block(origin).is_air() {
        return false;
    }

    let above = region.get_block(origin.above()).get_block();
    if above != &vanilla_blocks::NETHERRACK && above != &vanilla_blocks::NETHER_WART_BLOCK {
        return false;
    }

    place_roof_nether_wart(region, random, origin);
    place_roof_weeping_vines(region, random, origin);
    true
}
