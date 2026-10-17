use serde::Deserialize;
use shulkr::util::Key;

use crate::datapack;
use crate::datapack::lenient_or_default;
use crate::util::rng::legacy_random::LegacyRandomSource;
use crate::util::rng::{RandomSource, get_seed};
use crate::util::shuffle;
use crate::util::value_providers::int::IntProvider;
use crate::world::chunk::heightmap::HeightmapType;
use crate::world::worldgen::block_predicate::HolderSet;
use crate::world::worldgen::feature::rule_test::RuleTest;
use crate::world::worldgen::region::WorldGenRegion;
use crate::world::worldgen::structure::templatesystem::structure_place_settings::StructurePlaceSettings;
use crate::world::worldgen::structure::templatesystem::structure_template::StructureBlockInfo;
use shulkr::util as shapes;
use shulkr::util::Axis;
use shulkr::util::BlockPosition;
use shulkr::world::block::Block;
use shulkr::world::block::BlockState;
use shulkr::world::block::{Half, Properties};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(tag = "predicate_type")]
pub enum PosRuleTest {
    #[default]
    #[serde(rename = "minecraft:always_true")]
    AlwaysTrue,
    #[serde(rename = "minecraft:linear_pos")]
    LinearPos {
        #[serde(default)]
        min_chance: f32,
        #[serde(default)]
        max_chance: f32,
        #[serde(default)]
        min_dist: i32,
        #[serde(default)]
        max_dist: i32,
    },
    #[serde(rename = "minecraft:axis_aligned_linear_pos")]
    AxisAlignedLinearPos {
        #[serde(default)]
        min_chance: f32,
        #[serde(default)]
        max_chance: f32,
        #[serde(default)]
        min_dist: i32,
        #[serde(default)]
        max_dist: i32,
        #[serde(default = "default_axis")]
        axis: Axis,
    },
}

const fn default_axis() -> Axis {
    Axis::Y
}

impl PosRuleTest {
    pub fn test(
        &self,
        in_template_pos: BlockPosition,
        world_pos: BlockPosition,
        world_reference: BlockPosition,
        random: &mut RandomSource,
    ) -> bool {
        let _ = in_template_pos;
        match self {
            Self::AlwaysTrue => true,
            Self::LinearPos {
                min_chance,
                max_chance,
                min_dist,
                max_dist,
            } => {
                let dist = (world_pos.x() - world_reference.x()).abs()
                    + (world_pos.y() - world_reference.y()).abs()
                    + (world_pos.z() - world_reference.z()).abs();
                chance_test(random, dist, *min_dist, *max_dist, *min_chance, *max_chance)
            }
            Self::AxisAlignedLinearPos {
                min_chance,
                max_chance,
                min_dist,
                max_dist,
                axis,
            } => {
                let delta = match axis {
                    Axis::X => world_pos.x() - world_reference.x(),
                    Axis::Y => world_pos.y() - world_reference.y(),
                    Axis::Z => world_pos.z() - world_reference.z(),
                };
                chance_test(
                    random,
                    delta.abs(),
                    *min_dist,
                    *max_dist,
                    *min_chance,
                    *max_chance,
                )
            }
        }
    }
}

