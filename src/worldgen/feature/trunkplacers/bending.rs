use serde::Deserialize;

use crate::util::ident::BlockState;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::util::value_providers::int::IntProvider;
use crate::worldgen::feature::features::HORIZONTAL_DIRECTIONS;
use crate::worldgen::feature::features::tree::{TreeConfig, valid_tree_pos};
use crate::worldgen::feature::foliageplacers::FoliageAttachment;
use crate::worldgen::region::WorldGenRegion;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BendingTrunkPlacer {
    pub base_height: i32,
    pub height_rand_a: i32,
    pub height_rand_b: i32,
    #[serde(default = "default_min_height_for_leaves")]
    pub min_height_for_leaves: i32,
    pub bend_length: IntProvider,
}

const fn default_min_height_for_leaves() -> i32 {
    1
}

impl BendingTrunkPlacer {
    #[expect(clippy::too_many_arguments, reason = "mirrors vanilla BendingTrunkPlacer.placeTrunk")]
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
        let direction = HORIZONTAL_DIRECTIONS[random.next_i32_bounded(4) as usize];
        let log_height = tree_height - 1;
        let mut pos = origin;
        super::place_below_trunk_block(region, trunk_setter, random, origin.below(), config);
        let mut foliage_points = Vec::new();

        for i in 0..=log_height {
            if i + 1 >= log_height + random.next_i32_bounded(2) {
                pos = pos.offset(direction.offset().0, 0, direction.offset().2);
            }

            if valid_tree_pos(region, pos) {
                placer.place_log(region, trunk_setter, random, pos, config);
            }

            if i >= self.min_height_for_leaves {
                foliage_points.push(FoliageAttachment {
                    pos,
                    radius_offset: 0,
                    double_trunk: false,
                });
            }

            pos = pos.above();
        }

        let dir_length = self.bend_length.sample(random);
        for _ in 0..=dir_length {
            if valid_tree_pos(region, pos) {
                placer.place_log(region, trunk_setter, random, pos, config);
            }

            foliage_points.push(FoliageAttachment {
                pos,
                radius_offset: 0,
                double_trunk: false,
            });
            pos = pos.offset(direction.offset().0, 0, direction.offset().2);
        }

        foliage_points
    }
}
