use serde::Deserialize;

use crate::util::rng::RandomSource;
use crate::util::value_providers::int::{IntProvider, UniformIntProvider};
use crate::world::worldgen::feature::features::HORIZONTAL_DIRECTIONS;
use crate::world::worldgen::feature::features::tree::TreeConfig;
use crate::world::worldgen::feature::foliageplacers::FoliageAttachment;
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util::Axis;
use shulkr::util::BlockPosition;
use shulkr::util::Direction;
use shulkr::world::block::BlockState;

#[derive(Debug, Deserialize)]
pub struct CherryTrunkPlacer {
    pub base_height: i32,
    pub height_rand_a: i32,
    pub height_rand_b: i32,
    pub branch_count: IntProvider,
    pub branch_horizontal_length: IntProvider,
    pub branch_start_offset_from_top: UniformIntProvider,
    pub branch_end_offset_from_top: IntProvider,
}

impl CherryTrunkPlacer {
    const fn second_branch_start_offset_from_top(&self) -> UniformIntProvider {
        self.branch_start_offset_from_top
            .with_max_inclusive(self.branch_start_offset_from_top.max_inclusive - 1)
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors vanilla CherryTrunkPlacer.placeTrunk"
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
        let first_branch_offset_from_origin =
            (tree_height - 1 + self.branch_start_offset_from_top.sample(random)).max(0);
        let mut second_branch_offset_from_origin =
            (tree_height - 1 + self.second_branch_start_offset_from_top().sample(random)).max(0);
        if second_branch_offset_from_origin >= first_branch_offset_from_origin {
            second_branch_offset_from_origin += 1;
        }

        let branch_count = self.branch_count.sample(random);
        let has_middle_branch = branch_count == 3;
        let has_both_side_branches = branch_count >= 2;
        let trunk_height = if has_middle_branch {
            tree_height
        } else if has_both_side_branches {
            first_branch_offset_from_origin.max(second_branch_offset_from_origin) + 1
        } else {
            first_branch_offset_from_origin + 1
        };

        for y in 0..trunk_height {
            placer.place_log(region, trunk_setter, random, origin.above_n(y), config);
        }

        let mut attachments = Vec::new();
        if has_middle_branch {
            attachments.push(FoliageAttachment {
                pos: origin.above_n(trunk_height),
                radius_offset: 0,
                double_trunk: false,
            });
        }

        let tree_direction = HORIZONTAL_DIRECTIONS[random.next_i32_bounded(4) as usize];
        let axis = tree_direction.axis();
        attachments.push(self.generate_branch(
            placer,
            region,
            trunk_setter,
            random,
            tree_height,
            origin,
            config,
            axis,
            tree_direction,
            first_branch_offset_from_origin,
            first_branch_offset_from_origin < trunk_height - 1,
        ));
        if has_both_side_branches {
            attachments.push(self.generate_branch(
                placer,
                region,
                trunk_setter,
                random,
                tree_height,
                origin,
                config,
                axis,
                tree_direction.opposite(),
                second_branch_offset_from_origin,
                second_branch_offset_from_origin < trunk_height - 1,
            ));
        }

        attachments
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors vanilla CherryTrunkPlacer.generateBranch"
    )]
    fn generate_branch(
        &self,
        placer: &super::TrunkPlacer,
        region: &WorldGenRegion<'_>,
        trunk_setter: &mut dyn FnMut(BlockPosition, BlockState),
        random: &mut RandomSource,
        tree_height: i32,
        origin: BlockPosition,
        config: &TreeConfig,
        axis: Axis,
        branch_direction: Direction,
        offset_from_origin: i32,
        middle_continues_upwards: bool,
    ) -> FoliageAttachment {
        let mut log_pos = origin.above_n(offset_from_origin);
        let branch_end_pos_offset_from_origin =
            tree_height - 1 + self.branch_end_offset_from_top.sample(random);
        let extend_branch_away_from_trunk =
            middle_continues_upwards || branch_end_pos_offset_from_origin < offset_from_origin;
        let distance_to_trunk =
            self.branch_horizontal_length.sample(random) + i32::from(extend_branch_away_from_trunk);
        let branch_end_pos = BlockPosition::new(
            origin.x() + branch_direction.offset().0 * distance_to_trunk,
            origin.y() + branch_end_pos_offset_from_origin,
            origin.z() + branch_direction.offset().2 * distance_to_trunk,
        );
        let steps_horizontally = if extend_branch_away_from_trunk { 2 } else { 1 };

        for _ in 0..steps_horizontally {
            log_pos = log_pos.offset(branch_direction.offset().0, 0, branch_direction.offset().2);
            placer.place_log_with(region, trunk_setter, random, log_pos, config, |state| {
                super::set_axis(state, axis)
            });
        }

        let vertical_step = if branch_end_pos.y() > log_pos.y() {
            1
        } else {
            -1
        };

        loop {
            let distance = (branch_end_pos.x() - log_pos.x()).abs()
                + (branch_end_pos.y() - log_pos.y()).abs()
                + (branch_end_pos.z() - log_pos.z()).abs();
            if distance == 0 {
                return FoliageAttachment {
                    pos: branch_end_pos.above(),
                    radius_offset: 0,
                    double_trunk: false,
                };
            }

            let chance_to_grow_vertically =
                (branch_end_pos.y() - log_pos.y()).abs() as f32 / distance as f32;
            let grow_vertically = random.next_f32() < chance_to_grow_vertically;
            log_pos = if grow_vertically {
                log_pos.offset(0, vertical_step, 0)
            } else {
                log_pos.offset(branch_direction.offset().0, 0, branch_direction.offset().2)
            };
            if grow_vertically {
                placer.place_log(region, trunk_setter, random, log_pos, config);
            } else {
                placer.place_log_with(region, trunk_setter, random, log_pos, config, |state| {
                    super::set_axis(state, axis)
                });
            }
        }
    }
}
