use serde::Deserialize;

use crate::chunk::heightmap::HeightmapType;
use crate::registry::REGISTRY;
use crate::registry::blocks::vanilla_blocks;
use crate::util::shuffle;
use crate::util::axis::Axis;
use crate::util::ident::BlockState;
use crate::util::pos::BlockPos;
use crate::util::rng::legacy_random::LegacyRandomSource;
use crate::util::rng::{RandomSource, get_seed};
use crate::util::value_providers::int::IntProvider;
use crate::worldgen::block_predicate::HolderSet;
use crate::worldgen::feature::rule_test::RuleTest;
use crate::worldgen::region::WorldGenRegion;
use crate::worldgen::structure::templatesystem::structure_place_settings::StructurePlaceSettings;
use crate::worldgen::structure::templatesystem::structure_template::StructureBlockInfo;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "predicate_type")]
pub enum PosRuleTest {
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
    #[serde(default = "PosRuleTest::always_true")]
    pub position_predicate: PosRuleTest,
    pub output_state: BlockState,
}

impl PosRuleTest {
    const fn always_true() -> Self {
        Self::AlwaysTrue
    }
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
                let mut rule_random = RandomSource::Legacy(LegacyRandomSource::from_seed(get_seed(
                    processed.pos.x(),
                    processed.pos.y(),
                    processed.pos.z(),
                ) as u64));
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
