use crate::util::ident::Identifier;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::worldgen::feature::PlacedFeature;
use crate::worldgen::feature::features::bamboo::{self, BambooConfig};
use crate::worldgen::feature::features::basalt_columns::{self, ColumnConfig};
use crate::worldgen::feature::features::block_blob::{self, BlockBlobConfig};
use crate::worldgen::feature::features::block_column::{self, BlockColumnConfig};
use crate::worldgen::feature::features::block_pile::{self, BlockPileConfig};
use crate::worldgen::feature::features::delta_feature::{self, DeltaConfig};
use crate::worldgen::feature::features::disk::{self, DiskConfig};
use crate::worldgen::feature::features::end_gateway::{self, EndGatewayConfig};
use crate::worldgen::feature::features::end_spike::{self, EndSpikeConfig};
use crate::worldgen::feature::features::fallen_tree::{self, FallenTreeConfig};
use crate::worldgen::feature::features::fossil::{self, FossilConfig};
use crate::worldgen::feature::features::geode::{self, GeodeConfig};
use crate::worldgen::feature::features::huge_fungus::{self, HugeFungusConfig};
use crate::worldgen::feature::features::huge_mushroom::{
    self, HugeMushroomConfig, MushroomVariant,
};
use crate::worldgen::feature::features::iceberg::{self, IcebergConfig};
use crate::worldgen::feature::features::lake::{self, LakeConfig};
use crate::worldgen::feature::features::large_dripstone::{self, LargeDripstoneConfig};
use crate::worldgen::feature::features::multiface_growth::{self, MultifaceGrowthConfig};
use crate::worldgen::feature::features::nether_forest_vegetation::{
    self, NetherForestVegetationConfig,
};
use crate::worldgen::feature::features::netherrack_replace_blobs::{self, ReplaceSphereConfig};
use crate::worldgen::feature::features::ore::{self, OreConfig};
use crate::worldgen::feature::features::root_system::{self, RootSystemConfig};
use crate::worldgen::feature::features::sculk_patch::{self, SculkPatchConfig};
use crate::worldgen::feature::features::sea_pickle::{self, SeaPickleConfig};
use crate::worldgen::feature::features::seagrass::{self, SeagrassConfig};
use crate::worldgen::feature::features::simple_block::{self, SimpleBlockConfig};
use crate::worldgen::feature::features::speleothem::{self, SpeleothemConfig};
use crate::worldgen::feature::features::speleothem_cluster::{self, SpeleothemClusterConfig};
use crate::worldgen::feature::features::spike::{self, SpikeConfig};
use crate::worldgen::feature::features::spring::{self, SpringConfig};
use crate::worldgen::feature::features::template::{self, TemplateConfig};
use crate::worldgen::feature::features::tree::{self, TreeConfig};
use crate::worldgen::feature::features::twisting_vines::{self, TwistingVinesConfig};
use crate::worldgen::feature::features::underwater_magma::{self, UnderwaterMagmaConfig};
use crate::worldgen::feature::features::vegetation_patch::{self, VegetationPatchConfig};
use crate::worldgen::feature::features::{
    basalt_pillar, blue_ice, bonus_chest, chorus_plant, coral_claw, coral_mushroom, coral_tree,
    desert_well, end_island, end_platform, freeze_top_layer, glowstone_blob, kelp, monster_room,
    scattered_ore, vines, void_start_platform, weeping_vines,
};
use crate::worldgen::placement::PlacementContext;

