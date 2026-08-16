use serde::Deserialize;

use crate::util::ident::BlockState;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::worldgen::feature::features::HORIZONTAL_DIRECTIONS;
use crate::worldgen::feature::features::tree::TreeConfig;
use crate::worldgen::feature::foliageplacers::FoliageAttachment;
use crate::worldgen::region::WorldGenRegion;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForkingTrunkPlacer {
    pub base_height: i32,
    pub height_rand_a: i32,
    pub height_rand_b: i32,
}

impl ForkingTrunkPlacer {
    #[expect(clippy::too_many_arguments, reason = "mirrors vanilla ForkingTrunkPlacer.placeTrunk")]
    pub fn place_trunk(
        &self,
        placer: &super::TrunkPlacer,
        region: &WorldGenRegion<'_>,
        trunk_setter: &mut dyn FnMut(BlockPos, BlockState),
        random: &mut RandomSource,
        tree_height: i32,
        origin: BlockPos,
        config: &TreeConfig,
    ) -> Vec<FoliageAttachment> {
        super::place_below_trunk_block(region, trunk_setter, random, origin.below(), config);
        let mut attachments = Vec::new();
        let lean_direction = HORIZONTAL_DIRECTIONS[random.next_i32_bounded(4) as usize];
        let lean_height = tree_height - random.next_i32_bounded(4) - 1;
        let mut lean_steps = 3 - random.next_i32_bounded(3);
        let mut tx = origin.x();
        let mut tz = origin.z();
        let mut ey: Option<i32> = None;

        for yo in 0..tree_height {
            let yy = origin.y() + yo;
            if yo >= lean_height && lean_steps > 0 {
                tx += lean_direction.offset().0;
                tz += lean_direction.offset().2;
                lean_steps -= 1;
            }

            if placer.place_log(region, trunk_setter, random, BlockPos::new(tx, yy, tz), config) {
                ey = Some(yy + 1);
            }
        }

        if let Some(ey) = ey {
            attachments.push(FoliageAttachment {
                pos: BlockPos::new(tx, ey, tz),
                radius_offset: 1,
                double_trunk: false,
            });
        }

        tx = origin.x();
        tz = origin.z();
        let branch_direction = HORIZONTAL_DIRECTIONS[random.next_i32_bounded(4) as usize];
        if branch_direction != lean_direction {
            let branch_pos = lean_height - random.next_i32_bounded(2) - 1;
            let mut branch_steps = 1 + random.next_i32_bounded(3);
            ey = None;

            let mut yo = branch_pos;
            while yo < tree_height && branch_steps > 0 {
                if yo >= 1 {
                    let yy = origin.y() + yo;
                    tx += branch_direction.offset().0;
                    tz += branch_direction.offset().2;
                    if placer.place_log(region, trunk_setter, random, BlockPos::new(tx, yy, tz), config) {
                        ey = Some(yy + 1);
                    }
                }

                yo += 1;
                branch_steps -= 1;
            }

            if let Some(ey) = ey {
                attachments.push(FoliageAttachment {
                    pos: BlockPos::new(tx, ey, tz),
                    radius_offset: 0,
                    double_trunk: false,
                });
            }
        }

        attachments
    }
}
