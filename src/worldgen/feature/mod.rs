pub mod state_provider;
pub mod configured_feature;
pub mod featuresize;
pub mod features;
pub mod multiface;
pub mod sculk;
pub mod foliageplacers;
pub mod rootplacers;
pub mod rule_test;
pub mod sorter;
pub mod treedecorators;
pub mod trunkplacers;

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use rustc_hash::{FxHashMap, FxHashSet};
use serde_json::Value;

use crate::registry::REGISTRY;
use crate::registry::biome::BiomeRef;
use crate::util::datapack::{collect_json_files, path_to_id, read_and_parse_normalized};
use crate::util::datapack::{parse_id, parse_json, type_of, value_list};
use crate::util::ident::Identifier;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::worldgen::feature::configured_feature::{
    ConfiguredFeature, WeightedPlacedFeature, WeightedRandomFeature,
};
use crate::worldgen::feature::features::root_system::RootSystemConfig;
use crate::worldgen::feature::features::vegetation_patch::VegetationPatchConfig;
use crate::worldgen::placement::{PlacementContext, PlacementModifier, Positions};
use crate::worldgen::region::WorldGenRegion;

#[derive(Debug)]
pub struct PlacedFeature {
    pub id: usize,
    pub feature: &'static ConfiguredFeature,
    pub placement: Vec<PlacementModifier>,
}

impl PlacedFeature {
    pub fn place(
        &'static self,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        origin: BlockPos,
    ) -> bool {
        let context = PlacementContext::new(region, region.biome_zoom_seed());
        self.place_with_context(&context, random, origin)
    }

    pub fn place_with_biome_check(
        &'static self,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        origin: BlockPos,
    ) -> bool {
        let context =
            PlacementContext::new(region, region.biome_zoom_seed()).with_top_feature(self);
        self.place_with_context(&context, random, origin)
    }

    fn place_with_context(
        &'static self,
        context: &PlacementContext<'_, '_>,
        random: &mut RandomSource,
        origin: BlockPos,
    ) -> bool {
        let mut placed_any = false;
        self.run_placement(0, context, random, origin, &mut placed_any);
        placed_any
    }

    fn run_placement(
        &'static self,
        index: usize,
        context: &PlacementContext<'_, '_>,
        random: &mut RandomSource,
        origin: BlockPos,
        placed_any: &mut bool,
    ) {
        let Some(modifier) = self.placement.get(index) else {
            let before = random.draw_count();
            let ok = self.feature.place(context, random, origin);
            if let Some(trace) = trace_sink() {
                use std::io::Write as _;
                let mut trace = trace.lock().expect("trace lock");
                writeln!(
                    trace,
                    "PLACE {},{} {} {} {} draws={}->{} ok={} id={}",
                    context.region.center().0.x,
                    context.region.center().0.y,
                    origin.0.x,
                    origin.0.y,
                    origin.0.z,
                    before,
                    random.draw_count(),
                    ok,
                    std::ptr::from_ref(self.feature) as usize
                )
                .expect("trace write");
            }
            if ok {
                *placed_any = true;
            }
            return;
        };

        let mut positions = Positions::new();
        modifier.positions(context, random, origin, &mut positions);
        if positions.is_empty()
            && let Some(trace) = trace_sink()
        {
            use std::io::Write as _;
            let mut trace = trace.lock().expect("trace lock");
            writeln!(
                trace,
                "DROP {},{} {} {} {} mod={} {:?}",
                context.region.center().0.x,
                context.region.center().0.y,
                origin.0.x,
                origin.0.y,
                origin.0.z,
                index,
                modifier
            )
            .expect("trace write");
        }
        for pos in positions {
            self.run_placement(index + 1, context, random, pos, placed_any);
        }
    }
}

pub struct FeatureRegistry {
    configured: FxHashMap<Identifier, &'static ConfiguredFeature>,
    placed: FxHashMap<Identifier, &'static PlacedFeature>,
    biome_features: FxHashMap<usize, Vec<Vec<&'static PlacedFeature>>>,
    biome_feature_ids: FxHashMap<usize, FxHashSet<usize>>,
}

