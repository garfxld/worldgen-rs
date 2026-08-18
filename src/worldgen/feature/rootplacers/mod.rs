pub mod mangrove;

use serde::Deserialize;

use crate::util::ident::BlockState;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::worldgen::feature::features::tree::{TreeConfig, valid_tree_pos};
use crate::worldgen::feature::state_provider::BlockStateProvider;
use crate::worldgen::region::WorldGenRegion;

use mangrove::MangroveRootPlacer;

#[derive(Debug, Deserialize)]
pub struct AboveRootPlacement {
    pub above_root_provider: BlockStateProvider,
    pub above_root_placement_chance: f32,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum RootPlacer {
    #[serde(rename = "minecraft:mangrove_root_placer")]
    Mangrove(Box<MangroveRootPlacer>),
}

impl RootPlacer {
    pub fn get_trunk_origin(&self, origin: BlockPos, random: &mut RandomSource) -> BlockPos {
        match self {
            Self::Mangrove(p) => origin.above_n(p.trunk_offset_y.sample(random)),
        }
    }

    pub fn place_roots(
        &self,
        region: &WorldGenRegion<'_>,
        root_setter: &mut dyn FnMut(BlockPos, BlockState),
        random: &mut RandomSource,
        origin: BlockPos,
        trunk_origin: BlockPos,
        config: &TreeConfig,
    ) -> bool {
        match self {
            Self::Mangrove(p) => p.place_roots(
                self,
                region,
                root_setter,
                random,
                origin,
                trunk_origin,
                config,
            ),
        }
    }

    fn can_place_root(&self, region: &WorldGenRegion<'_>, pos: BlockPos) -> bool {
        match self {
            Self::Mangrove(p) => {
                valid_tree_pos(region, pos)
                    || p.mangrove_root_placement
                        .can_grow_through
                        .contains_block(region.get_block(pos))
            }
        }
    }

    fn place_root(
        &self,
        region: &WorldGenRegion<'_>,
        root_setter: &mut dyn FnMut(BlockPos, BlockState),
        random: &mut RandomSource,
        pos: BlockPos,
        _config: &TreeConfig,
    ) {
        let (root_provider, above_root_placement) = match self {
            Self::Mangrove(p) => (&p.root_provider, &p.above_root_placement),
        };
        if self.can_place_root(region, pos) {
            let state = root_provider.get_state(region, random, pos);
            root_setter(pos, potentially_waterlogged_state(region, pos, state));
            if let Some(above_placement) = above_root_placement {
                let above = pos.above();
                if random.next_f32() < above_placement.above_root_placement_chance
                    && region.get_block(above).is_air()
                {
                    let state = above_placement
                        .above_root_provider
                        .get_state(region, random, above);
                    root_setter(above, potentially_waterlogged_state(region, above, state));
                }
            }
        }
    }
}

fn potentially_waterlogged_state(
    region: &WorldGenRegion<'_>,
    pos: BlockPos,
    state: BlockState,
) -> BlockState {
    use crate::registry::blocks::properties::Properties;
    if state.try_get_value(&Properties::WATERLOGGED).is_some() {
        let waterlogged = region.get_block(pos).get_fluid_state().is_water();
        state.set_value(&Properties::WATERLOGGED, waterlogged)
    } else {
        state
    }
}
