use serde::Deserialize;

use crate::util::rng::RandomSource;
use crate::util::value_providers::int::IntProvider;
use crate::world::worldgen::block_predicate::BlockPredicate;
use crate::world::worldgen::feature::state_provider::ProviderHolder;
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util::BlockPosition;
use shulkr::util::Direction;

#[derive(Debug, Deserialize)]
pub struct Layer {
    pub height: IntProvider,
    #[serde(rename = "provider")]
    pub state: ProviderHolder,
}

#[derive(Debug, Deserialize)]
pub struct BlockColumnConfig {
    pub layers: Vec<Layer>,
    pub direction: Direction,
    pub allowed_placement: BlockPredicate,
    pub prioritize_tip: bool,
}

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPosition,
    config: &BlockColumnConfig,
) -> bool {
    let mut layer_heights: Vec<i32> = Vec::with_capacity(config.layers.len());
    let mut total_height = 0;

    for layer in &config.layers {
        let height = layer.height.sample(random);
        layer_heights.push(height);
        total_height += height;
    }

    if total_height == 0 {
        return false;
    }

    let mut place_pos = origin;
    let mut next_pos = place_pos.relative(config.direction);

    for y in 0..total_height {
        if !config.allowed_placement.test(region, next_pos) {
            truncate(&mut layer_heights, total_height, y, config.prioritize_tip);
            break;
        }
        next_pos = next_pos.relative(config.direction);
    }

    for (index, layer) in config.layers.iter().enumerate() {
        for _ in 0..layer_heights[index] {
            let state = layer.state.get_state(region, random, place_pos);
            region.set_block(place_pos, state);
            place_pos = place_pos.relative(config.direction);
        }
    }

    true
}

fn truncate(layer_heights: &mut [i32], total_height: i32, new_height: i32, prioritize_tip: bool) {
    let mut amount_to_remove = total_height - new_height;
    let indices: Vec<usize> = if prioritize_tip {
        (0..layer_heights.len()).collect()
    } else {
        (0..layer_heights.len()).rev().collect()
    };

    for index in indices {
        if amount_to_remove <= 0 {
            break;
        }
        let to_remove = layer_heights[index].min(amount_to_remove);
        amount_to_remove -= to_remove;
        layer_heights[index] -= to_remove;
    }
}