impl FeatureRegistry {
    pub fn configured(&self, key: &Identifier) -> Option<&'static ConfiguredFeature> {
        self.configured.get(key).copied()
    }

    pub fn placed(&self, key: &Identifier) -> Option<&'static PlacedFeature> {
        self.placed.get(key).copied()
    }

    pub fn features_for_biome(&self, biome: BiomeRef) -> &[Vec<&'static PlacedFeature>] {
        self.biome_features
            .get(&biome.id())
            .map_or(&[], Vec::as_slice)
    }

    pub fn configured_names(&self) -> FxHashMap<usize, Identifier> {
        self.configured
            .iter()
            .map(|(key, feature)| (std::ptr::from_ref(*feature) as usize, key.clone()))
            .collect()
    }

    pub fn placed_names(&self) -> FxHashMap<usize, Identifier> {
        self.placed
            .iter()
            .map(|(key, feature)| (feature.id, key.clone()))
            .collect()
    }

    pub fn biome_contains(&self, biome: BiomeRef, feature: &'static PlacedFeature) -> bool {
        self.biome_feature_ids
            .get(&biome.id())
            .is_some_and(|ids| ids.contains(&feature.id))
    }
}

static TRACE: LazyLock<Option<std::sync::Mutex<std::io::BufWriter<std::fs::File>>>> =
    LazyLock::new(|| {
        let path = std::env::var("WGTRACE").ok()?;
        let file = std::fs::File::create(path).expect("trace file");
        Some(std::sync::Mutex::new(std::io::BufWriter::new(file)))
    });

fn trace_sink() -> Option<&'static std::sync::Mutex<std::io::BufWriter<std::fs::File>>> {
    TRACE.as_ref()
}

pub fn flush_trace() {
    if let Some(trace) = trace_sink() {
        use std::io::Write as _;
        trace.lock().expect("trace lock").flush().expect("flush");
    }
}

static FEATURES: LazyLock<FeatureRegistry> = LazyLock::new(|| load(&vanilla_worldgen_dir()));

pub fn features() -> &'static FeatureRegistry {
    &FEATURES
}

fn vanilla_worldgen_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/datapack/worldgen")
}

struct Loader {
    configured_json: FxHashMap<Identifier, Value>,
    placed_json: FxHashMap<Identifier, Value>,
    configured: FxHashMap<Identifier, &'static ConfiguredFeature>,
    placed: FxHashMap<Identifier, &'static PlacedFeature>,
    resolving_placed: FxHashSet<Identifier>,
    resolving_configured: FxHashSet<Identifier>,
    next_placed_id: usize,
}

fn load(worldgen_dir: &Path) -> FeatureRegistry {
    let mut loader = Loader {
        configured_json: read_directory(&worldgen_dir.join("configured_feature")),
        placed_json: read_directory(&worldgen_dir.join("placed_feature")),
        configured: FxHashMap::default(),
        placed: FxHashMap::default(),
        resolving_placed: FxHashSet::default(),
        resolving_configured: FxHashSet::default(),
        next_placed_id: 0,
    };

    let placed_keys: Vec<Identifier> = loader.placed_json.keys().cloned().collect();
    for key in placed_keys {
        loader.placed_feature(&key);
    }

    let mut biome_features = FxHashMap::default();
    let mut biome_feature_ids = FxHashMap::default();
    for (id, biome) in REGISTRY.biomes.iter() {
        let steps: Vec<Vec<&'static PlacedFeature>> = biome
            .features
            .iter()
            .map(|step| {
                step.iter()
                    .map(|key| {
                        loader.placed.get(key).copied().unwrap_or_else(|| {
                            panic!(
                                "biome {} references unknown placed feature {key}",
                                biome.key
                            )
                        })
                    })
                    .collect()
            })
            .collect();
        let ids: FxHashSet<usize> = steps
            .iter()
            .flat_map(|step| step.iter().map(|feature| feature.id))
            .collect();
        biome_features.insert(id, steps);
        biome_feature_ids.insert(id, ids);
    }

    FeatureRegistry {
        configured: loader.configured,
        placed: loader.placed,
        biome_features,
        biome_feature_ids,
    }
}

fn read_directory(directory: &Path) -> FxHashMap<Identifier, Value> {
    collect_json_files(directory)
        .into_iter()
        .map(|file| {
            let key = parse_id(&path_to_id("minecraft", &file, directory));
            let json: Value = read_and_parse_normalized(&file)
                .unwrap_or_else(|e| panic!("failed to load {}: {e}", file.display()));
            (key, json)
        })
        .collect()
}

impl Loader {
    fn placed_feature(&mut self, key: &Identifier) -> &'static PlacedFeature {
        if let Some(feature) = self.placed.get(key).copied() {
            return feature;
        }
        assert!(
            self.resolving_placed.insert(key.clone()),
            "cycle while resolving placed feature {key}"
        );

        let json = self
            .placed_json
            .get(key)
            .unwrap_or_else(|| panic!("unknown placed feature {key}"))
            .clone();
        let feature = self.build_placed(&json);

