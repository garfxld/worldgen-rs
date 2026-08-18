use serde::Deserialize;

use crate::util::ident::BlockState;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::worldgen::feature::features::tree::TreeConfig;
use crate::worldgen::feature::foliageplacers::FoliageAttachment;
use crate::worldgen::region::WorldGenRegion;

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct GiantTrunkPlacer {
    pub base_height: i32,
    pub height_rand_a: i32,
    pub height_rand_b: i32,
}

impl GiantTrunkPlacer {
    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors vanilla GiantTrunkPlacer.placeTrunk"
    )]
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
        let below = origin.below();
        super::place_below_trunk_block(region, trunk_setter, random, below, config);
        super::place_below_trunk_block(region, trunk_setter, random, below.offset(1, 0, 0), config);
        super::place_below_trunk_block(region, trunk_setter, random, below.offset(0, 0, 1), config);
        super::place_below_trunk_block(region, trunk_setter, random, below.offset(1, 0, 1), config);

        for hh in 0..tree_height {
            placer.place_log_if_free(
                region,
                trunk_setter,
                random,
                origin.offset(0, hh, 0),
                config,
            );
            if hh < tree_height - 1 {
                placer.place_log_if_free(
                    region,
                    trunk_setter,
                    random,
                    origin.offset(1, hh, 0),
                    config,
                );
                placer.place_log_if_free(
                    region,
                    trunk_setter,
                    random,
                    origin.offset(1, hh, 1),
                    config,
                );
                placer.place_log_if_free(
                    region,
                    trunk_setter,
                    random,
                    origin.offset(0, hh, 1),
                    config,
                );
            }
        }

        vec![FoliageAttachment {
            pos: origin.above_n(tree_height),
            radius_offset: 0,
            double_trunk: true,
        }]
    }
}
