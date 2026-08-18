use std::cell::RefCell;

use serde::Deserialize;

use crate::chunk::heightmap::HeightmapType;
use crate::registry::REGISTRY;
use crate::registry::biome::BiomeRef;
use crate::util::geometry::BoundingBox;
use crate::util::ident::Identifier;
use crate::util::pos::{BlockPos, ChunkPos};
use crate::util::rng::RandomSource;
use crate::util::rng::legacy_random::LegacyRandomSource;
use crate::util::rotation::Rotation;
use crate::util::value_providers::height::HeightProvider;
use crate::worldgen::biome::biome_source::ChunkBiomeSampler;
use crate::worldgen::generator::Generator;
use crate::worldgen::region::WorldGenRegion;
use crate::worldgen::structure::pieces::pieces_container::PiecesContainer;
use crate::worldgen::structure::pieces::structure_pieces_builder::StructurePiecesBuilder;
use crate::worldgen::structure::structure_piece::PieceKind;
use crate::worldgen::structure::structure_start::StructureStart;
use crate::worldgen::structure::structures::buried_treasure_pieces::BuriedTreasurePiece;
use crate::worldgen::structure::structures::desert_pyramid;
use crate::worldgen::structure::structures::desert_pyramid_piece::{
    self as desert_pyramid_piece_mod, DesertPyramidPiece,
};
use crate::worldgen::structure::structures::end_city;
use crate::worldgen::structure::structures::igloo_pieces;
use crate::worldgen::structure::structures::jigsaw::JigsawStructure;
use crate::worldgen::structure::structures::jungle_temple_piece::{self, JungleTemplePiece};
use crate::worldgen::structure::structures::mineshaft;
use crate::worldgen::structure::structures::mineshaft_pieces;
use crate::worldgen::structure::structures::nether_fortress;
use crate::worldgen::structure::structures::nether_fossil;
use crate::worldgen::structure::structures::ocean_monument;
use crate::worldgen::structure::structures::ocean_ruin_pieces;
use crate::worldgen::structure::structures::ruined_portal;
use crate::worldgen::structure::structures::shipwreck_pieces;
use crate::worldgen::structure::structures::stronghold;
use crate::worldgen::structure::structures::swamp_hut_piece::SwampHutPiece;
use crate::worldgen::structure::structures::woodland_mansion;

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
    pub biome_sampler: RefCell<ChunkBiomeSampler<'a>>,
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
            biome_sampler: RefCell::new(generator.biome_source().chunk_sampler()),
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
    SwampHut(StructureSettings),
    DesertPyramid(StructureSettings),
    Igloo(StructureSettings),
    JungleTemple(StructureSettings),
    BuriedTreasure(StructureSettings),
    Shipwreck {
        settings: StructureSettings,
        is_beached: bool,
    },
    OceanRuin {
        settings: StructureSettings,
        biome_temp: ocean_ruin_pieces::Temperature,
        large_probability: f32,
        cluster_probability: f32,
    },
    RuinedPortal {
        settings: StructureSettings,
        setups: Vec<ruined_portal::Setup>,
    },
    Mineshaft {
        settings: StructureSettings,
        mineshaft_type: mineshaft_pieces::MineshaftType,
    },
    NetherFossil {
        settings: StructureSettings,
        height: HeightProvider,
    },
    EndCity(StructureSettings),
    WoodlandMansion(StructureSettings),
    Fortress(StructureSettings),
    Stronghold(StructureSettings),
    OceanMonument(StructureSettings),
    Unported(Identifier),
}