        self.resolving_placed.remove(key);
        self.placed.insert(key.clone(), feature);
        feature
    }

    fn build_placed(&mut self, json: &Value) -> &'static PlacedFeature {
        let configured = self.configured_feature_ref(&json["feature"]);
        let placement = value_list(&json["placement"])
            .iter()
            .map(parse_json)
            .collect();
        let id = self.next_placed_id;
        self.next_placed_id += 1;

        Box::leak(Box::new(PlacedFeature {
            id,
            feature: configured,
            placement,
        }))
    }

    fn placed_feature_ref(&mut self, value: &Value) -> &'static PlacedFeature {
        match value.as_str() {
            Some(raw) => {
                let key = parse_id(raw);
                self.placed_feature(&key)
            }
            None => self.build_placed(value),
        }
    }

    fn configured_feature_ref(&mut self, value: &Value) -> &'static ConfiguredFeature {
        match value.as_str() {
            Some(raw) => {
                let key = parse_id(raw);
                self.configured_feature(&key)
            }
            None => self.build_configured(value),
        }
    }

    fn configured_feature(&mut self, key: &Identifier) -> &'static ConfiguredFeature {
        if let Some(feature) = self.configured.get(key).copied() {
            return feature;
        }
        assert!(
            self.resolving_configured.insert(key.clone()),
            "cycle while resolving configured feature {key}"
        );

        let json = self
            .configured_json
            .get(key)
            .unwrap_or_else(|| panic!("unknown configured feature {key}"))
            .clone();
        let feature = self.build_configured(&json);

        self.resolving_configured.remove(key);
        self.configured.insert(key.clone(), feature);
        feature
    }

    fn root_system_config(&mut self, config: &Value) -> RootSystemConfig {
        RootSystemConfig {
            feature: self.placed_feature_ref(&config["feature"]),
            required_vertical_space_for_tree: parse_json(&config["required_vertical_space_for_tree"]),
            level_test_distance: parse_json(&config["level_test_distance"]),
            max_level_deviation: parse_json(&config["max_level_deviation"]),
            root_radius: parse_json(&config["root_radius"]),
            root_replaceable: parse_json(&config["root_replaceable"]),
            root_state_provider: parse_json(&config["root_state_provider"]),
            root_placement_attempts: parse_json(&config["root_placement_attempts"]),
            root_column_max_height: parse_json(&config["root_column_max_height"]),
            hanging_root_radius: parse_json(&config["hanging_root_radius"]),
            hanging_roots_vertical_span: parse_json(&config["hanging_roots_vertical_span"]),
            hanging_root_state_provider: parse_json(&config["hanging_root_state_provider"]),
            hanging_root_placement_attempts: parse_json(&config["hanging_root_placement_attempts"]),
            allowed_vertical_water_for_tree: parse_json(&config["allowed_vertical_water_for_tree"]),
            allowed_tree_position: parse_json(&config["allowed_tree_position"]),
        }
    }

    fn vegetation_patch_config(&mut self, config: &Value) -> VegetationPatchConfig {
        VegetationPatchConfig {
            replaceable: parse_json(&config["replaceable"]),
            ground_state: parse_json(&config["ground_state"]),
            vegetation_feature: self.placed_feature_ref(&config["vegetation_feature"]),
            surface: parse_json(&config["surface"]),
            depth: parse_json(&config["depth"]),
            extra_bottom_block_chance: parse_json(&config["extra_bottom_block_chance"]),
            vertical_range: parse_json(&config["vertical_range"]),
            vegetation_chance: parse_json(&config["vegetation_chance"]),
            xz_radius: parse_json(&config["xz_radius"]),
            extra_edge_column_chance: parse_json(&config["extra_edge_column_chance"]),
        }
    }

    fn build_configured(&mut self, json: &Value) -> &'static ConfiguredFeature {
        let feature_type = parse_id(type_of(json));
        let config = &json["config"];
        let feature = match feature_type.path.as_ref() {
            "random_selector" => {
                let features = value_list(&config["features"])
                    .iter()
                    .map(|entry| WeightedPlacedFeature {
                        chance: entry["chance"].as_f64().unwrap_or_default() as f32,
                        feature: self.placed_feature_ref(&entry["feature"]),
                    })
                    .collect::<Vec<_>>();
                ConfiguredFeature::RandomSelector {
                    features,
                    default: self.placed_feature_ref(&config["default"]),
                }
            }
            "simple_random_selector" => ConfiguredFeature::SimpleRandomSelector(
                value_list(&config["features"])
                    .iter()
                    .map(|entry| self.placed_feature_ref(entry))
                    .collect(),
            ),
            "sequence" => ConfiguredFeature::Sequence(
                value_list(&config["features"])
                    .iter()
                    .map(|entry| self.placed_feature_ref(entry))
                    .collect(),
            ),
            "weighted_random_selector" => ConfiguredFeature::WeightedRandomSelector(
                value_list(&config["features"])
                    .iter()
                    .map(|entry| WeightedRandomFeature {
                        weight: entry["weight"].as_i64().unwrap_or(1) as i32,
                        feature: self.placed_feature_ref(&entry["data"]),
                    })
                    .collect(),
            ),
            "random_boolean_selector" => ConfiguredFeature::RandomBooleanSelector {
                feature_true: self.placed_feature_ref(&config["feature_true"]),
                feature_false: self.placed_feature_ref(&config["feature_false"]),
            },
            "ore" => ConfiguredFeature::Ore(parse_json(config)),
            "scattered_ore" => ConfiguredFeature::ScatteredOre(parse_json(config)),
            "spring_feature" => ConfiguredFeature::Spring(parse_json(config)),
            "disk" => ConfiguredFeature::Disk(parse_json(config)),
            "lake" => ConfiguredFeature::Lake(parse_json(config)),
            "block_column" => ConfiguredFeature::BlockColumn(parse_json(config)),
            "block_pile" => ConfiguredFeature::BlockPile(parse_json(config)),
            "simple_block" => ConfiguredFeature::SimpleBlock(parse_json(config)),
            "vines" => ConfiguredFeature::Vines,
            "tree" => ConfiguredFeature::Tree(parse_json(config)),
            "huge_red_mushroom" => ConfiguredFeature::HugeRedMushroom(parse_json(config)),
            "huge_brown_mushroom" => ConfiguredFeature::HugeBrownMushroom(parse_json(config)),
            "multiface_growth" => ConfiguredFeature::MultifaceGrowth(parse_json(config)),
            "fallen_tree" => ConfiguredFeature::FallenTree(parse_json(config)),
            "block_blob" => ConfiguredFeature::BlockBlob(parse_json(config)),
            "basalt_pillar" => ConfiguredFeature::BasaltPillar,
            "blue_ice" => ConfiguredFeature::BlueIce,
            "glowstone_blob" => ConfiguredFeature::GlowstoneBlob,
            "kelp" => ConfiguredFeature::Kelp,
            "seagrass" => ConfiguredFeature::Seagrass(parse_json(config)),
            "sea_pickle" => ConfiguredFeature::SeaPickle(parse_json(config)),
            "end_island" => ConfiguredFeature::EndIsland,
            "end_platform" => ConfiguredFeature::EndPlatform,
            "void_start_platform" => ConfiguredFeature::VoidStartPlatform,
            "basalt_columns" => ConfiguredFeature::BasaltColumns(parse_json(config)),
            "delta_feature" => ConfiguredFeature::DeltaFeature(parse_json(config)),
            "netherrack_replace_blobs" => {
                ConfiguredFeature::NetherrackReplaceBlobs(parse_json(config))
            }
            "nether_forest_vegetation" => {
                ConfiguredFeature::NetherForestVegetation(parse_json(config))
            }
            "twisting_vines" => ConfiguredFeature::TwistingVines(parse_json(config)),
            "weeping_vines" => ConfiguredFeature::WeepingVines,
            "underwater_magma" => ConfiguredFeature::UnderwaterMagma(parse_json(config)),
            "bamboo" => ConfiguredFeature::Bamboo(parse_json(config)),
            "desert_well" => ConfiguredFeature::DesertWell,
            "spike" => ConfiguredFeature::Spike(parse_json(config)),
            "chorus_plant" => ConfiguredFeature::ChorusPlant,
            "root_system" => ConfiguredFeature::RootSystem(self.root_system_config(config)),
            "huge_fungus" => ConfiguredFeature::HugeFungus(parse_json(config)),
            "iceberg" => ConfiguredFeature::Iceberg(parse_json(config)),
            "sculk_patch" => ConfiguredFeature::SculkPatch(parse_json(config)),
            "speleothem_cluster" => ConfiguredFeature::SpeleothemCluster(parse_json(config)),
            "large_dripstone" => ConfiguredFeature::LargeDripstone(parse_json(config)),
            "geode" => ConfiguredFeature::Geode(parse_json(config)),
            "freeze_top_layer" => ConfiguredFeature::FreezeTopLayer,
            "monster_room" => ConfiguredFeature::MonsterRoom,
            "bonus_chest" => ConfiguredFeature::BonusChest,
            "end_spike" => ConfiguredFeature::EndSpike(parse_json(config)),
            "coral_tree" => ConfiguredFeature::CoralTree,
            "coral_claw" => ConfiguredFeature::CoralClaw,
            "coral_mushroom" => ConfiguredFeature::CoralMushroom,
            "vegetation_patch" => ConfiguredFeature::VegetationPatch {
                config: self.vegetation_patch_config(config),
                waterlogged: false,
            },
            "waterlogged_vegetation_patch" => ConfiguredFeature::VegetationPatch {
                config: self.vegetation_patch_config(config),
                waterlogged: true,
            },
            _ => ConfiguredFeature::Unported(feature_type),
        };

        Box::leak(Box::new(feature))
    }
}

