use serde::Deserialize;

use crate::util::direction::Direction;
use crate::util::ident::BlockState;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::util::value_providers::int::IntProvider;
use crate::worldgen::block_predicate::HolderSet;
use crate::worldgen::feature::features::HORIZONTAL_DIRECTIONS;
use crate::worldgen::feature::features::tree::TreeConfig;
use crate::worldgen::feature::foliageplacers::FoliageAttachment;
use crate::worldgen::region::WorldGenRegion;

#[derive(Debug, Deserialize)]
pub struct UpwardsBranchingTrunkPlacer {
    pub base_height: i32,
    pub height_rand_a: i32,
    pub height_rand_b: i32,
    pub extra_branch_steps: IntProvider,
    pub place_branch_per_log_probability: f32,
    pub extra_branch_length: IntProvider,
    pub can_grow_through: HolderSet,
}

impl UpwardsBranchingTrunkPlacer {
    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors vanilla UpwardsBranchingTrunkPlacer.placeTrunk"
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
        let mut attachments = Vec::new();

        for height_pos in 0..tree_height {
            let current_height = origin.y() + height_pos;
            let log_pos = origin.at_y(current_height);
            if placer.place_log(region, trunk_setter, random, log_pos, config)
                && height_pos < tree_height - 1
                && random.next_f32() < self.place_branch_per_log_probability
            {
                let branch_dir = HORIZONTAL_DIRECTIONS[random.next_i32_bounded(4) as usize];
                let branch_len = self.extra_branch_length.sample(random);
                let branch_pos = (branch_len - self.extra_branch_length.sample(random) - 1).max(0);
                let branch_steps = self.extra_branch_steps.sample(random);
                self.place_branch(
                    placer,
                    region,
                    trunk_setter,
                    random,
                    tree_height,
                    config,
                    &mut attachments,
                    log_pos,
                    current_height,
                    branch_dir,
                    branch_pos,
                    branch_steps,
                );
            }

            if height_pos == tree_height - 1 {
                attachments.push(FoliageAttachment {
                    pos: origin.at_y(current_height + 1),
                    radius_offset: 0,
                    double_trunk: false,
                });
            }
        }

        attachments
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors vanilla UpwardsBranchingTrunkPlacer.placeBranch"
    )]
    fn place_branch(
        &self,
        placer: &super::TrunkPlacer,
        region: &WorldGenRegion<'_>,
        trunk_setter: &mut dyn FnMut(BlockPos, BlockState),
        random: &mut RandomSource,
        tree_height: i32,
        config: &TreeConfig,
        attachments: &mut Vec<FoliageAttachment>,
        log_pos: BlockPos,
        current_height: i32,
        branch_dir: Direction,
        branch_pos: i32,
        mut branch_steps: i32,
    ) {
        let mut height_along_branch = current_height + branch_pos;
        let mut log_x = log_pos.x();
        let mut log_z = log_pos.z();
        let mut branch_placement_index = branch_pos;

        while branch_placement_index < tree_height && branch_steps > 0 {
            if branch_placement_index >= 1 {
                let placement_height = current_height + branch_placement_index;
                log_x += branch_dir.offset().0;
                log_z += branch_dir.offset().2;
                height_along_branch = placement_height;
                let pos = BlockPos::new(log_x, placement_height, log_z);
                if placer.place_log(region, trunk_setter, random, pos, config) {
                    height_along_branch += 1;
                }

                attachments.push(FoliageAttachment {
                    pos,
                    radius_offset: 0,
                    double_trunk: false,
                });
            }

            branch_placement_index += 1;
            branch_steps -= 1;
        }

        if height_along_branch - current_height > 1 {
            let foliage_pos = BlockPos::new(log_x, height_along_branch, log_z);
            attachments.push(FoliageAttachment {
                pos: foliage_pos,
                radius_offset: 0,
                double_trunk: false,
            });
            attachments.push(FoliageAttachment {
                pos: foliage_pos.below_n(2),
                radius_offset: 0,
                double_trunk: false,
            });
        }
    }
}
