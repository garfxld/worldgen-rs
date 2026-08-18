use std::collections::BTreeMap;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer};

use crate::registry::REGISTRY;
use crate::util::ident::{BlockState, Identifier};
use crate::util::pos::BlockPos;
use crate::worldgen::block_predicate::HolderSet;
use crate::worldgen::region::WorldGenRegion;

#[derive(Debug, Deserialize)]
pub struct SpringConfig {
    #[serde(deserialize_with = "fluid_as_block_state")]
    pub state: BlockState,
    #[serde(default = "requires_block_below_default")]
    pub requires_block_below: bool,
    #[serde(default = "rock_count_default")]
    pub rock_count: i32,
    #[serde(default = "hole_count_default")]
    pub hole_count: i32,
    pub valid_blocks: HolderSet,
}

const fn requires_block_below_default() -> bool {
    true
}

const fn rock_count_default() -> i32 {
    4
}

const fn hole_count_default() -> i32 {
    1
}

fn fluid_as_block_state<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BlockState, D::Error> {
    #[derive(Deserialize)]
    struct FluidState {
        #[serde(rename = "Name")]
        name: Identifier,
        #[serde(rename = "Properties", default)]
        #[expect(
            dead_code,
            reason = "a source fluid's properties do not affect its legacy block"
        )]
        properties: BTreeMap<String, String>,
    }

    let FluidState { name, .. } = FluidState::deserialize(deserializer)?;
    let block = REGISTRY
        .blocks
        .by_key(&name)
        .ok_or_else(|| D::Error::custom(format!("unknown fluid block {name}")))?;
    Ok(REGISTRY.blocks.get_default_state_id(block))
}

pub fn place(region: &WorldGenRegion<'_>, origin: BlockPos, config: &SpringConfig) -> bool {
    if !is_valid(region, config, origin.above()) {
        return false;
    }
    if config.requires_block_below && !is_valid(region, config, origin.below()) {
        return false;
    }

    let current = region.get_block(origin);
    if !current.is_air() && !config.valid_blocks.contains_block(current) {
        return false;
    }

    let mut rock_count = 0;
    for neighbor in [
        origin.west(),
        origin.east(),
        origin.north(),
        origin.south(),
        origin.below(),
    ] {
        if is_valid(region, config, neighbor) {
            rock_count += 1;
        }
    }

    let mut hole_count = 0;
    for neighbor in [
        origin.west(),
        origin.east(),
        origin.north(),
        origin.south(),
        origin.below(),
    ] {
        if region.get_block(neighbor).is_air() {
            hole_count += 1;
        }
    }

    if rock_count == config.rock_count && hole_count == config.hole_count {
        region.set_block(origin, config.state);
        return true;
    }

    false
}

fn is_valid(region: &WorldGenRegion<'_>, config: &SpringConfig, pos: BlockPos) -> bool {
    config.valid_blocks.contains_block(region.get_block(pos))
}
