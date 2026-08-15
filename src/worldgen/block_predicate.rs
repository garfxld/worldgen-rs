use glam::IVec3;
use serde_json::Value;

use crate::registry::REGISTRY;
use crate::util::direction::Direction;
use crate::util::ident::{BlockState, Identifier};
use crate::util::pos::BlockPos;
use crate::worldgen::block_survival::can_survive;
use crate::worldgen::json::{
    parse_block_state, parse_direction, parse_id, parse_offset, type_of, value_list,
};
use crate::worldgen::region::WorldGenRegion;

#[derive(Debug, Clone)]
pub enum HolderSet {
    Tag(Identifier),
    Entries(Vec<Identifier>),
}

impl HolderSet {
    pub fn parse(value: &Value) -> Self {
        match value {
            Value::String(raw) => match raw.strip_prefix('#') {
                Some(tag) => Self::Tag(parse_id(tag)),
                None => Self::Entries(vec![parse_id(raw)]),
            },
            Value::Array(entries) => Self::Entries(
                entries
                    .iter()
                    .filter_map(Value::as_str)
                    .map(parse_id)
                    .collect(),
            ),
            _ => Self::Entries(Vec::new()),
        }
    }

    fn contains_block(&self, state: BlockState) -> bool {
        let block = state.get_block();
        match self {
            Self::Tag(tag) => block.has_tag(tag),
            Self::Entries(keys) => keys.contains(&block.key),
        }
    }

    fn contains_fluid(&self, state: BlockState) -> bool {
        let fluid = state.get_fluid_state().fluid_id;
        match self {
            Self::Tag(tag) => REGISTRY.fluids.is_in_tag(&fluid.key, tag),
            Self::Entries(keys) => keys.contains(&fluid.key),
        }
    }
}

#[derive(Debug, Clone)]
pub enum BlockPredicate {
    AlwaysTrue,
    MatchingBlocks { offset: IVec3, blocks: HolderSet },
    MatchingBlockTag { offset: IVec3, tag: Identifier },
    MatchingFluids { offset: IVec3, fluids: HolderSet },
    Solid { offset: IVec3 },
    Replaceable { offset: IVec3 },
    WouldSurvive { offset: IVec3, state: BlockState },
    InsideWorldBounds { offset: IVec3 },
    HasSturdyFace { offset: IVec3, direction: Direction },
    Not(Box<BlockPredicate>),
    AllOf(Vec<BlockPredicate>),
    AnyOf(Vec<BlockPredicate>),
}

impl BlockPredicate {
    pub fn parse(value: &Value) -> Self {
        let offset = parse_offset(value);
        match type_of(value) {
            "minecraft:true" | "minecraft:always_true" => Self::AlwaysTrue,
            "minecraft:matching_blocks" => Self::MatchingBlocks {
                offset,
                blocks: HolderSet::parse(&value["blocks"]),
            },
            "minecraft:matching_block_tag" => Self::MatchingBlockTag {
                offset,
                tag: parse_id(value["tag"].as_str().unwrap_or_default()),
            },
            "minecraft:matching_fluids" => Self::MatchingFluids {
                offset,
                fluids: HolderSet::parse(&value["fluids"]),
            },
            "minecraft:solid" => Self::Solid { offset },
            "minecraft:replaceable" => Self::Replaceable { offset },
            "minecraft:would_survive" => Self::WouldSurvive {
                offset,
                state: parse_block_state(&value["state"]),
            },
            "minecraft:inside_world_bounds" => Self::InsideWorldBounds { offset },
            "minecraft:has_sturdy_face" => Self::HasSturdyFace {
                offset,
                direction: parse_direction(value.get("direction")),
            },
            "minecraft:not" => Self::Not(Box::new(Self::parse(&value["predicate"]))),
            "minecraft:all_of" => Self::AllOf(
                value_list(&value["predicates"])
                    .iter()
                    .map(Self::parse)
                    .collect(),
            ),
            "minecraft:any_of" => Self::AnyOf(
                value_list(&value["predicates"])
                    .iter()
                    .map(Self::parse)
                    .collect(),
            ),
            other => panic!("unknown block predicate type {other}"),
        }
    }

    pub fn test(&self, region: &WorldGenRegion, origin: BlockPos) -> bool {
        match self {
            Self::AlwaysTrue => true,
            Self::MatchingBlocks { offset, blocks } => {
                blocks.contains_block(state_at(region, origin, *offset))
            }
            Self::MatchingBlockTag { offset, tag } => {
                state_at(region, origin, *offset).get_block().has_tag(tag)
            }
            Self::MatchingFluids { offset, fluids } => {
                fluids.contains_fluid(state_at(region, origin, *offset))
            }
            Self::Solid { offset } => state_at(region, origin, *offset).is_solid(),
            Self::Replaceable { offset } => state_at(region, origin, *offset).is_replaceable(),
            Self::WouldSurvive { offset, state } => {
                can_survive(region, offset_pos(origin, *offset), *state)
            }
            Self::InsideWorldBounds { offset } => {
                region.is_inside_build_height(offset_pos(origin, *offset).0.y)
            }
            Self::HasSturdyFace { offset, direction } => {
                let pos = offset_pos(origin, *offset);
                region
                    .get_block_state(pos)
                    .is_face_sturdy_at(pos, *direction)
            }
            Self::Not(inner) => !inner.test(region, origin),
            Self::AllOf(predicates) => predicates.iter().all(|p| p.test(region, origin)),
            Self::AnyOf(predicates) => predicates.iter().any(|p| p.test(region, origin)),
        }
    }
}

const fn offset_pos(origin: BlockPos, offset: IVec3) -> BlockPos {
    BlockPos::new(
        origin.0.x + offset.x,
        origin.0.y + offset.y,
        origin.0.z + offset.z,
    )
}

fn state_at(region: &WorldGenRegion, origin: BlockPos, offset: IVec3) -> BlockState {
    region.get_block_state(offset_pos(origin, offset))
}