#[derive(Debug)]
pub enum ConfiguredFeature {
    RandomSelector {
        features: Vec<WeightedPlacedFeature>,
        default: &'static PlacedFeature,
    },
    SimpleRandomSelector(Vec<&'static PlacedFeature>),
    Sequence(Vec<&'static PlacedFeature>),
    WeightedRandomSelector(Vec<WeightedRandomFeature>),
    RandomBooleanSelector {
        feature_true: &'static PlacedFeature,
        feature_false: &'static PlacedFeature,
    },
    Ore(OreConfig),
    ScatteredOre(OreConfig),
    Spring(SpringConfig),
    Disk(DiskConfig),
    Lake(LakeConfig),
    BlockColumn(BlockColumnConfig),
    BlockPile(BlockPileConfig),
    SimpleBlock(SimpleBlockConfig),
    Vines,
    Template(TemplateConfig),
    Tree(TreeConfig),
    HugeRedMushroom(HugeMushroomConfig),
    HugeBrownMushroom(HugeMushroomConfig),
    MultifaceGrowth(MultifaceGrowthConfig),
    FallenTree(FallenTreeConfig),
    BlockBlob(BlockBlobConfig),
    BasaltPillar,
    BlueIce,
    GlowstoneBlob,
    Kelp,
    Seagrass(SeagrassConfig),
    SeaPickle(SeaPickleConfig),
    EndIsland,
    EndPlatform,
    VoidStartPlatform,
    BasaltColumns(ColumnConfig),
    DeltaFeature(DeltaConfig),
    NetherrackReplaceBlobs(ReplaceSphereConfig),
    NetherForestVegetation(NetherForestVegetationConfig),
    TwistingVines(TwistingVinesConfig),
    WeepingVines,
    UnderwaterMagma(UnderwaterMagmaConfig),
    Bamboo(BambooConfig),
    DesertWell,
    Spike(SpikeConfig),
    ChorusPlant,
    RootSystem(RootSystemConfig),
    HugeFungus(HugeFungusConfig),
    Iceberg(IcebergConfig),
    SculkPatch(SculkPatchConfig),
    Speleothem(SpeleothemConfig),
    SpeleothemCluster(SpeleothemClusterConfig),
    LargeDripstone(LargeDripstoneConfig),
    Geode(GeodeConfig),
    FreezeTopLayer,
    MonsterRoom,
    BonusChest,
    EndSpike(EndSpikeConfig),
    CoralTree,
    CoralClaw,
    CoralMushroom,
    Fossil(FossilConfig),
    EndGateway(EndGatewayConfig),
    VegetationPatch {
        config: VegetationPatchConfig,
        waterlogged: bool,
    },
    Unported(Identifier),
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

impl ConfiguredFeature {
    pub fn place(
        &'static self,
        context: &PlacementContext<'_, '_>,
        random: &mut RandomSource,
        origin: BlockPos,
    ) -> bool {
        match self {
            Self::RandomSelector { features, default } => {
                for entry in features {
                    if random.next_f32() < entry.chance {
                        return entry.feature.place(context.region, random, origin);
                    }
                }
                default.place(context.region, random, origin)
            }
            Self::Sequence(features) => {
                for feature in features {
                    if !feature.place(context.region, random, origin) {
                        return false;
                    }
                }
                true
            }
            Self::WeightedRandomSelector(features) => {
                let total_weight: i32 = features.iter().map(|entry| entry.weight).sum();
                if total_weight <= 0 {
                    return false;
                }
                let mut roll = random.next_i32_bounded(total_weight);
                for entry in features {
                    roll -= entry.weight;
                    if roll < 0 {
                        return entry.feature.place(context.region, random, origin);
                    }
                }
                false
            }
            Self::SimpleRandomSelector(features) => {
                if features.is_empty() {
                    return false;
                }
                let index = random.next_i32_bounded(features.len() as i32) as usize;
                features[index].place(context.region, random, origin)
            }
            Self::RandomBooleanSelector {
                feature_true,
                feature_false,
            } => {
                let chosen = if random.next_bool() {
                    feature_true
                } else {
                    feature_false
                };
                chosen.place(context.region, random, origin)
            }
            Self::Ore(config) => ore::place(context.region, random, origin, config),
            Self::ScatteredOre(config) => {
                scattered_ore::place(context.region, random, origin, config)
            }
            Self::Spring(config) => spring::place(context.region, origin, config),
            Self::Disk(config) => disk::place(context.region, random, origin, config),
            Self::Lake(config) => lake::place(context.region, random, origin, config),
            Self::BlockColumn(config) => {
                block_column::place(context.region, random, origin, config)
            }
            Self::BlockPile(config) => block_pile::place(context.region, random, origin, config),
            Self::SimpleBlock(config) => {
                simple_block::place(context.region, random, origin, config)
            }
            Self::Vines => vines::place(context.region, origin),
            Self::Template(config) => template::place(context.region, random, origin, config),
            Self::Tree(config) => tree::place(context.region, random, origin, config),
            Self::HugeRedMushroom(config) => {
                huge_mushroom::place(context.region, random, origin, config, MushroomVariant::Red)
            }
            Self::HugeBrownMushroom(config) => huge_mushroom::place(
                context.region,
                random,
                origin,
                config,
                MushroomVariant::Brown,
            ),
            Self::MultifaceGrowth(config) => {
                multiface_growth::place(context.region, random, origin, config)
            }
            Self::FallenTree(config) => fallen_tree::place(context.region, random, origin, config),
            Self::BlockBlob(config) => block_blob::place(context.region, random, origin, config),
            Self::BasaltPillar => basalt_pillar::place(context.region, random, origin),
            Self::BlueIce => blue_ice::place(context.region, random, origin),
            Self::GlowstoneBlob => glowstone_blob::place(context.region, random, origin),
            Self::Kelp => kelp::place(context.region, random, origin),
            Self::Seagrass(config) => seagrass::place(context.region, random, origin, config),
            Self::SeaPickle(config) => sea_pickle::place(context.region, random, origin, config),
            Self::EndIsland => end_island::place(context.region, random, origin),
            Self::EndPlatform => end_platform::place(context.region, origin),
            Self::VoidStartPlatform => void_start_platform::place(context.region, origin),
            Self::BasaltColumns(config) => {
                basalt_columns::place(context.region, random, origin, config)
            }
            Self::DeltaFeature(config) => {
                delta_feature::place(context.region, random, origin, config)
            }
            Self::NetherrackReplaceBlobs(config) => {
                netherrack_replace_blobs::place(context.region, random, origin, config)
            }
            Self::NetherForestVegetation(config) => {
                nether_forest_vegetation::place(context.region, random, origin, config)
            }
            Self::TwistingVines(config) => {
                twisting_vines::place(context.region, random, origin, config)
            }
            Self::WeepingVines => weeping_vines::place(context.region, random, origin),
            Self::UnderwaterMagma(config) => {
                underwater_magma::place(context.region, random, origin, config)
            }
            Self::Bamboo(config) => bamboo::place(context.region, random, origin, config),
            Self::DesertWell => desert_well::place(context.region, random, origin),
            Self::Spike(config) => spike::place(context.region, random, origin, config),
            Self::ChorusPlant => chorus_plant::place(context.region, random, origin),
            Self::RootSystem(config) => root_system::place(context.region, random, origin, config),
            Self::HugeFungus(config) => huge_fungus::place(context.region, random, origin, config),
            Self::Iceberg(config) => iceberg::place(context.region, random, origin, config),
            Self::SculkPatch(config) => sculk_patch::place(context.region, random, origin, config),
            Self::Speleothem(config) => speleothem::place(context.region, random, origin, config),
            Self::SpeleothemCluster(config) => {
                speleothem_cluster::place(context.region, random, origin, config)
            }
            Self::LargeDripstone(config) => {
                large_dripstone::place(context.region, random, origin, config)
            }
            Self::Geode(config) => geode::place(context.region, random, origin, config),
            Self::FreezeTopLayer => freeze_top_layer::place(context.region, origin),
            Self::MonsterRoom => monster_room::place(context.region, random, origin),
            Self::BonusChest => bonus_chest::place(context.region, random, origin),
            Self::EndSpike(config) => end_spike::place(context.region, random, origin, config),
            Self::CoralTree => coral_tree::place(context.region, random, origin),
            Self::CoralClaw => coral_claw::place(context.region, random, origin),
            Self::CoralMushroom => coral_mushroom::place(context.region, random, origin),
            Self::Fossil(config) => fossil::place(context.region, random, origin, config),
            Self::EndGateway(config) => end_gateway::place(context.region, origin, config),
            Self::VegetationPatch {
                config,
                waterlogged,
            } => vegetation_patch::place(context.region, random, origin, config, *waterlogged),
            Self::Unported(_) => false,
        }
    }
}
