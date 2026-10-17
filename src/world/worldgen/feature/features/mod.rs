pub mod bamboo;
pub mod block_blob;
pub mod block_column;
pub mod block_pile;
pub mod blockpos_iter;
pub mod blue_ice;
pub mod bonus_chest;
pub mod chorus_plant;
pub mod coral_claw;
pub mod coral_tree;
pub mod delta_feature;
pub mod disk;
pub mod end_gateway;
pub mod end_island;
pub mod end_platform;
pub mod end_podium;
pub mod end_spike;
pub mod fallen_tree;
pub mod fill_layer;
pub mod fossil;
pub mod freeze_top_layer;
pub mod geode;
pub mod huge_fungus;
pub mod huge_mushroom;
pub mod iceberg;
pub mod lake;
pub mod large_dripstone;
pub mod monster_room;
pub mod multiface_growth;
pub mod netherrack_replace_blobs;
pub mod ore;
pub mod projected_random_patchy_square;
pub mod random_neighbor_spread;
pub mod replace_single_block;
pub mod root_system;
pub mod scattered_ore;
pub mod sculk_patch;
pub mod simple_block;
pub mod single_block_pillar;
pub mod speleothem;
pub mod speleothem_cluster;
pub mod spike;
pub mod spring;
pub mod stepped_column_cluster;
pub mod template;
pub mod tree;
pub mod underwater_magma;
pub mod vegetation_patch;
pub mod vines;
pub mod void_start_platform;
pub mod weeping_vines;

use crate::world::worldgen::feature::PlacedFeature;

