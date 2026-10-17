pub mod mangrove;

use serde::Deserialize;

use crate::util::rng::RandomSource;
use crate::world::worldgen::feature::features::tree::{TreeConfig, valid_tree_pos};
use crate::world::worldgen::feature::state_provider::ProviderHolder;
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util::BlockPosition;
use shulkr::world::block::BlockState;

use mangrove::MangroveRootPlacer;

#[derive(Debug, Deserialize)]
pub struct AboveRootPlacement {
    pub above_root_provider: ProviderHolder,
    pub above_root_placement_chance: f32,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum RootPlacer {
    #[serde(rename = "minecraft:mangrove_root_placer")]
    Mangrove(Box<MangroveRootPlacer>),
}

impl RootPlacer {
    pub fn get_trunk_origin(
        &self,
        origin: BlockPosition,
        random: &mut RandomSource,
    ) -> BlockPosition {
        match self {
            Self::Mangrove(p) => origin.above_n(p.trunk_offset_y.sample(random)),
        }
    }

    pub fn place_roots(
        &self,
        region: &WorldGenRegion<'_>,
        root_setter: &mut dyn FnMut(BlockPosition, BlockState),
        random: &mut RandomSource,
        origin: BlockPosition,
        trunk_origin: BlockPosition,
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

    fn can_place_root(&self, region: &WorldGenRegion<'_>, pos: BlockPosition) -> bool {
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
        root_setter: &mut dyn FnMut(BlockPosition, BlockState),
        random: &mut RandomSource,
        pos: BlockPosition,
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
    pos: BlockPosition,
    state: BlockState,
) -> BlockState {
    use shulkr::world::block::Properties;
    if state.get(Properties::WATERLOGGED).is_some() {
        let waterlogged = region.get_block(pos).get_fluid_state().is_water();
        state.with(Properties::WATERLOGGED, waterlogged)
    } else {
        state
    }
}
