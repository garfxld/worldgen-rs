use crate::chunk::heightmap::HeightmapType;
use crate::util::ident::Identifier;
use crate::util::pos::BlockPos;
use crate::util::value_providers::height::HeightProvider;
use crate::worldgen::structure::pools::alias::pool_alias_binding::PoolAliasBinding;
use crate::worldgen::structure::pools::alias::pool_alias_lookup::PoolAliasLookup;
use crate::worldgen::structure::pools::dimension_padding::DimensionPadding;
use crate::worldgen::structure::pools::jigsaw_placement;
use crate::worldgen::structure::structure::{GenerationContext, GenerationStub, StructureSettings};
use crate::worldgen::structure::templatesystem::liquid_settings::LiquidSettings;

pub const DEFAULT_DIMENSION_PADDING: DimensionPadding = DimensionPadding::ZERO;
pub const DEFAULT_LIQUID_SETTINGS: LiquidSettings = LiquidSettings::ApplyWaterlogging;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaxDistance {
    pub horizontal: i32,
    pub vertical: i32,
}

impl MaxDistance {
    pub const fn of(value: i32) -> Self {
        Self {
            horizontal: value,
            vertical: value,
        }
    }
}

#[derive(Debug)]
pub struct JigsawStructure {
    pub settings: StructureSettings,
    pub start_pool: Identifier,
    pub start_jigsaw_name: Option<Identifier>,
    pub max_depth: i32,
    pub start_height: HeightProvider,
    pub use_expansion_hack: bool,
    pub project_start_to_heightmap: Option<HeightmapType>,
    pub max_distance_from_center: MaxDistance,
    pub pool_aliases: Vec<PoolAliasBinding>,
    pub dimension_padding: DimensionPadding,
    pub liquid_settings: LiquidSettings,
}

impl JigsawStructure {
    pub fn find_generation_point<'a>(
        &'a self,
        context: &'a GenerationContext<'a>,
    ) -> Option<GenerationStub<'a>> {
        let chunk_pos = context.chunk_pos;
        let height = context.with_random(|random| {
            self.start_height
                .sample(random, context.min_y, context.height)
        });
        let start_pos = BlockPos::new(chunk_pos.0.x * 16, height, chunk_pos.0.y * 16);
        jigsaw_placement::add_pieces(
            context,
            &self.start_pool,
            self.start_jigsaw_name.as_ref(),
            self.max_depth,
            start_pos,
            self.use_expansion_hack,
            self.project_start_to_heightmap,
            self.max_distance_from_center,
            PoolAliasLookup::create(&self.pool_aliases, start_pos, context.seed),
            self.dimension_padding,
            self.liquid_settings,
        )
    }
}
