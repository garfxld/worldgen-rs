use crate::registry::blocks::vanilla_blocks;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::worldgen::region::WorldGenRegion;

pub fn place(region: &WorldGenRegion<'_>, random: &mut RandomSource, origin: BlockPos) -> bool {
    if !region.get_block(origin).is_air() || region.get_block(origin.above()).is_air() {
        return false;
    }

    let basalt = vanilla_blocks::BASALT.default_state();
    let mut pos = origin;

    while region.get_block(pos).is_air() {
        if !region.is_inside_build_height(pos.y()) {
            return true;
        }

        region.set_block(pos, basalt);
        place_hangoff(region, random, pos.offset(-1, 0, 0));
        place_hangoff(region, random, pos.offset(1, 0, 0));
        place_hangoff(region, random, pos.offset(0, 0, -1));
        place_hangoff(region, random, pos.offset(0, 0, 1));
        pos = pos.below();
    }

    pos = pos.above();
    place_base_hangoff(region, random, pos.offset(-1, 0, 0));
    place_base_hangoff(region, random, pos.offset(1, 0, 0));
    place_base_hangoff(region, random, pos.offset(0, 0, -1));
    place_base_hangoff(region, random, pos.offset(0, 0, 1));
    pos = pos.below();

    for dx in -3i32..4 {
        for dz in -3i32..4 {
            let probability = dx.abs() * dz.abs();
            if random.next_i32_bounded(10) < 10 - probability {
                let mut base_pos = pos.offset(dx, 0, dz);
                let mut max_drop = 3;
                while region.get_block(base_pos.below()).is_air() {
                    base_pos = base_pos.below();
                    max_drop -= 1;
                    if max_drop <= 0 {
                        break;
                    }
                }

                if !region.get_block(base_pos.below()).is_air() {
                    region.set_block(base_pos, basalt);
                }
            }
        }
    }

    true
}

fn place_hangoff(region: &WorldGenRegion<'_>, random: &mut RandomSource, pos: BlockPos) -> bool {
    if random.next_i32_bounded(10) != 0 {
        region.set_block(pos, vanilla_blocks::BASALT.default_state());
        true
    } else {
        false
    }
}

fn place_base_hangoff(region: &WorldGenRegion<'_>, random: &mut RandomSource, pos: BlockPos) {
    if random.next_bool() {
        region.set_block(pos, vanilla_blocks::BASALT.default_state());
    }
}
