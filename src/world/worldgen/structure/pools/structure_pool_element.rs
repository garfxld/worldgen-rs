use crate::world::worldgen::structure::templatesystem::structure_template::{
    JointType, empty_jigsaw_id, empty_pool_id,
};
use std::sync::LazyLock;

use shulkr::util::Key;
use shulkr::world::block::{Block, BlockState};

use glam::IVec3;

use shulkr::util::BoundingBox;

use crate::util::rng::RandomSource;
use crate::util::shuffle;
use crate::world::worldgen::block_predicate::HolderSet;
use crate::world::worldgen::feature::PlacedFeature;
use crate::world::worldgen::region::WorldGenRegion;
use crate::world::worldgen::structure::pools::structure_template_pool::Projection;
use crate::world::worldgen::structure::templatesystem::liquid_settings::LiquidSettings;
use crate::world::worldgen::structure::templatesystem::structure_place_settings::StructurePlaceSettings;
use crate::world::worldgen::structure::templatesystem::structure_processor::StructureProcessor;
use crate::world::worldgen::structure::templatesystem::structure_template::{
    JigsawBlockInfo, StructureBlockInfo, StructureTemplate,
};
use crate::world::worldgen::structure::templatesystem::structure_template_manager::templates;
use shulkr::util::BlockPosition;
use shulkr::world::block::Rotation;

#[derive(Debug)]
pub struct SinglePoolElement {
    pub template: Key,
    pub processors: Vec<StructureProcessor>,
    pub projection: Projection,
    pub override_liquid_settings: Option<LiquidSettings>,
}

#[derive(Debug)]
pub struct FeaturePoolElement {
    pub feature: &'static PlacedFeature,
    pub projection: Projection,
}

#[derive(Debug)]
pub enum StructurePoolElement {
    Single(SinglePoolElement),
    LegacySingle(SinglePoolElement),
    List {
        elements: Vec<StructurePoolElement>,
        projection: Projection,
    },
    Feature(FeaturePoolElement),
    Empty,
}

static EMPTY: LazyLock<&'static StructurePoolElement> =
    LazyLock::new(|| Box::leak(Box::new(StructurePoolElement::Empty)));

