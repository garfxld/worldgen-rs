use serde::Deserialize;

use crate::chunk::heightmap::HeightmapType;
use crate::registry::blocks::properties::Properties;
use crate::registry::blocks::vanilla_blocks;
use crate::util::direction::Direction;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::util::value_providers::int::IntProvider;
use crate::worldgen::region::WorldGenRegion;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeaPickleConfig {
    pub count: IntProvider,
}

fn may_place_on(region: &WorldGenRegion<'_>, pos: BlockPos) -> bool {
    region.get_block(pos).is_face_sturdy_at(pos, Direction::Up)
}

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPos,
    config: &SeaPickleConfig,
) -> bool {
    let count = config.count.sample(random);
    let mut placed = 0;

    for _ in 0..count {
        let x = origin.x() + random.next_i32_bounded(8) - random.next_i32_bounded(8);
        let z = origin.z() + random.next_i32_bounded(8) - random.next_i32_bounded(8);
        let y = region.get_height(HeightmapType::OceanFloor, x, z);
        let pickle_pos = BlockPos::new(x, y, z);
        let pickles = random.next_i32_bounded(4) + 1;

        if region.get_block(pickle_pos).get_block() == &vanilla_blocks::WATER
            && may_place_on(region, pickle_pos.below())
        {
            region.set_block(
                pickle_pos,
                vanilla_blocks::SEA_PICKLE
                    .default_state()
                    .set_value(&Properties::PICKLES, pickles as u8),
            );
            placed += 1;
        }
    }

    placed > 0
}
