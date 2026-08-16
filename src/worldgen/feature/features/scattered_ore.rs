use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::worldgen::feature::features::ore::{OreConfig, can_place_ore};
use crate::worldgen::region::WorldGenRegion;

const MAX_DIST_FROM_ORIGIN: i32 = 7;

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPos,
    config: &OreConfig,
) -> bool {
    let number_of_tries = random.next_i32_bounded(config.size + 1);

    for i in 0..number_of_tries {
        let target_pos = offset_target_pos(random, origin, i.min(MAX_DIST_FROM_ORIGIN));
        let state = region.get_block(target_pos);

        for target in &config.targets {
            if can_place_ore(state, region, random, config, target, target_pos) {
                region.set_block(target_pos, target.state);
                break;
            }
        }
    }

    true
}

fn offset_target_pos(
    random: &mut RandomSource,
    origin: BlockPos,
    max_dist_from_origin: i32,
) -> BlockPos {
    let dx = random_placement_in_one_axis(random, max_dist_from_origin);
    let dy = random_placement_in_one_axis(random, max_dist_from_origin);
    let dz = random_placement_in_one_axis(random, max_dist_from_origin);
    origin.offset(dx, dy, dz)
}

fn random_placement_in_one_axis(random: &mut RandomSource, max_distance_from_origin: i32) -> i32 {
    let offset = (random.next_f32() - random.next_f32()) * max_distance_from_origin as f32;
    (offset + 0.5).floor() as i32
}