#[cfg(test)]
mod tests {
    use std::ptr;
    use std::sync::Once;

    use super::*;
    use crate::registry::Registries;

    fn init_global_registry() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let mut registries = Registries::new_vanilla().expect("vanilla datapack should load");
            registries.freeze();
            let _ = REGISTRY.init(registries);
        });
    }

    #[test]
    fn loads_every_vanilla_placed_feature() {
        init_global_registry();
        let registry = features();

        let oak = registry
            .placed(&Identifier::const_vanilla("oak_checked"))
            .expect("oak_checked should load");
        assert!(ptr::eq(
            oak.feature,
            registry
                .configured(&Identifier::const_vanilla("oak"))
                .expect("oak should load")
        ));
        assert_eq!(oak.placement.len(), 1);

        let ore_granite = registry
            .placed(&Identifier::const_vanilla("ore_granite_upper"))
            .expect("ore_granite_upper should load");
        assert_eq!(ore_granite.placement.len(), 4);
    }

    #[test]
    fn resolves_selector_sub_features() {
        init_global_registry();
        let registry = features();

        let trees_plains = registry
            .configured(&Identifier::const_vanilla("trees_plains"))
            .expect("trees_plains should load");
        let ConfiguredFeature::RandomSelector { features, default } = trees_plains else {
            panic!("trees_plains should be a random selector");
        };
        assert_eq!(features.len(), 2);
        assert!(ptr::eq(
            default.feature,
            registry
                .configured(&Identifier::const_vanilla("oak_bees_005"))
                .expect("oak_bees_005 should load")
        ));
        assert!(ptr::eq(
            features[1].feature,
            registry
                .placed(&Identifier::const_vanilla("fallen_oak_tree"))
                .expect("fallen_oak_tree should load")
        ));
    }

    #[test]
    fn ported_feature_types_parse_into_their_own_kinds() {
        init_global_registry();
        let registry = features();

        let feature_of = |key: &str| {
            registry
                .configured(&parse_id(key))
                .unwrap_or_else(|| panic!("{key} should load"))
        };

        assert!(matches!(feature_of("ore_coal"), ConfiguredFeature::Ore(_)));
        assert!(matches!(
            feature_of("ore_ancient_debris_small"),
            ConfiguredFeature::ScatteredOre(_)
        ));
        assert!(matches!(
            feature_of("spring_lava_overworld"),
            ConfiguredFeature::Spring(_)
        ));
        assert!(matches!(
            feature_of("disk_clay"),
            ConfiguredFeature::Disk(_)
        ));
        assert!(matches!(
            feature_of("cactus"),
            ConfiguredFeature::BlockColumn(_)
        ));
        assert!(matches!(
            feature_of("pile_hay"),
            ConfiguredFeature::BlockPile(_)
        ));
        assert!(matches!(
            feature_of("tall_grass"),
            ConfiguredFeature::SimpleBlock(_)
        ));
        assert!(matches!(feature_of("vines"), ConfiguredFeature::Vines));

        let ConfiguredFeature::Ore(ore) = feature_of("ore_coal") else {
            panic!("ore_coal should be an ore");
        };
        assert_eq!(ore.size, 17);
        assert_eq!(ore.targets.len(), 2);
    }

    #[test]
    fn biome_feature_membership_matches_biome_definition() {
        init_global_registry();
        let registry = features();

        let plains = REGISTRY
            .biomes
            .by_key(&Identifier::const_vanilla("plains"))
            .expect("plains biome");
        let trees_plains = registry
            .placed(&Identifier::const_vanilla("trees_plains"))
            .expect("trees_plains placed feature");
        let iceberg = registry
            .placed(&Identifier::const_vanilla("iceberg_packed"))
            .expect("iceberg_packed placed feature");

        assert!(registry.biome_contains(plains, trees_plains));
        assert!(!registry.biome_contains(plains, iceberg));
        assert_eq!(registry.features_for_biome(plains).len(), 11);
    }
}