#[derive(Debug)]
pub enum Feature {
    Bamboo(bamboo::BambooConfig),
    BlockBlob(block_blob::BlockBlobConfig),
    BlockColumn(block_column::BlockColumnConfig),
    BlockPile(block_pile::BlockPileConfig),
    BlueIce,
    BonusChest,
    ChorusPlant,
    CoralClaw(&'static PlacedFeature),
    CoralTree(&'static PlacedFeature),
    DeltaFeature(delta_feature::DeltaConfig),
    Disk(disk::DiskConfig),
    EndGateway(end_gateway::EndGatewayConfig),
    EndIsland,
    EndPlatform,
    EndPodium(end_podium::EndPodiumFeature),
    EndSpike(end_spike::EndSpikeConfig),
    FallenTree(fallen_tree::FallenTreeConfig),
    FillLayer(fill_layer::FillLayerFeature),
    Fossil(fossil::FossilConfig),
    FreezeTopLayer,
    Geode(Box<geode::GeodeConfig>),
    HugeBrownMushroom(huge_mushroom::HugeMushroomConfig),
    HugeFungus(huge_fungus::HugeFungusConfig),
    HugeRedMushroom(huge_mushroom::HugeMushroomConfig),
    Iceberg(iceberg::IcebergConfig),
    Lake(lake::LakeConfig),
    LargeDripstone(large_dripstone::LargeDripstoneConfig),
    MonsterRoom,
    MultifaceGrowth(multiface_growth::MultifaceGrowthConfig),
    NetherrackReplaceBlobs(netherrack_replace_blobs::ReplaceSphereConfig),
    NoOp,
    Ore(ore::OreConfig),
    Overlay(Vec<&'static PlacedFeature>),
    ProjectedRandomPatchySquare(projected_random_patchy_square::ProjectedRandomPatchySquare),
    RandomBooleanSelector {
        feature_true: &'static PlacedFeature,
        feature_false: &'static PlacedFeature,
    },
    RandomNeighborSpread(random_neighbor_spread::RandomNeighborSpreadFeature),
    RandomSelector {
        features: Vec<WeightedPlacedFeature>,
        default: &'static PlacedFeature,
    },
    ReplaceSingleBlock(replace_single_block::ReplaceBlockFeature),
    RootSystem(Box<root_system::RootSystemConfig>),
    ScatteredOre(ore::OreConfig),
    SculkPatch(sculk_patch::SculkPatchConfig),
    Sequence(Vec<&'static PlacedFeature>),
    SimpleBlock(simple_block::SimpleBlockConfig),
    SimpleRandomSelector(Vec<&'static PlacedFeature>),
    SingleBlockPillar(single_block_pillar::SingleBlockPillarFeature),
    Speleothem(speleothem::SpeleothemConfig),
    SpeleothemCluster(speleothem_cluster::SpeleothemClusterConfig),
    Spike(spike::SpikeConfig),
    Spring(spring::SpringConfig),
    SteppedColumnCluster(stepped_column_cluster::SteppedColumnClusterFeature),
    Template(template::TemplateConfig),
    Tree(Box<tree::TreeConfig>),
    UnderwaterMagma(underwater_magma::UnderwaterMagmaConfig),
    VegetationPatch {
        config: Box<vegetation_patch::VegetationPatchConfig>,
        waterlogged: bool,
    },
    Vines,
    VoidStartPlatform,
    WeightedRandomSelector(Vec<WeightedRandomFeature>),
}

#[derive(Debug)]
pub struct WeightedRandomFeature {
    pub weight: i32,
    pub feature: &'static PlacedFeature,
}

#[derive(Debug)]
pub struct WeightedPlacedFeature {
    pub chance: f32,
    pub feature: &'static PlacedFeature,
}

impl Feature {
    pub fn place(
        &'static self,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        origin: BlockPosition,
    ) -> bool {
        match self {
            Self::Bamboo(config) => bamboo::place(region, random, origin, config),
            Self::BlockBlob(config) => block_blob::place(region, random, origin, config),
            Self::BlockColumn(config) => block_column::place(region, random, origin, config),
            Self::BlockPile(config) => block_pile::place(region, random, origin, config),
            Self::BlueIce => blue_ice::place(region, random, origin),
            Self::BonusChest => bonus_chest::place(region, random, origin),
            Self::ChorusPlant => chorus_plant::place(region, random, origin),
            Self::CoralClaw(feature) => coral_claw::place(region, random, origin, feature),
            Self::CoralTree(feature) => coral_tree::place(region, random, origin, feature),
            Self::DeltaFeature(config) => delta_feature::place(region, random, origin, config),
            Self::Disk(config) => disk::place(region, random, origin, config),
            Self::EndGateway(config) => end_gateway::place(region, origin, config),
            Self::EndIsland => end_island::place(region, random, origin),
            Self::EndPlatform => end_platform::place(region, origin),
            Self::EndPodium(feature) => feature.place(region, origin),
            Self::EndSpike(config) => end_spike::place(region, random, origin, config),
            Self::FallenTree(config) => fallen_tree::place(region, random, origin, config),
            Self::FillLayer(feature) => feature.place(region, origin),
            Self::Fossil(config) => fossil::place(region, random, origin, config),
            Self::FreezeTopLayer => freeze_top_layer::place(region, origin),
            Self::Geode(config) => geode::place(region, random, origin, config),
            Self::HugeBrownMushroom(config) => huge_mushroom::place(
                region,
                random,
                origin,
                config,
                huge_mushroom::MushroomVariant::Brown,
            ),
            Self::HugeFungus(config) => huge_fungus::place(region, random, origin, config),
            Self::HugeRedMushroom(config) => huge_mushroom::place(
                region,
                random,
                origin,
                config,
                huge_mushroom::MushroomVariant::Red,
            ),
            Self::Iceberg(config) => iceberg::place(region, random, origin, config),
            Self::Lake(config) => lake::place(region, random, origin, config),
            Self::LargeDripstone(config) => large_dripstone::place(region, random, origin, config),
            Self::MonsterRoom => monster_room::place(region, random, origin),
            Self::MultifaceGrowth(config) => {
                multiface_growth::place(region, random, origin, config)
            }
            Self::NetherrackReplaceBlobs(config) => {
                netherrack_replace_blobs::place(region, random, origin, config)
            }
            Self::NoOp => true,
            Self::Ore(config) => ore::place(region, random, origin, config),
            Self::Overlay(features) => {
                let mut placed_any = false;
                for feature in features {
                    placed_any |= feature.place(region, random, origin);
                }
                placed_any
            }
            Self::ProjectedRandomPatchySquare(feature) => feature.place(region, random, origin),
            Self::RandomBooleanSelector {
                feature_true,
                feature_false,
            } => {
                let chosen = if random.next_bool() {
                    feature_true
                } else {
                    feature_false
                };
                chosen.place(region, random, origin)
            }
            Self::RandomNeighborSpread(feature) => feature.place(region, random, origin),
            Self::RandomSelector { features, default } => {
                for entry in features {
                    if random.next_f32() < entry.chance {
                        return entry.feature.place(region, random, origin);
                    }
                }
                default.place(region, random, origin)
            }
            Self::ReplaceSingleBlock(feature) => feature.place(region, random, origin),
            Self::RootSystem(config) => root_system::place(region, random, origin, config),
            Self::ScatteredOre(config) => scattered_ore::place(region, random, origin, config),
            Self::SculkPatch(config) => sculk_patch::place(region, random, origin, config),
            Self::Sequence(features) => {
                for feature in features {
                    if !feature.place(region, random, origin) {
                        return false;
                    }
                }
                true
            }
            Self::SimpleBlock(config) => simple_block::place(region, random, origin, config),
            Self::SimpleRandomSelector(features) => {
                let index = random.next_i32_bounded(features.len() as i32) as usize;
                features[index].place(region, random, origin)
            }
            Self::SingleBlockPillar(feature) => feature.place(region, random, origin),
            Self::Speleothem(config) => speleothem::place(region, random, origin, config),
            Self::SpeleothemCluster(config) => {
                speleothem_cluster::place(region, random, origin, config)
            }
            Self::Spike(config) => spike::place(region, random, origin, config),
            Self::Spring(config) => spring::place(region, origin, config),
            Self::SteppedColumnCluster(feature) => feature.place(region, random, origin),
            Self::Template(config) => template::place(region, random, origin, config),
            Self::Tree(config) => tree::place(region, random, origin, config),
            Self::UnderwaterMagma(config) => {
                underwater_magma::place(region, random, origin, config)
            }
            Self::VegetationPatch {
                config,
                waterlogged,
            } => vegetation_patch::place(region, random, origin, config, *waterlogged),
            Self::Vines => vines::place(region, origin),
            Self::VoidStartPlatform => void_start_platform::place(region, origin),
            Self::WeightedRandomSelector(features) => {
                let total_weight: i32 = features.iter().map(|entry| entry.weight).sum();
                if total_weight <= 0 {
                    return false;
                }
                let mut roll = random.next_i32_bounded(total_weight);
                for entry in features {
                    roll -= entry.weight;
                    if roll < 0 {
                        return entry.feature.place(region, random, origin);
                    }
                }
                false
            }
        }
    }
}

use crate::util::rng::RandomSource;
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util::BlockPosition;
use shulkr::util::Direction;
use shulkr::world::block::BlockState;

pub const DIRECTIONS: [Direction; 6] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];

pub const HORIZONTAL_DIRECTIONS: [Direction; 4] = Direction::HORIZONTAL;

fn check_neighbors(
    region: &WorldGenRegion<'_>,
    pos: BlockPosition,
    predicate: impl Fn(BlockState) -> bool,
) -> bool {
    DIRECTIONS
        .iter()
        .any(|direction| predicate(region.get_block(pos.relative(*direction))))
}

pub fn is_adjacent_to_air(region: &WorldGenRegion<'_>, pos: BlockPosition) -> bool {
    check_neighbors(region, pos, |state| state.is_air())
}

pub fn should_skip_air_check(
    random: &mut RandomSource,
    discard_chance_on_air_exposure: f32,
) -> bool {
    if discard_chance_on_air_exposure <= 0.0 {
        return true;
    }
    if discard_chance_on_air_exposure >= 1.0 {
        return false;
    }
    random.next_f32() >= discard_chance_on_air_exposure
}

pub fn mark_above_for_post_processing(region: &WorldGenRegion<'_>, place_pos: BlockPosition) {
    let mut pos = place_pos;
    for _ in 0..2 {
        pos = pos.above();
        if region.get_block(pos).is_air() {
            return;
        }
        region.mark_pos_for_postprocessing(pos);
    }
}
