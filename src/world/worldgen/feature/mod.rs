pub mod features;
pub mod featuresize;
pub mod foliageplacers;
pub mod multiface;
pub mod rootplacers;
pub mod rule_test;
pub mod sculk;
pub mod sorter;
pub mod state_provider;
pub mod treedecorators;
pub mod trunkplacers;

use std::sync::LazyLock;

use rustc_hash::{FxHashMap, FxHashSet};
use serde_json::Value;
use shulkr::util::Key;

use crate::datapack::{self, Datapack};
use crate::datapack::{collect_json_files, path_to_id, read_and_parse_normalized};
use crate::datapack::{parse_id, parse_json, type_of, value_list};
use crate::registry::biome::BiomeRef;
use shulkr::registry::{Id, registries};

use crate::util::rng::RandomSource;
use crate::world::worldgen::block_predicate::BlockPredicate;
use crate::world::worldgen::feature::features::root_system::RootSystemConfig;
use crate::world::worldgen::feature::features::single_block_pillar;
use crate::world::worldgen::feature::features::vegetation_patch::VegetationPatchConfig;
use crate::world::worldgen::feature::features::{
    Feature, WeightedPlacedFeature, WeightedRandomFeature,
};
use crate::world::worldgen::placement::{PlacementContext, PlacementModifier, Positions};
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util::BlockPosition;
use std::env;
use std::fs;
use std::fs::File;
use std::io;
use std::io::BufWriter;
use std::ptr;
use std::sync;
use std::sync::Mutex;

#[derive(Debug)]
pub struct PlacedFeature {
    pub id: usize,
    pub feature: &'static Feature,
    pub placement: Vec<PlacementModifier>,
}

impl PlacedFeature {
    pub fn place(
        &'static self,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        origin: BlockPosition,
    ) -> bool {
        let context = PlacementContext::new(region);
        self.place_with_context(&context, random, origin)
    }

    pub fn place_with_biome_check(
        &'static self,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        origin: BlockPosition,
    ) -> bool {
        let context = PlacementContext::new(region).with_top_feature(self);
        self.place_with_context(&context, random, origin)
    }

    fn place_with_context(
        &'static self,
        context: &PlacementContext<'_, '_>,
        random: &mut RandomSource,
        origin: BlockPosition,
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
        origin: BlockPosition,
        placed_any: &mut bool,
    ) {
        let Some(modifier) = self.placement.get(index) else {
            let before = random.draw_count();
            let ok = self.feature.place(context.region, random, origin);
            if let Some(trace) = trace_sink() {
                use std::io::Write as _;
                let mut trace = trace.lock().expect("trace lock");
                let name = trace_names()
                    .get(&(ptr::from_ref(self.feature) as usize))
                    .map_or_else(|| "inline".to_owned(), ToString::to_string);
                writeln!(
                    trace,
                    "PLACE {},{} {} {} {} draws={}->{} ok={} {}",
                    context.region.center().0.x,
                    context.region.center().0.y,
                    origin.x(),
                    origin.y(),
                    origin.z(),
                    before,
                    random.draw_count(),
                    ok,
                    name
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
                origin.x(),
                origin.y(),
                origin.z(),
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
    configured: FxHashMap<Key, &'static Feature>,
    placed: FxHashMap<Key, &'static PlacedFeature>,
    biome_features: FxHashMap<usize, Vec<Vec<&'static PlacedFeature>>>,
    biome_feature_ids: FxHashMap<usize, FxHashSet<usize>>,
}

impl FeatureRegistry {
    pub fn configured(&self, key: &Key) -> Option<&'static Feature> {
        self.configured.get(key).copied()
    }

    pub fn placed(&self, key: &Key) -> Option<&'static PlacedFeature> {
        self.placed.get(key).copied()
    }

    pub fn features_for_biome(&self, biome: BiomeRef) -> &[Vec<&'static PlacedFeature>] {
        self.biome_features
            .get(&usize::from(biome.id()))
            .map_or(&[], Vec::as_slice)
    }

    pub fn configured_names(&self) -> FxHashMap<usize, Key> {
        self.configured
            .iter()
            .map(|(key, feature)| (ptr::from_ref(*feature) as usize, key.clone()))
            .collect()
    }

    pub fn biome_contains(&self, biome: BiomeRef, feature: &'static PlacedFeature) -> bool {
        self.biome_feature_ids
            .get(&usize::from(biome.id()))
            .is_some_and(|ids| ids.contains(&feature.id))
    }
}

static TRACE: LazyLock<Option<Mutex<BufWriter<File>>>> = LazyLock::new(|| {
    let path = env::var("WGTRACE").ok()?;
    let file = fs::File::create(path).expect("trace file");
    Some(sync::Mutex::new(io::BufWriter::new(file)))
});

fn trace_names() -> &'static FxHashMap<usize, Key> {
    static NAMES: LazyLock<FxHashMap<usize, Key>> = LazyLock::new(|| features().configured_names());
    &NAMES
}

fn trace_sink() -> Option<&'static Mutex<BufWriter<File>>> {
    TRACE.as_ref()
}