impl Structure {
    pub const fn settings(&self) -> Option<&StructureSettings> {
        match self {
            Self::Jigsaw(jigsaw) => Some(&jigsaw.settings),
            Self::SwampHut(settings)
            | Self::DesertPyramid(settings)
            | Self::Igloo(settings)
            | Self::JungleTemple(settings)
            | Self::BuriedTreasure(settings) => Some(settings),
            Self::Shipwreck { settings, .. }
            | Self::OceanRuin { settings, .. }
            | Self::RuinedPortal { settings, .. }
            | Self::Mineshaft { settings, .. }
            | Self::NetherFossil { settings, .. } => Some(settings),
            Self::EndCity(settings)
            | Self::WoodlandMansion(settings)
            | Self::Fortress(settings)
            | Self::Stronghold(settings)
            | Self::OceanMonument(settings) => Some(settings),
            Self::Unported(_) => None,
        }
    }

    pub fn step(&self) -> GenerationStep {
        self.settings()
            .map_or(GenerationStep::SurfaceStructures, |settings| settings.step)
    }

    pub fn terrain_adaptation(&self) -> TerrainAdjustment {
        self.settings().map_or(TerrainAdjustment::None, |settings| {
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
            Self::SwampHut(_) => Some(on_top_of_chunk_center(
                context,
                HeightmapType::WorldSurfaceWg,
                Box::new(|builder| {
                    let chunk_pos = context.chunk_pos;
                    let piece = context.with_random(|random| {
                        SwampHutPiece::new(random, chunk_pos.0.x * 16, chunk_pos.0.y * 16)
                    });
                    builder.add_piece(piece);
                }),
            )),
            Self::DesertPyramid(_) => single_piece_generation_point(
                context,
                desert_pyramid_piece_mod::WIDTH,
                desert_pyramid_piece_mod::DEPTH,
                Box::new(|builder| {
                    let chunk_pos = context.chunk_pos;
                    let piece = context.with_random(|random| {
                        DesertPyramidPiece::new(random, chunk_pos.0.x * 16, chunk_pos.0.y * 16)
                    });
                    builder.add_piece(piece);
                }),
            ),
            Self::Igloo(_) => Some(on_top_of_chunk_center(
                context,
                HeightmapType::WorldSurfaceWg,
                Box::new(|builder| {
                    let chunk_pos = context.chunk_pos;
                    let start_pos = BlockPos::new(
                        chunk_pos.0.x * 16,
                        igloo_pieces::GENERATION_HEIGHT,
                        chunk_pos.0.y * 16,
                    );
                    context.with_random(|random| {
                        let rotation = Rotation::get_random(random);
                        igloo_pieces::add_pieces(start_pos, rotation, builder, random);
                    });
                }),
            )),
            Self::JungleTemple(_) => single_piece_generation_point(
                context,
                jungle_temple_piece::WIDTH,
                jungle_temple_piece::DEPTH,
                Box::new(|builder| {
                    let chunk_pos = context.chunk_pos;
                    let piece = context.with_random(|random| {
                        JungleTemplePiece::new(random, chunk_pos.0.x * 16, chunk_pos.0.y * 16)
                    });
                    builder.add_piece(piece);
                }),
            ),
            Self::BuriedTreasure(_) => Some(on_top_of_chunk_center(
                context,
                HeightmapType::OceanFloorWg,
                Box::new(|builder| {
                    let chunk_pos = context.chunk_pos;
                    let offset = BlockPos::new(chunk_pos.0.x * 16 + 9, 90, chunk_pos.0.y * 16 + 9);
                    builder.add_piece(BuriedTreasurePiece::new(offset));
                }),
            )),
            Self::Shipwreck { is_beached, .. } => {
                let is_beached = *is_beached;
                let heightmap = if is_beached {
                    HeightmapType::WorldSurfaceWg
                } else {
                    HeightmapType::OceanFloorWg
                };
                Some(on_top_of_chunk_center(
                    context,
                    heightmap,
                    Box::new(move |builder| {
                        let chunk_pos = context.chunk_pos;
                        let offset = BlockPos::new(chunk_pos.0.x * 16, 90, chunk_pos.0.y * 16);
                        let mut piece = context.with_random(|random| {
                            let rotation = Rotation::get_random(random);
                            shipwreck_pieces::random_piece(offset, rotation, random, is_beached)
                        });

                        let too_big = match &piece.kind {
                            PieceKind::Shipwreck(shipwreck) => {
                                shipwreck.is_too_big_to_fit_in_world_gen_region()
                            }
                            _ => false,
                        };
                        if too_big {
                            let bb = piece.bounding_box();
                            let height = if is_beached {
                                let min_y = get_lowest_y_at(
                                    context,
                                    bb.min_x(),
                                    bb.x_span(),
                                    bb.min_z(),
                                    bb.z_span(),
                                );
                                let PieceKind::Shipwreck(shipwreck) = &piece.kind else {
                                    unreachable!()
                                };
                                context.with_random(|random| {
                                    shipwreck.calculate_beached_position(min_y, random)
                                })
                            } else {
                                get_mean_first_occupied_height(
                                    context,
                                    bb.min_x(),
                                    bb.x_span(),
                                    bb.min_z(),
                                    bb.z_span(),
                                )
                            };
                            if let PieceKind::Shipwreck(shipwreck) = &mut piece.kind {
                                shipwreck.adjust_position_height(height);
                            }
                        }

                        builder.add_piece(piece);
                    }),
                ))
            }
            Self::OceanRuin {
                biome_temp,
                large_probability,
                cluster_probability,
                ..
            } => {
                let biome_temp = *biome_temp;
                let large_probability = *large_probability;
                let cluster_probability = *cluster_probability;
                Some(on_top_of_chunk_center(
                    context,
                    HeightmapType::OceanFloorWg,
                    Box::new(move |builder| {
                        let chunk_pos = context.chunk_pos;
                        let offset = BlockPos::new(chunk_pos.0.x * 16, 90, chunk_pos.0.y * 16);
                        context.with_random(|random| {
                            let rotation = Rotation::get_random(random);
                            ocean_ruin_pieces::add_pieces(
                                offset,
                                rotation,
                                builder,
                                random,
                                biome_temp,
                                large_probability,
                                cluster_probability,
                            );
                        });
                    }),
                ))
            }
            Self::RuinedPortal { setups, .. } => {
                ruined_portal::find_generation_point(context, setups)
            }
            Self::Mineshaft { mineshaft_type, .. } => {
                mineshaft::find_generation_point(context, *mineshaft_type)
            }
            Self::NetherFossil { height, .. } => {
                nether_fossil::find_generation_point(context, *height)
            }
            Self::EndCity(_) => end_city::find_generation_point(context),
            Self::WoodlandMansion(_) => woodland_mansion::find_generation_point(context),
            Self::Fortress(_) => nether_fortress::find_generation_point(context),
            Self::Stronghold(_) => stronghold::find_generation_point(context),
            Self::OceanMonument(_) => ocean_monument::find_generation_point(context),
            Self::Unported(_) => None,
        }
    }

    pub fn after_place(
        &self,
        region: &WorldGenRegion<'_>,
        chunk_bb: BoundingBox,
        pieces: &PiecesContainer,
    ) {
        match self {
            Self::DesertPyramid(_) => desert_pyramid::after_place(region, chunk_bb, pieces),
            Self::WoodlandMansion(_) => woodland_mansion::after_place(region, chunk_bb, pieces),
            Self::Jigsaw(_)
            | Self::SwampHut(_)
            | Self::Igloo(_)
            | Self::JungleTemple(_)
            | Self::BuriedTreasure(_)
            | Self::Shipwreck { .. }
            | Self::OceanRuin { .. }
            | Self::RuinedPortal { .. }
            | Self::Mineshaft { .. }
            | Self::NetherFossil { .. }
            | Self::EndCity(_)
            | Self::Fortress(_)
            | Self::Stronghold(_)
            | Self::OceanMonument(_)
            | Self::Unported(_) => {}
        }
    }
}

pub fn on_top_of_chunk_center<'a>(
    context: &'a GenerationContext<'a>,
    heightmap: HeightmapType,
    build: Box<dyn FnOnce(&mut StructurePiecesBuilder) + 'a>,
) -> GenerationStub<'a> {
    let chunk_pos = context.chunk_pos;
    let block_x = chunk_pos.0.x * 16 + 8;
    let block_z = chunk_pos.0.y * 16 + 8;
    let block_y = context
        .generator
        .get_first_occupied_height(block_x, block_z, heightmap);
    GenerationStub {
        position: BlockPos::new(block_x, block_y, block_z),
        build,
    }
}

