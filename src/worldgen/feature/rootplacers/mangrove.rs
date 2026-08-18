use serde::Deserialize;

use crate::util::direction::Direction;
use crate::util::ident::BlockState;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::util::value_providers::int::IntProvider;
use crate::worldgen::block_predicate::HolderSet;
use crate::worldgen::feature::features::HORIZONTAL_DIRECTIONS;
use crate::worldgen::feature::features::tree::TreeConfig;
use crate::worldgen::feature::rootplacers::{AboveRootPlacement, RootPlacer};
use crate::worldgen::feature::state_provider::BlockStateProvider;
use crate::worldgen::region::WorldGenRegion;

#[derive(Debug, Deserialize)]
pub struct MangroveRootPlacement {
    pub can_grow_through: HolderSet,
    pub muddy_roots_in: HolderSet,
    pub muddy_roots_provider: BlockStateProvider,
    pub max_root_width: i32,
    pub max_root_length: i32,
    pub random_skew_chance: f32,
}

#[derive(Debug, Deserialize)]
pub struct MangroveRootPlacer {
    pub trunk_offset_y: IntProvider,
    pub root_provider: BlockStateProvider,
    #[serde(default)]
    pub above_root_placement: Option<AboveRootPlacement>,
    pub mangrove_root_placement: MangroveRootPlacement,
}

impl MangroveRootPlacer {
    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors vanilla MangroveRootPlacer.placeRoots"
    )]
    pub fn place_roots(
        &self,
        placer: &RootPlacer,
        region: &WorldGenRegion<'_>,
        root_setter: &mut dyn FnMut(BlockPos, BlockState),
        random: &mut RandomSource,
        origin: BlockPos,
        trunk_origin: BlockPos,
        config: &TreeConfig,
    ) -> bool {
        let mut root_positions: Vec<BlockPos> = Vec::new();
        let mut column_pos = origin;
        while column_pos.y() < trunk_origin.y() {
            if !placer.can_place_root(region, column_pos) {
                return false;
            }
            column_pos = column_pos.above();
        }

        root_positions.push(trunk_origin.below());

        for dir in HORIZONTAL_DIRECTIONS {
            let pos = trunk_origin.offset(dir.offset().0, 0, dir.offset().2);
            let mut positions_in_direction = Vec::new();
            if !self.simulate_roots(
                placer,
                region,
                random,
                pos,
                dir,
                trunk_origin,
                &mut positions_in_direction,
                0,
            ) {
                return false;
            }

            root_positions.extend(positions_in_direction);
            root_positions.push(pos);
        }

        for root_pos in root_positions {
            self.place_root(placer, region, root_setter, random, root_pos, config);
        }

        true
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors vanilla MangroveRootPlacer.simulateRoots"
    )]
    fn simulate_roots(
        &self,
        placer: &RootPlacer,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        root_pos: BlockPos,
        dir: Direction,
        root_origin: BlockPos,
        root_positions: &mut Vec<BlockPos>,
        layer: i32,
    ) -> bool {
        let max_root_length = self.mangrove_root_placement.max_root_length;
        if layer == max_root_length || root_positions.len() as i32 > max_root_length {
            return false;
        }

        for pos in self.potential_root_positions(root_pos, dir, random, root_origin) {
            if placer.can_place_root(region, pos) {
                root_positions.push(pos);
                if !self.simulate_roots(
                    placer,
                    region,
                    random,
                    pos,
                    dir,
                    root_origin,
                    root_positions,
                    layer + 1,
                ) {
                    return false;
                }
            }
        }

        true
    }

    fn potential_root_positions(
        &self,
        pos: BlockPos,
        prev_dir: Direction,
        random: &mut RandomSource,
        root_origin: BlockPos,
    ) -> Vec<BlockPos> {
        let below = pos.below();
        let next_to = pos.offset(prev_dir.offset().0, 0, prev_dir.offset().2);
        let width = (pos.x() - root_origin.x()).abs()
            + (pos.y() - root_origin.y()).abs()
            + (pos.z() - root_origin.z()).abs();
        let max_root_width = self.mangrove_root_placement.max_root_width;
        let random_skew_chance = self.mangrove_root_placement.random_skew_chance;
        #[expect(
            clippy::if_same_then_else,
            reason = "branches match but each arm gates a different RNG draw, matching vanilla MangroveRootPlacer.potentialRootPositions"
        )]
        if width > max_root_width - 3 && width <= max_root_width {
            if random.next_f32() < random_skew_chance {
                vec![below, next_to.below()]
            } else {
                vec![below]
            }
        } else if width > max_root_width {
            vec![below]
        } else if random.next_f32() < random_skew_chance {
            vec![below]
        } else if random.next_bool() {
            vec![next_to]
        } else {
            vec![below]
        }
    }

    fn place_root(
        &self,
        placer: &RootPlacer,
        region: &WorldGenRegion<'_>,
        root_setter: &mut dyn FnMut(BlockPos, BlockState),
        random: &mut RandomSource,
        pos: BlockPos,
        config: &TreeConfig,
    ) {
        if self
            .mangrove_root_placement
            .muddy_roots_in
            .contains_block(region.get_block(pos))
        {
            let muddy_roots = self
                .mangrove_root_placement
                .muddy_roots_provider
                .get_state(region, random, pos);
            root_setter(
                pos,
                super::potentially_waterlogged_state(region, pos, muddy_roots),
            );
        } else {
            placer.place_root(region, root_setter, random, pos, config);
        }
    }
}
