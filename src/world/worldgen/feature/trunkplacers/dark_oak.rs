use serde::Deserialize;

use crate::util::rng::RandomSource;
use crate::world::worldgen::feature::features::HORIZONTAL_DIRECTIONS;
use crate::world::worldgen::feature::features::tree::{TreeConfig, is_air_or_leaves};
use crate::world::worldgen::feature::foliageplacers::FoliageAttachment;
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util::BlockPosition;
use shulkr::world::block::BlockState;

#[derive(Debug, Deserialize)]
pub struct DarkOakTrunkPlacer {
    pub base_height: i32,
    pub height_rand_a: i32,
    pub height_rand_b: i32,
}

impl DarkOakTrunkPlacer {
    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors vanilla DarkOakTrunkPlacer.placeTrunk"
    )]
    pub fn place_trunk(
        &self,
        placer: &super::TrunkPlacer,
        region: &WorldGenRegion<'_>,
        trunk_setter: &mut dyn FnMut(BlockPosition, BlockState),
        random: &mut RandomSource,
        tree_height: i32,
        origin: BlockPosition,
        config: &TreeConfig,
    ) -> Vec<FoliageAttachment> {
        let mut attachments = Vec::new();
        let below = origin.below();
        let east = |pos: BlockPosition| pos.offset(1, 0, 0);
        let south = |pos: BlockPosition| pos.offset(0, 0, 1);
        super::place_below_trunk_block(region, trunk_setter, random, below, config);
        super::place_below_trunk_block(region, trunk_setter, random, east(below), config);
        super::place_below_trunk_block(region, trunk_setter, random, south(below), config);
        super::place_below_trunk_block(region, trunk_setter, random, east(south(below)), config);

        let lean_direction = HORIZONTAL_DIRECTIONS[random.next_i32_bounded(4) as usize];
        let lean_height = tree_height - random.next_i32_bounded(4);
        let mut lean_steps = 2 - random.next_i32_bounded(3);
        let (x, y, z) = (origin.x(), origin.y(), origin.z());
        let mut tx = x;
        let mut tz = z;
        let ey = y + tree_height - 1;

        for dy in 0..tree_height {
            if dy >= lean_height && lean_steps > 0 {
                tx += lean_direction.offset().0;
                tz += lean_direction.offset().2;
                lean_steps -= 1;
            }

            let pos = BlockPosition::new(tx, y + dy, tz);
            if is_air_or_leaves(region, pos) {
                placer.place_log(region, trunk_setter, random, pos, config);
                placer.place_log(region, trunk_setter, random, east(pos), config);
                placer.place_log(region, trunk_setter, random, south(pos), config);
                placer.place_log(region, trunk_setter, random, east(south(pos)), config);
            }
        }

        attachments.push(FoliageAttachment {
            pos: BlockPosition::new(tx, ey, tz),
            radius_offset: 0,
            double_trunk: true,
        });

        for ox in -1..=2 {
            for oz in -1..=2 {
                if (!(0..=1).contains(&ox) || !(0..=1).contains(&oz))
                    && random.next_i32_bounded(3) <= 0
                {
                    let length = random.next_i32_bounded(3) + 2;
                    for branch_y in 0..length {
                        placer.place_log(
                            region,
                            trunk_setter,
                            random,
                            BlockPosition::new(x + ox, ey - branch_y - 1, z + oz),
                            config,
                        );
                    }
                    attachments.push(FoliageAttachment {
                        pos: BlockPosition::new(x + ox, ey, z + oz),
                        radius_offset: 0,
                        double_trunk: false,
                    });
                }
            }
        }

        attachments
    }
}