fn chance_test(
    random: &mut RandomSource,
    dist: i32,
    min_dist: i32,
    max_dist: i32,
    min_chance: f32,
    max_chance: f32,
) -> bool {
    let delta = (dist - min_dist) as f32 / (max_dist - min_dist) as f32;
    let chance = if delta < 0.0 {
        min_chance
    } else if delta > 1.0 {
        max_chance
    } else {
        min_chance + delta * (max_chance - min_chance)
    };
    random.next_f32() <= chance
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProcessorRule {
    pub input_predicate: RuleTest,
    pub location_predicate: RuleTest,
    #[serde(default, deserialize_with = "lenient_or_default")]
    pub position_predicate: PosRuleTest,
    pub output_state: BlockState,
}

impl ProcessorRule {
    pub fn test(
        &self,
        region: &WorldGenRegion<'_>,
        input_state: BlockState,
        in_template_pos: BlockPosition,
        world_pos: BlockPosition,
        reference: BlockPosition,
        random: &mut RandomSource,
    ) -> bool {
        self.input_predicate.test(input_state, world_pos, random)
            && self
                .location_predicate
                .test(region.get_block(world_pos), world_pos, random)
            && self
                .position_predicate
                .test(in_template_pos, world_pos, reference, random)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "processor_type")]
pub enum StructureProcessor {
    #[serde(rename = "minecraft:nop")]
    Nop,
    #[serde(rename = "minecraft:block_ignore")]
    BlockIgnore { blocks: HolderSet },
    #[serde(rename = "minecraft:block_rot")]
    BlockRot {
        rottable_blocks: Option<HolderSet>,
        integrity: f32,
    },
    #[serde(rename = "minecraft:protected_blocks")]
    ProtectedBlocks { value: HolderSet },
    #[serde(rename = "minecraft:rule")]
    Rule { rules: Vec<ProcessorRule> },
    #[serde(rename = "minecraft:capped")]
    Capped {
        delegate: Box<StructureProcessor>,
        limit: IntProvider,
    },
    #[serde(rename = "minecraft:gravity")]
    Gravity {
        #[serde(default = "default_gravity_heightmap")]
        heightmap: HeightmapType,
        #[serde(default)]
        offset: i32,
    },
    #[serde(rename = "minecraft:jigsaw_replacement")]
    JigsawReplacement,
    #[serde(rename = "minecraft:block_age")]
    BlockAge { mossiness: f32 },
    #[serde(rename = "minecraft:lava_submerged_block")]
    LavaSubmergedBlock,
    #[serde(rename = "minecraft:blackstone_replace")]
    BlackstoneReplace,
}

const fn default_gravity_heightmap() -> HeightmapType {
    HeightmapType::WorldSurfaceWg
}

impl StructureProcessor {
    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors vanilla StructureProcessor::processBlock"
    )]
    pub fn process_block(
        &self,
        region: &WorldGenRegion<'_>,
        _target_position: BlockPosition,
        reference_pos: BlockPosition,
        template_relative_pos: BlockPosition,
        processed: StructureBlockInfo,
        settings: &StructurePlaceSettings,
        random: &mut RandomSource,
    ) -> Option<StructureBlockInfo> {
        match self {
            Self::Nop | Self::Capped { .. } => Some(processed),
            Self::BlockIgnore { blocks } => {
                if blocks.contains_block(processed.state) {
                    None
                } else {
                    Some(processed)
                }
            }
            Self::BlockRot {
                rottable_blocks,
                integrity,
            } => {
                let rottable = rottable_blocks
                    .as_ref()
                    .is_none_or(|blocks| blocks.contains_block(processed.state));
                let rotted = rottable
                    && settings.with_random(processed.pos, random, |random| {
                        random.next_f32() > *integrity
                    });
                if rotted { None } else { Some(processed) }
            }
            Self::ProtectedBlocks { value } => {
                if value.contains_block(region.get_block(processed.pos)) {
                    None
                } else {
                    Some(processed)
                }
            }
            Self::Rule { rules } => {
                let mut rule_random = RandomSource::Legacy(LegacyRandomSource::from_seed(
                    get_seed(processed.pos.x(), processed.pos.y(), processed.pos.z()) as u64,
                ));
                for rule in rules {
                    if rule.test(
                        region,
                        processed.state,
                        template_relative_pos,
                        processed.pos,
                        reference_pos,
                        &mut rule_random,
                    ) {
                        return Some(StructureBlockInfo {
                            pos: processed.pos,
                            state: rule.output_state,
                            nbt: processed.nbt.clone(),
                        });
                    }
                }
                Some(processed)
            }
            Self::Gravity { heightmap, offset } => {
                let pos = processed.pos;
                let height = region.get_height(*heightmap, pos.x(), pos.z()) + offset;
                let delta = template_relative_pos.y();
                Some(StructureBlockInfo {
                    pos: BlockPosition::new(pos.x(), height + delta, pos.z()),
                    state: processed.state,
                    nbt: processed.nbt,
                })
            }
            Self::JigsawReplacement => {
                let is_jigsaw = Some(processed.state.as_block())
                    .is_some_and(|block| block.key() == Block::JIGSAW.key());
                if !is_jigsaw {
                    return Some(processed);
                }
                let Some(nbt) = processed.nbt.as_ref() else {
                    return Some(processed);
                };
                let final_state = nbt
                    .string("final_state")
                    .map_or_else(|| String::from("minecraft:air"), |value| value.to_string());
                let state = BlockState::parse(&final_state)?;
                let is_void = Some(state.as_block())
                    .is_some_and(|block| block.key() == Block::STRUCTURE_VOID.key());
                if is_void {
                    None
                } else {
                    Some(StructureBlockInfo {
                        pos: processed.pos,
                        state,
                        nbt: None,
                    })
                }
            }
            Self::BlockAge { mossiness } => {
                let new_state = settings.with_random(processed.pos, random, |random| {
                    age_block(processed.state, *mossiness, random)
                });
                Some(match new_state {
                    Some(state) => StructureBlockInfo {
                        pos: processed.pos,
                        state,
                        nbt: processed.nbt,
                    },
                    None => processed,
                })
            }
            Self::LavaSubmergedBlock => {
                let was_lava = region.get_block(processed.pos).is(Block::LAVA);
                if was_lava
                    && !shapes::is_shape_full_block(*processed.state.get_static_collision_shape())
                {
                    Some(StructureBlockInfo {
                        pos: processed.pos,
                        state: Block::LAVA.default_state(),
                        nbt: processed.nbt,
                    })
                } else {
                    Some(processed)
                }
            }
            Self::BlackstoneReplace => {
                let Some(new_block) = blackstone_replacement(processed.state.as_block()) else {
                    return Some(processed);
                };
                let new_state = processed.state.copy_properties_to(new_block);
                Some(StructureBlockInfo {
                    pos: processed.pos,
                    state: new_state,
                    nbt: processed.nbt,
                })
            }
        }
    }

    pub const fn evaluates_entire_piece_state(&self) -> bool {
        matches!(self, Self::Capped { .. })
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors vanilla StructureProcessor::finalizeProcessing"
    )]
    pub fn finalize_processing(
        &self,
        region: &WorldGenRegion<'_>,
        position: BlockPosition,
        reference_pos: BlockPosition,
        original: &[StructureBlockInfo],
        processed: &mut [StructureBlockInfo],
        settings: &StructurePlaceSettings,
        shared_random: &mut RandomSource,
    ) {
        let Self::Capped { delegate, limit } = self else {
            return;
        };
        if limit.max() == 0 || processed.is_empty() || original.len() != processed.len() {
            return;
        }

        let mut source = LegacyRandomSource::from_seed(region.seed() as u64);
        let factory = source.next_positional();
        let mut random = factory.at(position.x(), position.y(), position.z());
        let max_to_replace = limit.sample(&mut random).min(processed.len() as i32);
        if max_to_replace < 1 {
            return;
        }

        let mut indices: Vec<usize> = (0..processed.len()).collect();
        shuffle(&mut indices, &mut random);

        let mut replaced = 0;
        for index in indices {
            if replaced >= max_to_replace {
                break;
            }
            let altered = delegate.process_block(
                region,
                position,
                reference_pos,
                original[index].pos,
                processed[index].clone(),
                settings,
                shared_random,
            );
            if let Some(altered) = altered
                && altered != processed[index]
            {
                replaced += 1;
                processed[index] = altered;
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct StructureProcessorList {
    pub processors: Vec<StructureProcessor>,
}

impl<'de> Deserialize<'de> for StructureProcessorList {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Wrapped {
            processors: Vec<StructureProcessor>,
        }
        use serde::de::Error as _;
        let value = serde_json::Value::deserialize(deserializer)?;
        let processors = if value.is_array() {
            Vec::<StructureProcessor>::deserialize(value).map_err(D::Error::custom)?
        } else {
            Wrapped::deserialize(value)
                .map_err(D::Error::custom)?
                .processors
        };
        Ok(Self { processors })
    }
}

pub fn deserialize_processor_list_holder<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<StructureProcessorList, D::Error> {
    use serde::de::Error as _;
    let value = serde_json::Value::deserialize(deserializer)?;
    if let Some(id) = value.as_str() {
        let key = datapack::parse_id(id);
        return super::structure_template_manager::processor_list(&key)
            .cloned()
            .ok_or_else(|| D::Error::custom(format!("unknown processor list {key}")));
    }
    StructureProcessorList::deserialize(value).map_err(D::Error::custom)
}

pub fn deserialize_optional_processor_list_holder<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<StructureProcessorList>, D::Error> {
    deserialize_processor_list_holder(deserializer).map(Some)
}

const STAIRS_TAG: Key = Key::const_vanilla("stairs");
const SLABS_TAG: Key = Key::const_vanilla("slabs");
const WALLS_TAG: Key = Key::const_vanilla("walls");

fn random_facing_stairs(random: &mut RandomSource, stair_block: Block) -> BlockState {
    let facing = random.next_horizontal_direction();
    let half = if random.next_i32_bounded(2) == 0 {
        Half::Top
    } else {
        Half::Bottom
    };
    stair_block
        .default_state()
        .with(Properties::HORIZONTAL_FACING, facing)
        .with(Properties::HALF, half)
}

fn random_of(random: &mut RandomSource, blocks: &[BlockState]) -> BlockState {
    blocks[random.next_i32_bounded(blocks.len() as i32) as usize]
}

fn age_block(state: BlockState, mossiness: f32, random: &mut RandomSource) -> Option<BlockState> {
    let block = state.as_block();
    if block == Block::STONE_BRICKS
        || block == Block::STONE
        || block == Block::CHISELED_STONE_BRICKS
    {
        if random.next_f32() >= 0.5 {
            return None;
        }
        let non_mossy = [
            Block::CRACKED_STONE_BRICKS.default_state(),
            random_facing_stairs(random, Block::STONE_BRICK_STAIRS),
        ];
        let mossy = [
            Block::MOSSY_STONE_BRICKS.default_state(),
            random_facing_stairs(random, Block::MOSSY_STONE_BRICK_STAIRS),
        ];
        return Some(if random.next_f32() < mossiness {
            random_of(random, &mossy)
        } else {
            random_of(random, &non_mossy)
        });
    }

    if block.has_tag(&STAIRS_TAG) {
        if random.next_f32() >= 0.5 {
            return None;
        }
        let non_mossy = [
            Block::STONE_SLAB.default_state(),
            Block::STONE_BRICK_SLAB.default_state(),
        ];
        let mossy = [
            state.copy_properties_to(Block::MOSSY_STONE_BRICK_STAIRS),
            Block::MOSSY_STONE_BRICK_SLAB.default_state(),
        ];
        return Some(if random.next_f32() < mossiness {
            random_of(random, &mossy)
        } else {
            random_of(random, &non_mossy)
        });
    }

    if block.has_tag(&SLABS_TAG) {
        return (random.next_f32() < mossiness)
            .then(|| state.copy_properties_to(Block::MOSSY_STONE_BRICK_SLAB));
    }

    if block.has_tag(&WALLS_TAG) {
        return (random.next_f32() < mossiness)
            .then(|| state.copy_properties_to(Block::MOSSY_STONE_BRICK_WALL));
    }

    if block == Block::OBSIDIAN {
        return (random.next_f32() < 0.15).then(|| Block::CRYING_OBSIDIAN.default_state());
    }

    None
}

fn blackstone_replacement(block: Block) -> Option<Block> {
    let replacement: Block = if block == Block::COBBLESTONE || block == Block::MOSSY_COBBLESTONE {
        Block::BLACKSTONE
    } else if block == Block::STONE {
        Block::POLISHED_BLACKSTONE
    } else if block == Block::STONE_BRICKS || block == Block::MOSSY_STONE_BRICKS {
        Block::POLISHED_BLACKSTONE_BRICKS
    } else if block == Block::COBBLESTONE_STAIRS || block == Block::MOSSY_COBBLESTONE_STAIRS {
        Block::BLACKSTONE_STAIRS
    } else if block == Block::STONE_STAIRS {
        Block::POLISHED_BLACKSTONE_STAIRS
    } else if block == Block::STONE_BRICK_STAIRS || block == Block::MOSSY_STONE_BRICK_STAIRS {
        Block::POLISHED_BLACKSTONE_BRICK_STAIRS
    } else if block == Block::COBBLESTONE_SLAB || block == Block::MOSSY_COBBLESTONE_SLAB {
        Block::BLACKSTONE_SLAB
    } else if block == Block::SMOOTH_STONE_SLAB || block == Block::STONE_SLAB {
        Block::POLISHED_BLACKSTONE_SLAB
    } else if block == Block::STONE_BRICK_SLAB || block == Block::MOSSY_STONE_BRICK_SLAB {
        Block::POLISHED_BLACKSTONE_BRICK_SLAB
    } else if block == Block::STONE_BRICK_WALL || block == Block::MOSSY_STONE_BRICK_WALL {
        Block::POLISHED_BLACKSTONE_BRICK_WALL
    } else if block == Block::COBBLESTONE_WALL || block == Block::MOSSY_COBBLESTONE_WALL {
        Block::BLACKSTONE_WALL
    } else if block == Block::CHISELED_STONE_BRICKS {
        Block::CHISELED_POLISHED_BLACKSTONE
    } else if block == Block::CRACKED_STONE_BRICKS {
        Block::CRACKED_POLISHED_BLACKSTONE_BRICKS
    } else if block == Block::IRON_BARS {
        Block::IRON_CHAIN
    } else {
        return None;
    };
    Some(replacement)
}