pub fn flush_trace() {
    if let Some(trace) = trace_sink() {
        use std::io::Write as _;
        trace.lock().expect("trace lock").flush().expect("flush");
    }
}

static FEATURES: LazyLock<FeatureRegistry> = LazyLock::new(|| load(datapack::active()));

pub fn features() -> &'static FeatureRegistry {
    &FEATURES
}

struct Loader {
    configured_json: FxHashMap<Key, Value>,
    placed_json: FxHashMap<Key, Value>,
    configured: FxHashMap<Key, &'static Feature>,
    placed: FxHashMap<Key, &'static PlacedFeature>,
    resolving_placed: FxHashSet<Key>,
    resolving_configured: FxHashSet<Key>,
    next_placed_id: usize,
}

fn load(pack: &Datapack) -> FeatureRegistry {
    let mut loader = Loader {
        configured_json: read_directory(pack, "worldgen/feature"),
        placed_json: read_directory(pack, "worldgen/placed_feature"),
        configured: FxHashMap::default(),
        placed: FxHashMap::default(),
        resolving_placed: FxHashSet::default(),
        resolving_configured: FxHashSet::default(),
        next_placed_id: 0,
    };

    let configured_keys: Vec<Key> = loader.configured_json.keys().cloned().collect();
    for key in configured_keys {
        loader.configured_feature(&key);
    }
    let placed_keys: Vec<Key> = loader.placed_json.keys().cloned().collect();
    for key in placed_keys {
        loader.placed_feature(&key);
    }

    let mut biome_features = FxHashMap::default();
    let mut biome_feature_ids = FxHashMap::default();
    for biome in (0..registries::BIOME.len() as Id).filter_map(|id| registries::BIOME.by_id(id)) {
        let id = usize::from(biome.id());
        let steps: Vec<Vec<&'static PlacedFeature>> = biome
            .generation_settings()
            .features()
            .iter()
            .map(|step| {
                step.iter()
                    .map(|key| {
                        loader.placed.get(key).copied().unwrap_or_else(|| {
                            panic!(
                                "biome {} references unknown placed feature {key}",
                                biome.key()
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

fn read_directory(pack: &Datapack, relative: &str) -> FxHashMap<Key, Value> {
    let mut entries = FxHashMap::default();
    for (namespace, directory) in pack.namespaced_dirs(relative) {
        for file in collect_json_files(&directory) {
            let key = parse_id(&path_to_id(&namespace, &file, &directory));
            let json: Value = read_and_parse_normalized(&file)
                .unwrap_or_else(|e| panic!("failed to load {}: {e}", file.display()));
            entries.insert(key, json);
        }
    }
    entries
}

impl Loader {
    fn placed_feature(&mut self, key: &Key) -> &'static PlacedFeature {
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

    fn configured_feature_ref(&mut self, value: &Value) -> &'static Feature {
        match value.as_str() {
            Some(raw) => {
                let key = parse_id(raw);
                self.configured_feature(&key)
            }
            None => self.build_configured(value),
        }
    }

    fn configured_feature(&mut self, key: &Key) -> &'static Feature {
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

    fn placed_feature_list(&mut self, value: &Value) -> Vec<&'static PlacedFeature> {
        match value.as_str() {
            Some(raw) if raw.starts_with('#') => {
                panic!("placed feature tags are not supported: {raw}")
            }
            Some(_) => vec![self.placed_feature_ref(value)],
            None => value_list(value)
                .iter()
                .map(|entry| self.placed_feature_ref(entry))
                .collect(),
        }
    }

    fn root_system_config(&mut self, json: &Value) -> RootSystemConfig {
        RootSystemConfig {
            feature: self.placed_feature_ref(&json["feature"]),
            required_vertical_space_for_tree: parse_json(&json["required_vertical_space_for_tree"]),
            level_test_distance: parse_json(&json["level_test_distance"]),
            max_level_deviation: parse_json(&json["max_level_deviation"]),
            root_radius: parse_json(&json["root_radius"]),
            root_replaceable: parse_json(&json["root_replaceable"]),
            root_state_provider: parse_json(&json["root_state_provider"]),
            root_placement_attempts: parse_json(&json["root_placement_attempts"]),
            root_column_max_height: parse_json(&json["root_column_max_height"]),
            hanging_root_radius: parse_json(&json["hanging_root_radius"]),
            hanging_roots_vertical_span: parse_json(&json["hanging_roots_vertical_span"]),
            hanging_root_state_provider: parse_json(&json["hanging_root_state_provider"]),
            hanging_root_placement_attempts: parse_json(&json["hanging_root_placement_attempts"]),
            allowed_vertical_water_for_tree: parse_json(&json["allowed_vertical_water_for_tree"]),
            allowed_tree_position: parse_json(&json["allowed_tree_position"]),
        }
    }

    fn vegetation_patch_config(&mut self, json: &Value) -> VegetationPatchConfig {
        VegetationPatchConfig {
            replaceable: parse_json(&json["replaceable"]),
            ground_state: parse_json(&json["ground_state"]),
            vegetation_feature: self.placed_feature_ref(&json["vegetation_feature"]),
            surface: parse_json(&json["surface"]),
            depth: parse_json(&json["depth"]),
            extra_bottom_block_chance: parse_json(&json["extra_bottom_block_chance"]),
            vertical_range: parse_json(&json["vertical_range"]),
            vegetation_chance: parse_json(&json["vegetation_chance"]),
            xz_radius: parse_json(&json["xz_radius"]),
            extra_edge_column_chance: parse_json(&json["extra_edge_column_chance"]),
        }
    }

    fn build_configured(&mut self, json: &Value) -> &'static Feature {
        let feature_type = parse_id(type_of(json));
        let feature = match feature_type.path() {
            "bamboo" => Feature::Bamboo(parse_json(json)),
            "block_blob" => Feature::BlockBlob(parse_json(json)),
            "block_column" => Feature::BlockColumn(parse_json(json)),
            "block_pile" => Feature::BlockPile(parse_json(json)),
            "blue_ice" => Feature::BlueIce,
            "bonus_chest" => Feature::BonusChest,
            "chorus_plant" => Feature::ChorusPlant,
            "coral_claw" => Feature::CoralClaw(self.placed_feature_ref(&json["feature"])),
            "coral_tree" => Feature::CoralTree(self.placed_feature_ref(&json["feature"])),
            "delta_feature" => Feature::DeltaFeature(parse_json(json)),
            "disk" => Feature::Disk(parse_json(json)),
            "end_gateway" => Feature::EndGateway(parse_json(json)),
            "end_island" => Feature::EndIsland,
            "end_platform" => Feature::EndPlatform,
            "end_podium" => Feature::EndPodium(parse_json(json)),
            "end_spike" => Feature::EndSpike(parse_json(json)),
            "fallen_tree" => Feature::FallenTree(parse_json(json)),
            "fill_layer" => Feature::FillLayer(parse_json(json)),
            "fossil" => Feature::Fossil(parse_json(json)),
            "freeze_top_layer" => Feature::FreezeTopLayer,
            "geode" => Feature::Geode(Box::new(parse_json(json))),
            "huge_brown_mushroom" => Feature::HugeBrownMushroom(parse_json(json)),
            "huge_fungus" => Feature::HugeFungus(parse_json(json)),
            "huge_red_mushroom" => Feature::HugeRedMushroom(parse_json(json)),
            "iceberg" => Feature::Iceberg(parse_json(json)),
            "lake" => Feature::Lake(parse_json(json)),
            "large_dripstone" => Feature::LargeDripstone(parse_json(json)),
            "monster_room" => Feature::MonsterRoom,
            "multiface_growth" => Feature::MultifaceGrowth(parse_json(json)),
            "netherrack_replace_blobs" => Feature::NetherrackReplaceBlobs(parse_json(json)),
            "no_op" => Feature::NoOp,
            "ore" => Feature::Ore(parse_json(json)),
            "overlay" => Feature::Overlay(self.placed_feature_list(&json["features"])),
            "projected_random_patchy_square" => {
                Feature::ProjectedRandomPatchySquare(parse_json(json))
            }
            "random_boolean_selector" => Feature::RandomBooleanSelector {
                feature_true: self.placed_feature_ref(&json["feature_true"]),
                feature_false: self.placed_feature_ref(&json["feature_false"]),
            },
            "random_neighbor_spread" => Feature::RandomNeighborSpread(parse_json(json)),
            "random_selector" => {
                let features = value_list(&json["features"])
                    .iter()
                    .map(|entry| WeightedPlacedFeature {
                        chance: entry["chance"].as_f64().unwrap_or_default() as f32,
                        feature: self.placed_feature_ref(&entry["feature"]),
                    })
                    .collect::<Vec<_>>();
                Feature::RandomSelector {
                    features,
                    default: self.placed_feature_ref(&json["default"]),
                }
            }
            "replace_single_block" => Feature::ReplaceSingleBlock(parse_json(json)),
            "root_system" => Feature::RootSystem(Box::new(self.root_system_config(json))),
            "scattered_ore" => Feature::ScatteredOre(parse_json(json)),
            "sculk_patch" => Feature::SculkPatch(parse_json(json)),
            "sequence" => Feature::Sequence(self.placed_feature_list(&json["features"])),
            "simple_block" => Feature::SimpleBlock(parse_json(json)),
            "simple_random_selector" => {
                Feature::SimpleRandomSelector(self.placed_feature_list(&json["features"]))
            }
            "single_block_pillar" => {
                Feature::SingleBlockPillar(single_block_pillar::SingleBlockPillarFeature {
                    block: parse_json(&json["block"]),
                    can_replace: json
                        .get("can_replace")
                        .map_or(BlockPredicate::AlwaysTrue, parse_json),
                    direction: parse_json(&json["direction"]),
                    chance_to_continue: json
                        .get("chance_to_continue")
                        .and_then(Value::as_f64)
                        .map_or(1.0, |chance| chance as f32),
                    cap_feature: json
                        .get("cap_feature")
                        .map(|feature| self.placed_feature_ref(feature)),
                })
            }
            "speleothem" => Feature::Speleothem(parse_json(json)),
            "speleothem_cluster" => Feature::SpeleothemCluster(parse_json(json)),
            "spike" => Feature::Spike(parse_json(json)),
            "spring_feature" => Feature::Spring(parse_json(json)),
            "stepped_column_cluster" => Feature::SteppedColumnCluster(parse_json(json)),
            "template" => Feature::Template(parse_json(json)),
            "tree" => Feature::Tree(Box::new(parse_json(json))),
            "underwater_magma" => Feature::UnderwaterMagma(parse_json(json)),
            "vegetation_patch" => Feature::VegetationPatch {
                config: Box::new(self.vegetation_patch_config(json)),
                waterlogged: false,
            },
            "vines" => Feature::Vines,
            "void_start_platform" => Feature::VoidStartPlatform,
            "waterlogged_vegetation_patch" => Feature::VegetationPatch {
                config: Box::new(self.vegetation_patch_config(json)),
                waterlogged: true,
            },
            "weighted_random_selector" => Feature::WeightedRandomSelector(
                value_list(&json["features"])
                    .iter()
                    .map(|entry| WeightedRandomFeature {
                        weight: entry["weight"].as_i64().unwrap_or(1) as i32,
                        feature: self.placed_feature_ref(&entry["data"]),
                    })
                    .collect(),
            ),
            other => panic!("unknown feature type {other}"),
        };

        Box::leak(Box::new(feature))
    }
}

#[cfg(test)]
mod tests {
    use crate::datapack;
    use crate::world::worldgen;
    use std::ptr;
    use std::sync::Once;

    use super::*;

    fn init_global_registry() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            worldgen::ensure_registry_initialized(&datapack::Datapack::builtin())
                .expect("vanilla datapack should load");
        });
    }

    #[test]
    fn loads_every_vanilla_placed_feature() {
        init_global_registry();
        let registry = features();

        let oak = registry
            .placed(&Key::const_vanilla("oak_checked"))
            .expect("oak_checked should load");
        assert!(ptr::eq(
            oak.feature,
            registry
                .configured(&Key::const_vanilla("oak"))
                .expect("oak should load")
        ));
        assert_eq!(oak.placement.len(), 1);

        let ore_granite = registry
            .placed(&Key::const_vanilla("ore_granite_upper"))
            .expect("ore_granite_upper should load");
        assert_eq!(ore_granite.placement.len(), 4);
    }

    #[test]
    fn every_vanilla_feature_loads() {
        init_global_registry();
        let registry = features();
        let pack = Datapack::builtin();

        assert_eq!(
            registry.configured.len(),
            read_directory(&pack, "worldgen/feature").len()
        );
        assert_eq!(
            registry.placed.len(),
            read_directory(&pack, "worldgen/placed_feature").len()
        );
    }

    #[test]
    fn sulfur_spring_resolves_to_template_features() {
        init_global_registry();
        let registry = features();

        let sulfur_spring = registry
            .configured(&Key::const_vanilla("sulfur_spring"))
            .expect("sulfur_spring should load");
        let Feature::WeightedRandomSelector(entries) = sulfur_spring else {
            panic!("sulfur_spring should be a weighted random selector");
        };

        let mut templates = 0;
        for entry in entries {
            let Feature::Sequence(steps) = entry.feature.feature else {
                continue;
            };
            for step in steps {
                if let Feature::Template(config) = step.feature {
                    assert!(!config.templates.is_empty());
                    templates += 1;
                }
            }
        }
        assert!(templates > 0, "sulfur_spring should place templates");
    }

    #[test]
    fn resolves_selector_sub_features() {
        init_global_registry();
        let registry = features();

        let trees_plains = registry
            .configured(&Key::const_vanilla("trees_plains"))
            .expect("trees_plains should load");
        let Feature::RandomSelector { features, default } = trees_plains else {
            panic!("trees_plains should be a random selector");
        };
        assert_eq!(features.len(), 2);
        assert!(ptr::eq(
            default.feature,
            registry
                .configured(&Key::const_vanilla("oak_bees_005"))
                .expect("oak_bees_005 should load")
        ));
        assert!(ptr::eq(
            features[1].feature,
            registry
                .placed(&Key::const_vanilla("fallen_oak_tree"))
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

        assert!(matches!(feature_of("ore_coal"), Feature::Ore(_)));
        assert!(matches!(
            feature_of("ore_ancient_debris_small"),
            Feature::ScatteredOre(_)
        ));
        assert!(matches!(
            feature_of("spring_lava_overworld"),
            Feature::Spring(_)
        ));
        assert!(matches!(feature_of("disk_clay"), Feature::Disk(_)));
        assert!(matches!(feature_of("cactus"), Feature::BlockColumn(_)));
        assert!(matches!(feature_of("pile_hay"), Feature::BlockPile(_)));
        assert!(matches!(feature_of("tall_grass"), Feature::SimpleBlock(_)));
        assert!(matches!(feature_of("vines"), Feature::Vines));

        let Feature::Ore(ore) = feature_of("ore_coal") else {
            panic!("ore_coal should be an ore");
        };
        assert_eq!(ore.size, 17);
        assert_eq!(ore.targets.len(), 2);
    }

    #[test]
    fn biome_feature_membership_matches_biome_definition() {
        init_global_registry();
        let registry = features();

        let plains = registries::BIOME
            .by_key(&Key::const_vanilla("plains"))
            .expect("plains biome");
        let trees_plains = registry
            .placed(&Key::const_vanilla("trees_plains"))
            .expect("trees_plains placed feature");
        let iceberg = registry
            .placed(&Key::const_vanilla("iceberg_packed"))
            .expect("iceberg_packed placed feature");

        assert!(registry.biome_contains(plains, trees_plains));
        assert!(!registry.biome_contains(plains, iceberg));
        assert_eq!(registry.features_for_biome(plains).len(), 11);
    }
}
