use crate::util::rng::legacy_random::LegacyRandomSource;
use crate::util::rng::{RandomSource, get_seed};
use crate::world::worldgen::structure::templatesystem::liquid_settings::LiquidSettings;
use crate::world::worldgen::structure::templatesystem::structure_processor::StructureProcessor;
use shulkr::util::BlockPosition;
use shulkr::util::BoundingBox;
use shulkr::world::block::Mirror;
use shulkr::world::block::Rotation;

#[derive(Debug, Clone, Default)]
pub struct StructurePlaceSettings {
    mirror: Mirror,
    rotation: Rotation,
    rotation_pivot: BlockPosition,
    ignore_entities: bool,
    bounding_box: Option<BoundingBox>,
    liquid_settings: LiquidSettings,
    uses_shared_random: bool,
    processors: Vec<StructureProcessor>,
    known_shape: bool,
    finalize_entities: bool,
}

impl StructurePlaceSettings {
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub const fn set_mirror(mut self, mirror: Mirror) -> Self {
        self.mirror = mirror;
        self
    }

    #[must_use]
    pub const fn set_rotation(mut self, rotation: Rotation) -> Self {
        self.rotation = rotation;
        self
    }

    #[must_use]
    pub const fn set_rotation_pivot(mut self, pivot: BlockPosition) -> Self {
        self.rotation_pivot = pivot;
        self
    }

    #[must_use]
    pub const fn set_ignore_entities(mut self, ignore_entities: bool) -> Self {
        self.ignore_entities = ignore_entities;
        self
    }

    #[must_use]
    pub const fn set_bounding_box(mut self, bounding_box: BoundingBox) -> Self {
        self.bounding_box = Some(bounding_box);
        self
    }

    #[must_use]
    pub const fn set_liquid_settings(mut self, liquid_settings: LiquidSettings) -> Self {
        self.liquid_settings = liquid_settings;
        self
    }

    #[must_use]
    pub const fn set_known_shape(mut self, known_shape: bool) -> Self {
        self.known_shape = known_shape;
        self
    }

    #[must_use]
    pub const fn set_finalize_entities(mut self, finalize_entities: bool) -> Self {
        self.finalize_entities = finalize_entities;
        self
    }

    #[must_use]
    pub const fn use_shared_random(mut self, uses_shared_random: bool) -> Self {
        self.uses_shared_random = uses_shared_random;
        self
    }

    pub fn clear_processors(&mut self) {
        self.processors.clear();
    }

    pub fn add_processor(&mut self, processor: StructureProcessor) {
        self.processors.push(processor);
    }

    pub const fn mirror(&self) -> Mirror {
        self.mirror
    }

    pub const fn rotation(&self) -> Rotation {
        self.rotation
    }

    pub const fn rotation_pivot(&self) -> BlockPosition {
        self.rotation_pivot
    }

    pub const fn bounding_box(&self) -> Option<BoundingBox> {
        self.bounding_box
    }

    pub const fn known_shape(&self) -> bool {
        self.known_shape
    }

    pub fn processors(&self) -> &[StructureProcessor] {
        &self.processors
    }

    pub const fn should_apply_waterlogging(&self) -> bool {
        matches!(self.liquid_settings, LiquidSettings::ApplyWaterlogging)
    }

    pub fn with_random<R>(
        &self,
        pos: BlockPosition,
        shared: &mut RandomSource,
        body: impl FnOnce(&mut RandomSource) -> R,
    ) -> R {
        if self.uses_shared_random {
            body(shared)
        } else {
            let seed = get_seed(pos.x(), pos.y(), pos.z());
            let mut random = RandomSource::Legacy(LegacyRandomSource::from_seed(seed as u64));
            body(&mut random)
        }
    }

    pub fn random_palette(
        &self,
        palette_count: usize,
        pos: BlockPosition,
        shared: &mut RandomSource,
    ) -> usize {
        assert!(palette_count > 0, "No palettes");
        self.with_random(pos, shared, |random| {
            random.next_i32_bounded(palette_count as i32) as usize
        })
    }
}