impl StructurePoolElement {
    pub fn empty() -> &'static Self {
        *EMPTY
    }

    pub const fn projection(&self) -> Projection {
        match self {
            Self::Single(single) | Self::LegacySingle(single) => single.projection,
            Self::List { projection, .. } => *projection,
            Self::Feature(feature) => feature.projection,
            Self::Empty => Projection::TerrainMatching,
        }
    }

    pub const fn ground_level_delta(&self) -> i32 {
        1
    }

    fn template(single: &SinglePoolElement) -> &'static StructureTemplate {
        templates().get_or_create(&single.template)
    }

    pub fn get_size(&self, rotation: Rotation) -> IVec3 {
        match self {
            Self::Single(single) | Self::LegacySingle(single) => {
                Self::template(single).size_rotated(rotation)
            }
            Self::List { elements, .. } => elements
                .iter()
                .map(|element| element.get_size(rotation))
                .fold(IVec3::ZERO, IVec3::max),
            Self::Feature(_) | Self::Empty => IVec3::ZERO,
        }
    }

    pub fn get_shuffled_jigsaw_blocks(
        &self,
        position: BlockPosition,
        rotation: Rotation,
        random: &mut RandomSource,
    ) -> Vec<JigsawBlockInfo> {
        match self {
            Self::Single(single) | Self::LegacySingle(single) => {
                let mut jigsaws = Self::template(single).get_jigsaws(position, rotation, random);
                shuffle(&mut jigsaws, random);
                jigsaws.sort_by_key(|jigsaw| -jigsaw.selection_priority);
                jigsaws
            }
            Self::List { elements, .. } => elements
                .first()
                .map(|element| element.get_shuffled_jigsaw_blocks(position, rotation, random))
                .unwrap_or_default(),
            Self::Feature(_) => vec![JigsawBlockInfo::new(
                StructureBlockInfo {
                    pos: position,
                    state: default_feature_jigsaw_state(),
                    nbt: None,
                },
                JointType::Rollable,
                None,
                empty_pool_id(),
                empty_jigsaw_id(),
                0,
                0,
            )],
            Self::Empty => Vec::new(),
        }
    }

    pub fn get_bounding_box(&self, position: BlockPosition, rotation: Rotation) -> BoundingBox {
        match self {
            Self::Single(single) | Self::LegacySingle(single) => Self::template(single)
                .get_bounding_box(
                    &StructurePlaceSettings::new().set_rotation(rotation),
                    position,
                ),
            Self::List { elements, .. } => elements
                .iter()
                .filter(|element| !matches!(element, Self::Empty))
                .map(|element| element.get_bounding_box(position, rotation))
                .reduce(BoundingBox::encapsulate_box)
                .expect("Unable to calculate boundingbox for ListPoolElement"),
            Self::Feature(_) | Self::Empty => {
                let size = self.get_size(rotation);
                BoundingBox::from_corners(
                    position,
                    BlockPosition::new(
                        position.x() + size.x,
                        position.y() + size.y,
                        position.z() + size.z,
                    ),
                )
            }
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors vanilla StructurePoolElement::place"
    )]
    pub fn place(
        &self,
        region: &WorldGenRegion<'_>,
        position: BlockPosition,
        reference_pos: BlockPosition,
        rotation: Rotation,
        chunk_bb: BoundingBox,
        random: &mut RandomSource,
        liquid_settings: LiquidSettings,
        keep_jigsaws: bool,
    ) -> bool {
        match self {
            Self::Single(single) | Self::LegacySingle(single) => {
                let template = Self::template(single);
                let settings = self.settings(rotation, chunk_bb, liquid_settings, keep_jigsaws);
                template.place_in_world(region, position, reference_pos, &settings, random)
            }
            Self::List { elements, .. } => elements.iter().all(|element| {
                element.place(
                    region,
                    position,
                    reference_pos,
                    rotation,
                    chunk_bb,
                    random,
                    liquid_settings,
                    keep_jigsaws,
                )
            }),
            Self::Feature(feature) => feature.feature.place(region, random, position),
            Self::Empty => true,
        }
    }

    fn settings(
        &self,
        rotation: Rotation,
        chunk_bb: BoundingBox,
        liquid_settings: LiquidSettings,
        keep_jigsaws: bool,
    ) -> StructurePlaceSettings {
        let (Self::Single(single) | Self::LegacySingle(single)) = self else {
            return StructurePlaceSettings::new();
        };

        let mut settings = StructurePlaceSettings::new()
            .set_bounding_box(chunk_bb)
            .set_rotation(rotation)
            .set_known_shape(true)
            .set_ignore_entities(false)
            .set_finalize_entities(true)
            .set_liquid_settings(single.override_liquid_settings.unwrap_or(liquid_settings));

        if matches!(self, Self::LegacySingle(_)) {
            settings.add_processor(block_ignore_structure_and_air());
        } else {
            settings.add_processor(block_ignore_structure_block());
        }

        if !keep_jigsaws {
            settings.add_processor(StructureProcessor::JigsawReplacement);
        }

        for processor in &single.processors {
            settings.add_processor(processor.clone());
        }
        for processor in single.projection.processors() {
            settings.add_processor(processor);
        }

        settings
    }
}

fn block_ignore_structure_block() -> StructureProcessor {
    StructureProcessor::BlockIgnore {
        blocks: HolderSet::Entries(vec![Block::STRUCTURE_BLOCK.key().clone()]),
    }
}

fn block_ignore_structure_and_air() -> StructureProcessor {
    StructureProcessor::BlockIgnore {
        blocks: HolderSet::Entries(vec![
            Block::AIR.key().clone(),
            Block::STRUCTURE_BLOCK.key().clone(),
        ]),
    }
}

fn default_feature_jigsaw_state() -> BlockState {
    BlockState::parse("minecraft:jigsaw[orientation=down_south]")
        .expect("jigsaw block state should exist")
}
