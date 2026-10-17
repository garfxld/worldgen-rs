use serde::Deserialize;

use crate::util::rng::RandomSource;
use crate::world::worldgen::feature::features::tree::TreeConfig;
use crate::world::worldgen::feature::foliageplacers::FoliageAttachment;
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util::BlockPosition;
use shulkr::world::block::BlockState;

#[derive(Debug, Deserialize)]
pub struct StraightTrunkPlacer {
    pub base_height: i32,
    pub height_rand_a: i32,
    pub height_rand_b: i32,
}

impl StraightTrunkPlacer {
    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors vanilla StraightTrunkPlacer.placeTrunk"
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
        super::place_below_trunk_block(region, trunk_setter, random, origin.below(), config);
        for y in 0..tree_height {
            placer.place_log(region, trunk_setter, random, origin.above_n(y), config);
        }
        vec![FoliageAttachment {
            pos: origin.above_n(tree_height),
            radius_offset: 0,
            double_trunk: false,
        }]
    }
}
