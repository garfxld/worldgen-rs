use serde::Deserialize;

use crate::util::rng::RandomSource;
use crate::util::shuffle;
use crate::util::value_providers::int::IntProvider;
use crate::world::worldgen::feature::features::tree::TreeConfig;
use crate::world::worldgen::feature::foliageplacers::FoliageAttachment;
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util::BlockPosition;
use shulkr::util::Direction;
use shulkr::world::block::BlockState;

#[derive(Debug, Deserialize)]
pub struct PoplarTrunkPlacer {
    pub base_height: i32,
    pub height_rand_a: i32,
    pub height_rand_b: i32,
    pub trunk_height_above_branches: IntProvider,
    pub branch_amount: IntProvider,
}

impl PoplarTrunkPlacer {
    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors vanilla PoplarTrunkPlacer.placeTrunk"
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
        let trunk_height_up_to_foliage_branches =
            tree_height - self.trunk_height_above_branches.sample(random);

        for y in 0..tree_height {
            placer.place_log(region, trunk_setter, random, origin.above_n(y), config);
            let directions = shuffled_branch_directions(random);
            if trunk_height_up_to_foliage_branches - 1 == y {
                let branches = self.branch_amount.sample(random);
                for &branch_direction in directions.iter().take(branches as usize) {
                    let axis = branch_direction.get_axis();
                    placer.place_log_with(
                        region,
                        trunk_setter,
                        random,
                        origin.above_n(y).relative(branch_direction),
                        config,
                        |state| super::set_axis(state, axis),
                    );
                }
            }
        }

        vec![FoliageAttachment {
            pos: origin.above_n(trunk_height_up_to_foliage_branches),
            radius_offset: 0,
            double_trunk: false,
        }]
    }
}

fn shuffled_branch_directions(random: &mut RandomSource) -> Vec<Direction> {
    let mut directions = Direction::ALL;
    shuffle(&mut directions, random);
    directions
        .into_iter()
        .filter(|direction| !direction.get_axis().is_vertical())
        .collect()
}