fn corner_heights(
    context: &GenerationContext<'_>,
    min_x: i32,
    size_x: i32,
    min_z: i32,
    size_z: i32,
) -> [i32; 4] {
    let generator = context.generator;
    let heightmap = HeightmapType::WorldSurfaceWg;
    [
        generator.get_first_occupied_height(min_x, min_z, heightmap),
        generator.get_first_occupied_height(min_x, min_z + size_z, heightmap),
        generator.get_first_occupied_height(min_x + size_x, min_z, heightmap),
        generator.get_first_occupied_height(min_x + size_x, min_z + size_z, heightmap),
    ]
}

pub fn get_mean_first_occupied_height(
    context: &GenerationContext<'_>,
    min_x: i32,
    size_x: i32,
    min_z: i32,
    size_z: i32,
) -> i32 {
    let heights = corner_heights(context, min_x, size_x, min_z, size_z);
    (heights[0] + heights[1] + heights[2] + heights[3]) / 4
}

pub fn get_lowest_y_at(
    context: &GenerationContext<'_>,
    min_x: i32,
    min_z: i32,
    size_x: i32,
    size_z: i32,
) -> i32 {
    let heights = corner_heights(context, min_x, size_x, min_z, size_z);
    heights[0].min(heights[1]).min(heights[2]).min(heights[3])
}

