use serde::Deserialize;

use crate::chunk::heightmap::HeightmapType;
use crate::datapack::lenient_or_default;
use crate::registry::REGISTRY;
use crate::registry::blocks::BlockRef;
use crate::registry::blocks::properties::{Half, Properties};
use crate::registry::blocks::shapes;
use crate::registry::blocks::vanilla_blocks;
use crate::util::axis::Axis;
use crate::util::direction::Direction;
use crate::util::ident::{BlockState, Identifier};
use crate::util::pos::BlockPos;
use crate::util::rng::legacy_random::LegacyRandomSource;
use crate::util::rng::{RandomSource, get_seed};
use crate::util::shuffle;
use crate::util::value_providers::int::IntProvider;
use crate::worldgen::block_predicate::HolderSet;
use crate::worldgen::feature::rule_test::RuleTest;
use crate::worldgen::region::WorldGenRegion;
use crate::worldgen::structure::templatesystem::structure_place_settings::StructurePlaceSettings;
use crate::worldgen::structure::templatesystem::structure_template::StructureBlockInfo;

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
        in_template_pos: BlockPos,
        world_pos: BlockPos,
        world_reference: BlockPos,
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
        in_template_pos: BlockPos,
        world_pos: BlockPos,
        reference: BlockPos,
        random: &mut RandomSource,
    ) -> bool {
        self.input_predicate.test(input_state, random)
            && self
                .location_predicate
                .test(region.get_block(world_pos), random)
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
        _target_position: BlockPos,
        reference_pos: BlockPos,
        template_relative_pos: BlockPos,
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
                    pos: BlockPos::new(pos.x(), height + delta, pos.z()),
                    state: processed.state,
                    nbt: processed.nbt,
                })
            }
            Self::JigsawReplacement => {
                let is_jigsaw = REGISTRY
                    .blocks
                    .by_state_id(processed.state)
                    .is_some_and(|block| block.key == vanilla_blocks::JIGSAW.key);
                if !is_jigsaw {
                    return Some(processed);
                }
                let Some(nbt) = processed.nbt.as_ref() else {
                    return Some(processed);
                };
                let final_state = nbt
                    .string("final_state")
                    .map_or_else(|| String::from("minecraft:air"), |value| value.to_string());
                let state = REGISTRY.blocks.parse_for_block(&final_state)?;
                let is_void = REGISTRY
                    .blocks
                    .by_state_id(state)
                    .is_some_and(|block| block.key == vanilla_blocks::STRUCTURE_VOID.key);
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
                let was_lava = region.get_block(processed.pos).get_block() == &vanilla_blocks::LAVA;
                if was_lava
                    && !shapes::is_shape_full_block(processed.state.get_static_collision_shape())
                {
                    Some(StructureBlockInfo {
                        pos: processed.pos,
                        state: vanilla_blocks::LAVA.default_state(),
                        nbt: processed.nbt,
                    })
                } else {
                    Some(processed)
                }
            }
            Self::BlackstoneReplace => {
                let Some(new_block) = blackstone_replacement(processed.state.get_block()) else {
                    return Some(processed);
                };
                let new_state = REGISTRY
                    .blocks
                    .copy_matching_properties(processed.state, new_block);
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
        position: BlockPos,
        reference_pos: BlockPos,
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

#[derive(Debug, Clone, Deserialize)]
pub struct StructureProcessorList {
    pub processors: Vec<StructureProcessor>,
}

const STAIRS_TAG: Identifier = Identifier::const_vanilla("stairs");
const SLABS_TAG: Identifier = Identifier::const_vanilla("slabs");
const WALLS_TAG: Identifier = Identifier::const_vanilla("walls");

fn random_facing_stairs(random: &mut RandomSource, stair_block: BlockRef) -> BlockState {
    let facing = Direction::get_random_horizontal(random);
    let half = if random.next_i32_bounded(2) == 0 {
        Half::Top
    } else {
        Half::Bottom
    };
    stair_block
        .default_state()
        .set_value(&Properties::HORIZONTAL_FACING, facing)
        .set_value(&Properties::HALF, half)
}

fn random_of(random: &mut RandomSource, blocks: &[BlockState]) -> BlockState {
    blocks[random.next_i32_bounded(blocks.len() as i32) as usize]
}

fn age_block(state: BlockState, mossiness: f32, random: &mut RandomSource) -> Option<BlockState> {
    let block = state.get_block();
    if block == &vanilla_blocks::STONE_BRICKS
        || block == &vanilla_blocks::STONE
        || block == &vanilla_blocks::CHISELED_STONE_BRICKS
    {
        if random.next_f32() >= 0.5 {
            return None;
        }
        let non_mossy = [
            vanilla_blocks::CRACKED_STONE_BRICKS.default_state(),
            random_facing_stairs(random, &vanilla_blocks::STONE_BRICK_STAIRS),
        ];
        let mossy = [
            vanilla_blocks::MOSSY_STONE_BRICKS.default_state(),
            random_facing_stairs(random, &vanilla_blocks::MOSSY_STONE_BRICK_STAIRS),
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
            vanilla_blocks::STONE_SLAB.default_state(),
            vanilla_blocks::STONE_BRICK_SLAB.default_state(),
        ];
        let mossy = [
            REGISTRY
                .blocks
                .copy_matching_properties(state, &vanilla_blocks::MOSSY_STONE_BRICK_STAIRS),
            vanilla_blocks::MOSSY_STONE_BRICK_SLAB.default_state(),
        ];
        return Some(if random.next_f32() < mossiness {
            random_of(random, &mossy)
        } else {
            random_of(random, &non_mossy)
        });
    }

    if block.has_tag(&SLABS_TAG) {
        return (random.next_f32() < mossiness).then(|| {
            REGISTRY
                .blocks
                .copy_matching_properties(state, &vanilla_blocks::MOSSY_STONE_BRICK_SLAB)
        });
    }

    if block.has_tag(&WALLS_TAG) {
        return (random.next_f32() < mossiness).then(|| {
            REGISTRY
                .blocks
                .copy_matching_properties(state, &vanilla_blocks::MOSSY_STONE_BRICK_WALL)
        });
    }

    if block == &vanilla_blocks::OBSIDIAN {
        return (random.next_f32() < 0.15).then(|| vanilla_blocks::CRYING_OBSIDIAN.default_state());
    }

    None
}

fn blackstone_replacement(block: BlockRef) -> Option<BlockRef> {
    let replacement: BlockRef = if block == &vanilla_blocks::COBBLESTONE
        || block == &vanilla_blocks::MOSSY_COBBLESTONE
    {
        &vanilla_blocks::BLACKSTONE
    } else if block == &vanilla_blocks::STONE {
        &vanilla_blocks::POLISHED_BLACKSTONE
    } else if block == &vanilla_blocks::STONE_BRICKS || block == &vanilla_blocks::MOSSY_STONE_BRICKS
    {
        &vanilla_blocks::POLISHED_BLACKSTONE_BRICKS
    } else if block == &vanilla_blocks::COBBLESTONE_STAIRS
        || block == &vanilla_blocks::MOSSY_COBBLESTONE_STAIRS
    {
        &vanilla_blocks::BLACKSTONE_STAIRS
    } else if block == &vanilla_blocks::STONE_STAIRS {
        &vanilla_blocks::POLISHED_BLACKSTONE_STAIRS
    } else if block == &vanilla_blocks::STONE_BRICK_STAIRS
        || block == &vanilla_blocks::MOSSY_STONE_BRICK_STAIRS
    {
        &vanilla_blocks::POLISHED_BLACKSTONE_BRICK_STAIRS
    } else if block == &vanilla_blocks::COBBLESTONE_SLAB
        || block == &vanilla_blocks::MOSSY_COBBLESTONE_SLAB
    {
        &vanilla_blocks::BLACKSTONE_SLAB
    } else if block == &vanilla_blocks::SMOOTH_STONE_SLAB || block == &vanilla_blocks::STONE_SLAB {
        &vanilla_blocks::POLISHED_BLACKSTONE_SLAB
    } else if block == &vanilla_blocks::STONE_BRICK_SLAB
        || block == &vanilla_blocks::MOSSY_STONE_BRICK_SLAB
    {
        &vanilla_blocks::POLISHED_BLACKSTONE_BRICK_SLAB
    } else if block == &vanilla_blocks::STONE_BRICK_WALL
        || block == &vanilla_blocks::MOSSY_STONE_BRICK_WALL
    {
        &vanilla_blocks::POLISHED_BLACKSTONE_BRICK_WALL
    } else if block == &vanilla_blocks::COBBLESTONE_WALL
        || block == &vanilla_blocks::MOSSY_COBBLESTONE_WALL
    {
        &vanilla_blocks::BLACKSTONE_WALL
    } else if block == &vanilla_blocks::CHISELED_STONE_BRICKS {
        &vanilla_blocks::CHISELED_POLISHED_BLACKSTONE
    } else if block == &vanilla_blocks::CRACKED_STONE_BRICKS {
        &vanilla_blocks::CRACKED_POLISHED_BLACKSTONE_BRICKS
    } else if block == &vanilla_blocks::IRON_BARS {
        &vanilla_blocks::IRON_CHAIN
    } else {
        return None;
    };
    Some(replacement)
}
