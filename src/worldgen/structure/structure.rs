use std::cell::RefCell;

use serde::Deserialize;

use crate::registry::REGISTRY;
use crate::registry::biome::BiomeRef;
use crate::util::geometry::BoundingBox;
use crate::util::ident::Identifier;
use crate::util::pos::{BlockPos, ChunkPos};
use crate::util::rng::RandomSource;
use crate::util::rng::legacy_random::LegacyRandomSource;
use crate::worldgen::generator::Generator;
use crate::worldgen::structure::pieces::structure_pieces_builder::StructurePiecesBuilder;
use crate::worldgen::structure::structure_start::StructureStart;
use crate::worldgen::structure::structures::jigsaw::JigsawStructure;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, PartialOrd, Ord)]
pub enum GenerationStep {
    #[serde(rename = "raw_generation")]
    RawGeneration,
    #[serde(rename = "lakes")]
    Lakes,
    #[serde(rename = "local_modifications")]
    LocalModifications,
    #[serde(rename = "underground_structures")]
    UndergroundStructures,
    #[serde(rename = "surface_structures")]
    SurfaceStructures,
    #[serde(rename = "strongholds")]
    Strongholds,
    #[serde(rename = "underground_ores")]
    UndergroundOres,
    #[serde(rename = "underground_decoration")]
    UndergroundDecoration,
    #[serde(rename = "fluid_springs")]
    FluidSprings,
    #[serde(rename = "vegetal_decoration")]
    VegetalDecoration,
    #[serde(rename = "top_layer_modification")]
    TopLayerModification,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum TerrainAdjustment {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "bury")]
    Bury,
    #[serde(rename = "beard_thin")]
    BeardThin,
    #[serde(rename = "beard_box")]
    BeardBox,
    #[serde(rename = "encapsulate")]
    Encapsulate,
}

#[derive(Debug)]
pub struct StructureSettings {
    pub biomes: BiomeHolderSet,
    pub step: GenerationStep,
    pub terrain_adaptation: TerrainAdjustment,
}

#[derive(Debug, Clone)]
pub enum BiomeHolderSet {
    Tag(Identifier),
    Entries(Vec<Identifier>),
}

impl BiomeHolderSet {
    pub fn contains(&self, biome: BiomeRef) -> bool {
        match self {
            Self::Tag(tag) => REGISTRY.biomes.is_in_tag(&biome.key, tag),
            Self::Entries(keys) => keys.contains(&biome.key),
        }
    }
}

pub struct GenerationContext<'a> {
    pub generator: &'a Generator,
    pub random: RefCell<RandomSource>,
    pub seed: i64,
    pub chunk_pos: ChunkPos,
    pub min_y: i32,
    pub height: i32,
    pub valid_biome: &'a dyn Fn(BiomeRef) -> bool,
}

impl<'a> GenerationContext<'a> {
    pub fn new(
        generator: &'a Generator,
        seed: i64,
        chunk_pos: ChunkPos,
        valid_biome: &'a dyn Fn(BiomeRef) -> bool,
    ) -> Self {
        let mut random = LegacyRandomSource::from_seed(0);
        random.set_large_feature_seed(seed, chunk_pos.0.x, chunk_pos.0.y);
        Self {
            generator,
            random: RefCell::new(RandomSource::Legacy(random)),
            seed,
            chunk_pos,
            min_y: generator.noises().min_y(),
            height: generator.noises().height(),
            valid_biome,
        }
    }

    pub const fn max_y(&self) -> i32 {
        self.min_y + self.height - 1
    }

    pub fn with_random<R>(&self, body: impl FnOnce(&mut RandomSource) -> R) -> R {
        body(&mut self.random.borrow_mut())
    }
}

pub struct GenerationStub<'a> {
    pub position: BlockPos,
    pub build: Box<dyn FnOnce(&mut StructurePiecesBuilder) + 'a>,
}

#[derive(Debug)]
pub enum Structure {
    Jigsaw(JigsawStructure),
    Unported(Identifier),
}

impl Structure {
    pub const fn settings(&self) -> Option<&StructureSettings> {
        match self {
            Self::Jigsaw(jigsaw) => Some(&jigsaw.settings),
            Self::Unported(_) => None,
        }
    }

    pub fn step(&self) -> GenerationStep {
        self.settings()
            .map_or(GenerationStep::SurfaceStructures, |settings| settings.step)
    }

    pub fn terrain_adaptation(&self) -> TerrainAdjustment {
        self.settings()
            .map_or(TerrainAdjustment::None, |settings| {
                settings.terrain_adaptation
            })
    }

    pub fn biomes(&self) -> Option<&BiomeHolderSet> {
        self.settings().map(|settings| &settings.biomes)
    }

    pub fn adjust_bounding_box(&self, bounding_box: BoundingBox) -> BoundingBox {
        if self.terrain_adaptation() == TerrainAdjustment::None {
            bounding_box
        } else {
            bounding_box.inflated_by(12)
        }
    }

    pub fn generate(
        &'static self,
        generator: &Generator,
        seed: i64,
        chunk_pos: ChunkPos,
        references: i32,
        valid_biome: &dyn Fn(BiomeRef) -> bool,
    ) -> StructureStart {
        let context = GenerationContext::new(generator, seed, chunk_pos, valid_biome);
        let Some(stub) = self.find_valid_generation_point(&context) else {
            return StructureStart::invalid();
        };

        let mut builder = StructurePiecesBuilder::new();
        (stub.build)(&mut builder);
        let start = StructureStart::new(Some(self), chunk_pos, references, builder.build());
        if start.is_valid() {
            start
        } else {
            StructureStart::invalid()
        }
    }

    fn find_valid_generation_point<'a>(
        &'a self,
        context: &'a GenerationContext<'a>,
    ) -> Option<GenerationStub<'a>> {
        let stub = self.find_generation_point(context)?;
        is_valid_biome(&stub, context).then_some(stub)
    }

    fn find_generation_point<'a>(
        &'a self,
        context: &'a GenerationContext<'a>,
    ) -> Option<GenerationStub<'a>> {
        match self {
            Self::Jigsaw(jigsaw) => jigsaw.find_generation_point(context),
            Self::Unported(_) => None,
        }
    }
}

fn is_valid_biome(stub: &GenerationStub<'_>, context: &GenerationContext<'_>) -> bool {
    let position = stub.position;
    let mut sampler = context.generator.biome_source().chunk_sampler();
    sampler.init_grid(position.x() & !15, position.z() & !15);
    let biome = sampler.sample(position.x() >> 2, position.y() >> 2, position.z() >> 2);
    (context.valid_biome)(biome)
}
