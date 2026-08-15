use glam::IVec3;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::chunk::heightmap::HeightmapType;
use crate::registry::REGISTRY;
use crate::util::direction::Direction;
use crate::util::ident::{BlockState, Identifier};

const EMPTY_LIST: &[Value] = &[];

pub fn parse_id(raw: &str) -> Identifier {
    raw.parse()
        .unwrap_or_else(|e| panic!("invalid identifier {raw}: {e}"))
}

pub fn type_of(value: &Value) -> &str {
    value["type"]
        .as_str()
        .unwrap_or_else(|| panic!("missing type field in {value}"))
}

pub fn value_list(value: &Value) -> &[Value] {
    value.as_array().map_or(EMPTY_LIST, Vec::as_slice)
}

pub fn parse_offset(value: &Value) -> IVec3 {
    let Some(offset) = value.get("offset").and_then(Value::as_array) else {
        return IVec3::ZERO;
    };
    let component = |index: usize| {
        offset
            .get(index)
            .and_then(Value::as_i64)
            .unwrap_or_default() as i32
    };
    IVec3::new(component(0), component(1), component(2))
}

pub fn parse_direction(value: Option<&Value>) -> Direction {
    match value.and_then(Value::as_str).unwrap_or("down") {
        "down" => Direction::Down,
        "up" => Direction::Up,
        "north" => Direction::North,
        "south" => Direction::South,
        "west" => Direction::West,
        "east" => Direction::East,
        other => panic!("unknown direction {other}"),
    }
}

pub fn parse_heightmap_type(value: &Value) -> HeightmapType {
    match value.as_str().unwrap_or_default() {
        "WORLD_SURFACE" => HeightmapType::WorldSurface,
        "WORLD_SURFACE_WG" => HeightmapType::WorldSurfaceWg,
        "MOTION_BLOCKING" => HeightmapType::MotionBlocking,
        "MOTION_BLOCKING_NO_LEAVES" => HeightmapType::MotionBlockingNoLeaves,
        "OCEAN_FLOOR" => HeightmapType::OceanFloor,
        "OCEAN_FLOOR_WG" => HeightmapType::OceanFloorWg,
        other => panic!("unknown heightmap type {other}"),
    }
}

pub fn parse_block_state(value: &Value) -> BlockState {
    let name = value["Name"]
        .as_str()
        .unwrap_or_else(|| panic!("block state without Name: {value}"));
    let key = parse_id(name);
    let block = REGISTRY
        .blocks
        .by_key(&key)
        .unwrap_or_else(|| panic!("unknown block {key}"));

    let Some(properties) = value.get("Properties").and_then(Value::as_object) else {
        return REGISTRY.blocks.get_default_state_id(block);
    };

    let overrides: Vec<(&str, &str)> = properties
        .iter()
        .map(|(name, raw)| {
            let raw = raw
                .as_str()
                .unwrap_or_else(|| panic!("non-string property {name} on {key}"));
            (name.as_str(), raw)
        })
        .collect();

    REGISTRY
        .blocks
        .state_id_from_block_defaulted_properties(block, overrides.iter().copied())
        .unwrap_or_else(|| panic!("invalid properties {overrides:?} for {key}"))
}

pub fn parse_json<T: DeserializeOwned>(value: &Value) -> T {
    serde_json::from_value(value.clone()).unwrap_or_else(|e| panic!("invalid json {value}: {e}"))
}