pub fn get_lowest_y(context: &GenerationContext<'_>, size_x: i32, size_z: i32) -> i32 {
    let chunk_pos = context.chunk_pos;
    get_lowest_y_at(
        context,
        chunk_pos.0.x * 16,
        chunk_pos.0.y * 16,
        size_x,
        size_z,
    )
}

pub fn get_lowest_y_in_5by5_box_offset_7_blocks(
    context: &GenerationContext<'_>,
    rotation: Rotation,
) -> BlockPos {
    let (offset_x, offset_z) = match rotation {
        Rotation::None => (5, 5),
        Rotation::Clockwise90 => (-5, 5),
        Rotation::Clockwise180 => (-5, -5),
        Rotation::CounterClockwise90 => (5, -5),
    };
    let chunk_pos = context.chunk_pos;
    let block_x = chunk_pos.0.x * 16 + 7;
    let block_z = chunk_pos.0.y * 16 + 7;
    BlockPos::new(
        block_x,
        get_lowest_y_at(context, block_x, block_z, offset_x, offset_z),
        block_z,
    )
}

fn single_piece_generation_point<'a>(
    context: &'a GenerationContext<'a>,
    width: i32,
    depth: i32,
    build: Box<dyn FnOnce(&mut StructurePiecesBuilder) + 'a>,
) -> Option<GenerationStub<'a>> {
    if get_lowest_y(context, width, depth) < context.generator.sea_level() {
        return None;
    }
    Some(on_top_of_chunk_center(
        context,
        HeightmapType::WorldSurfaceWg,
        build,
    ))
}

fn is_valid_biome(stub: &GenerationStub<'_>, context: &GenerationContext<'_>) -> bool {
    let position = stub.position;
    let biome = context.biome_sampler.borrow_mut().sample(
        position.x() >> 2,
        position.y() >> 2,
        position.z() >> 2,
    );
    (context.valid_biome)(biome)
}
