use crate::registry::blocks::vanilla_blocks;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::worldgen::feature::features::DIRECTIONS;
use crate::worldgen::region::WorldGenRegion;

pub fn place(region: &WorldGenRegion<'_>, random: &mut RandomSource, origin: BlockPos) -> bool {
    if !region.get_block(origin).is_air() {
        return false;
    }

    let above = region.get_block(origin.above()).get_block();
    if above != &vanilla_blocks::NETHERRACK
        && above != &vanilla_blocks::BASALT
        && above != &vanilla_blocks::BLACKSTONE
    {
        return false;
    }

    region.set_block(origin, vanilla_blocks::GLOWSTONE.default_state());

    for _ in 0..1500 {
        let place_pos = origin.offset(
            random.next_i32_bounded(8) - random.next_i32_bounded(8),
            -random.next_i32_bounded(12),
            random.next_i32_bounded(8) - random.next_i32_bounded(8),
        );
        if !region.get_block(place_pos).is_air() {
            continue;
        }

        let mut neighbours = 0;
        for direction in DIRECTIONS {
            if region.get_block(place_pos.relative(direction)).get_block()
                == &vanilla_blocks::GLOWSTONE
            {
                neighbours += 1;
            }
            if neighbours > 1 {
                break;
            }
        }

        if neighbours == 1 {
            region.set_block(place_pos, vanilla_blocks::GLOWSTONE.default_state());
        }
    }

    true
}
