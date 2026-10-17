use std::f32::consts::TAU;

use serde::Deserialize;

use crate::util::math;
use crate::util::rng::RandomSource;
use crate::world::worldgen::feature::features::tree::TreeConfig;
use crate::world::worldgen::feature::foliageplacers::FoliageAttachment;
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util::BlockPosition;
use shulkr::world::block::BlockState;

use super::giant::GiantTrunkPlacer;

#[derive(Debug, Deserialize)]
pub struct MegaJungleTrunkPlacer {
    #[serde(flatten)]
    pub giant: GiantTrunkPlacer,
}

impl MegaJungleTrunkPlacer {
    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors vanilla MegaJungleTrunkPlacer.placeTrunk"
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
        let mut attachments = self.giant.place_trunk(
            placer,
            region,
            trunk_setter,
            random,
            tree_height,
            origin,
            config,
        );

        let mut branch_height = tree_height - 2 - random.next_i32_bounded(4);
        while branch_height > tree_height / 2 {
            let angle = random.next_f32() * TAU;
            let mut bx = 0;
            let mut bz = 0;
            for b in 0..5 {
                bx = (1.5 + math::cos(f64::from(angle)) * b as f32) as i32;
                bz = (1.5 + math::sin(f64::from(angle)) * b as f32) as i32;
                placer.place_log(
                    region,
                    trunk_setter,
                    random,
                    origin.offset(bx, branch_height - 3 + b / 2, bz),
                    config,
                );
            }
            attachments.push(FoliageAttachment {
                pos: origin.offset(bx, branch_height, bz),
                radius_offset: -2,
                double_trunk: false,
            });
            branch_height -= 2 + random.next_i32_bounded(4);
        }

        attachments
    }
}
