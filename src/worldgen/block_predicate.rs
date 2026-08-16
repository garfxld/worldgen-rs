use glam::IVec3;
use serde::{Deserialize, Deserializer};

use crate::registry::REGISTRY;
use crate::util::direction::Direction;
use crate::util::ident::{BlockState, Identifier};
use crate::util::pos::BlockPos;
use crate::worldgen::block_survival::can_survive;
use crate::worldgen::region::WorldGenRegion;

#[derive(Debug, Clone, Deserialize)]
#[serde(try_from = "RawHolderSet")]
pub enum HolderSet {
    Tag(Identifier),
    Entries(Vec<Identifier>),
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum RawHolderSet {
    One(String),
    Many(Vec<Identifier>),
}

impl TryFrom<RawHolderSet> for HolderSet {
    type Error = &'static str;

    fn try_from(raw: RawHolderSet) -> Result<Self, Self::Error> {
        Ok(match raw {
            RawHolderSet::One(entry) => match entry.strip_prefix('#') {
                Some(tag) => Self::Tag(tag.parse()?),
                None => Self::Entries(vec![entry.parse()?]),
            },
            RawHolderSet::Many(entries) => Self::Entries(entries),
        })
    }
}

impl HolderSet {
    pub fn contains_block(&self, state: BlockState) -> bool {
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

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum BlockPredicate {
    #[serde(rename = "minecraft:true", alias = "minecraft:always_true")]
    AlwaysTrue,
    #[serde(rename = "minecraft:matching_blocks")]
    MatchingBlocks {
        #[serde(default, deserialize_with = "offset")]
        offset: IVec3,
        blocks: HolderSet,
    },
    #[serde(rename = "minecraft:matching_block_tag")]
    MatchingBlockTag {
        #[serde(default, deserialize_with = "offset")]
        offset: IVec3,
        tag: Identifier,
    },
    #[serde(rename = "minecraft:matching_fluids")]
    MatchingFluids {
        #[serde(default, deserialize_with = "offset")]
        offset: IVec3,
        fluids: HolderSet,
    },
    #[serde(rename = "minecraft:solid")]
    Solid {
        #[serde(default, deserialize_with = "offset")]
        offset: IVec3,
    },
    #[serde(rename = "minecraft:replaceable")]
    Replaceable {
        #[serde(default, deserialize_with = "offset")]
        offset: IVec3,
    },
    #[serde(rename = "minecraft:would_survive")]
    WouldSurvive {
        #[serde(default, deserialize_with = "offset")]
        offset: IVec3,
        state: BlockState,
    },
    #[serde(rename = "minecraft:inside_world_bounds")]
    InsideWorldBounds {
        #[serde(default, deserialize_with = "offset")]
        offset: IVec3,
    },
    #[serde(rename = "minecraft:has_sturdy_face")]
    HasSturdyFace {
        #[serde(default, deserialize_with = "offset")]
        offset: IVec3,
        direction: Direction,
    },
    #[serde(rename = "minecraft:not")]
    Not { predicate: Box<BlockPredicate> },
    #[serde(rename = "minecraft:all_of")]
    AllOf { predicates: Vec<BlockPredicate> },
    #[serde(rename = "minecraft:any_of")]
    AnyOf { predicates: Vec<BlockPredicate> },
}

fn offset<'de, D: Deserializer<'de>>(deserializer: D) -> Result<IVec3, D::Error> {
    let [x, y, z] = <[i32; 3]>::deserialize(deserializer)?;
    Ok(IVec3::new(x, y, z))
}

impl BlockPredicate {
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
                region.get_block(pos).is_face_sturdy_at(pos, *direction)
            }
            Self::Not { predicate } => !predicate.test(region, origin),
            Self::AllOf { predicates } => predicates.iter().all(|p| p.test(region, origin)),
            Self::AnyOf { predicates } => predicates.iter().any(|p| p.test(region, origin)),
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
    region.get_block(offset_pos(origin, offset))
}
