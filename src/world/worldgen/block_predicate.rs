use glam::IVec3;
use serde::{Deserialize, Deserializer};
use shulkr::util::Key;
use shulkr::world::block::{Block, BlockState};

use crate::util::value_providers::VerticalAnchor;
use crate::world::worldgen::block_survival::can_survive;
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::registry::registries;
use shulkr::util::BlockPosition;
use shulkr::util::Direction;

#[derive(Debug, Clone, Deserialize)]
#[serde(try_from = "RawHolderSet")]
pub enum HolderSet {
    Tag(Key),
    Entries(Vec<Key>),
}
#[derive(Deserialize)]
#[serde(untagged)]
pub enum RawHolderSet {
    One(String),
    Many(Vec<Key>),
}

impl TryFrom<RawHolderSet> for HolderSet {
    type Error = &'static str;

    fn try_from(raw: RawHolderSet) -> Result<Self, Self::Error> {
        Ok(match raw {
            RawHolderSet::One(entry) => match entry.strip_prefix('#') {
                Some(tag) => Self::Tag(Key::of(tag.to_owned())),
                None => Self::Entries(vec![Key::of(entry)]),
            },
            RawHolderSet::Many(entries) => Self::Entries(entries),
        })
    }
}

impl HolderSet {
    pub fn contains_block(&self, state: BlockState) -> bool {
        let block = state.as_block();
        match self {
            Self::Tag(tag) => block.has_tag(tag),
            Self::Entries(keys) => keys.contains(block.key()),
        }
    }

    pub fn blocks(&self) -> Vec<&'static Block> {
        match self {
            Self::Tag(tag) => registries::BLOCK
                .iter_tag(tag)
                .map(|(_, block)| block)
                .collect(),
            Self::Entries(keys) => keys
                .iter()
                .map(|key| {
                    Block::from_key(key.clone()).unwrap_or_else(|| panic!("unknown block {key}"))
                })
                .collect(),
        }
    }

    pub fn contains_biome(&self, key: &Key) -> bool {
        match self {
            Self::Tag(tag) => registries::BIOME.has_tag(key, tag),
            Self::Entries(keys) => keys.contains(key),
        }
    }

    fn contains_fluid(&self, state: BlockState) -> bool {
        let fluid = state.get_fluid_state().fluid();
        match self {
            Self::Tag(tag) => fluid.has_tag(tag),
            Self::Entries(keys) => keys.contains(fluid.key()),
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
        tag: Key,
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
    #[serde(rename = "minecraft:matching_biomes")]
    MatchingBiomes { biomes: HolderSet },
    #[serde(rename = "minecraft:unobstructed")]
    Unobstructed {
        #[serde(default, deserialize_with = "offset")]
        offset: IVec3,
    },
    #[serde(rename = "minecraft:height_range")]
    HeightRange {
        min_inclusive: VerticalAnchor,
        max_inclusive: VerticalAnchor,
    },
    #[serde(rename = "minecraft:volume_match")]
    VolumeMatch {
        #[serde(deserialize_with = "offset")]
        min: IVec3,
        #[serde(deserialize_with = "offset")]
        max: IVec3,
        #[serde(rename = "match")]
        predicate: Box<BlockPredicate>,
    },
}

fn offset<'de, D: Deserializer<'de>>(deserializer: D) -> Result<IVec3, D::Error> {
    let [x, y, z] = <[i32; 3]>::deserialize(deserializer)?;
    Ok(IVec3::new(x, y, z))
}

impl BlockPredicate {
    pub fn test(&self, region: &WorldGenRegion, origin: BlockPosition) -> bool {
        match self {
            Self::AlwaysTrue => true,
            Self::MatchingBlocks { offset, blocks } => {
                blocks.contains_block(state_at(region, origin, *offset))
            }
            Self::MatchingBlockTag { offset, tag } => {
                state_at(region, origin, *offset).as_block().has_tag(tag)
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
                region.is_inside_build_height(offset_pos(origin, *offset).y())
            }
            Self::HasSturdyFace { offset, direction } => {
                let pos = offset_pos(origin, *offset);
                region.get_block(pos).is_side_solid_at(pos, *direction)
            }
            Self::Not { predicate } => !predicate.test(region, origin),
            Self::AllOf { predicates } => predicates.iter().all(|p| p.test(region, origin)),
            Self::AnyOf { predicates } => predicates.iter().any(|p| p.test(region, origin)),
            Self::MatchingBiomes { biomes } => {
                let id = region.get_biome(origin);
                registries::BIOME
                    .by_id(id)
                    .is_some_and(|biome| biomes.contains_biome(biome.key()))
            }
            Self::Unobstructed { .. } => true,
            Self::HeightRange {
                min_inclusive,
                max_inclusive,
            } => {
                let context = region.worldgen_context();
                origin.y() >= min_inclusive.resolve_y(&context)
                    && origin.y() <= max_inclusive.resolve_y(&context)
            }
            Self::VolumeMatch {
                min,
                max,
                predicate,
            } => {
                for ox in min.x..=max.x {
                    for oz in min.z..=max.z {
                        for oy in min.y..=max.y {
                            if !predicate.test(region, offset_pos(origin, IVec3::new(ox, oy, oz))) {
                                return false;
                            }
                        }
                    }
                }
                true
            }
        }
    }
}

const fn offset_pos(origin: BlockPosition, offset: IVec3) -> BlockPosition {
    BlockPosition::new(
        origin.x() + offset.x,
        origin.y() + offset.y,
        origin.z() + offset.z,
    )
}

fn state_at(region: &WorldGenRegion, origin: BlockPosition, offset: IVec3) -> BlockState {
    region.get_block(offset_pos(origin, offset))
}
