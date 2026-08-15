pub mod rule_test;
pub mod sorter;
pub mod block_state_provider;

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use rustc_hash::{FxHashMap, FxHashSet};
use serde_json::Value;

use crate::registry::REGISTRY;
use crate::registry::biome::BiomeRef;
use crate::util::datapack::{collect_json_files, path_to_id, read_and_parse_normalized};
use crate::util::ident::Identifier;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::worldgen::json::{parse_id, type_of, value_list};
use crate::worldgen::placement::{PlacementContext, PlacementModifier, Positions};
use crate::worldgen::region::WorldGenRegion;

#[derive(Debug)]
pub struct ConfiguredFeature {
    pub key: Identifier,
    pub kind: FeatureKind,
}

#[derive(Debug)]
pub enum FeatureKind {
    RandomSelector {
        features: Vec<WeightedPlacedFeature>,
        default: &'static PlacedFeature,
    },
    SimpleRandomSelector(Vec<&'static PlacedFeature>),
    RandomBooleanSelector {
        feature_true: &'static PlacedFeature,
        feature_false: &'static PlacedFeature,
    },
    Unported(Identifier),
}

#[derive(Debug)]
pub struct WeightedPlacedFeature {
    pub chance: f32,
    pub feature: &'static PlacedFeature,
}

#[derive(Debug)]
pub struct PlacedFeature {
    pub id: usize,
    pub key: Identifier,
    pub feature: &'static ConfiguredFeature,
    pub placement: Vec<PlacementModifier>,
}

impl ConfiguredFeature {
    pub fn place(
        &'static self,
        context: &PlacementContext<'_, '_>,
        random: &mut RandomSource,
        origin: BlockPos,
    ) -> bool {
        match &self.kind {
            FeatureKind::RandomSelector { features, default } => {
                for entry in features {
                    if random.next_f32() < entry.chance {
                        return entry.feature.place(context.region, random, origin);
                    }
                }
                default.place(context.region, random, origin)
            }
            FeatureKind::SimpleRandomSelector(features) => {
                if features.is_empty() {
                    return false;
                }
                let index = random.next_i32_bounded(features.len() as i32) as usize;
                features[index].place(context.region, random, origin)
            }
            FeatureKind::RandomBooleanSelector {
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
            FeatureKind::Unported(_) => false,
        }
    }
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
            if self.feature.place(context, random, origin) {
                *placed_any = true;
            }
            return;
        };

        let mut positions = Positions::new();
        modifier.positions(context, random, origin, &mut positions);
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

    pub fn biome_contains(&self, biome: BiomeRef, feature: &'static PlacedFeature) -> bool {
        self.biome_feature_ids
            .get(&biome.id())
            .is_some_and(|ids| ids.contains(&feature.id))
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
    next_inline_configured_id: usize,
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
        next_inline_configured_id: 0,
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
        let feature = self.build_placed(key.clone(), &json);

        self.resolving_placed.remove(key);
        self.placed.insert(key.clone(), feature);
        feature
    }

    fn build_placed(&mut self, key: Identifier, json: &Value) -> &'static PlacedFeature {
        let configured = self.configured_feature_ref(&json["feature"]);
        let placement = value_list(&json["placement"])
            .iter()
            .map(PlacementModifier::parse)
            .collect();
        let id = self.next_placed_id;
        self.next_placed_id += 1;

        Box::leak(Box::new(PlacedFeature {
            id,
            key,
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
            None => {
                let key = Identifier::new("worldgen_rs", format!("inline/{}", self.next_placed_id));
                self.build_placed(key, value)
            }
        }
    }

    fn configured_feature_ref(&mut self, value: &Value) -> &'static ConfiguredFeature {
        match value.as_str() {
            Some(raw) => {
                let key = parse_id(raw);
                self.configured_feature(&key)
            }
            None => {
                let key = Identifier::new(
                    "worldgen_rs",
                    format!("inline/{}", self.next_inline_configured_id),
                );
                self.next_inline_configured_id += 1;
                self.build_configured(key, value)
            }
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
        let feature = self.build_configured(key.clone(), &json);

        self.resolving_configured.remove(key);
        self.configured.insert(key.clone(), feature);
        feature
    }

    fn build_configured(&mut self, key: Identifier, json: &Value) -> &'static ConfiguredFeature {
        let feature_type = parse_id(type_of(json));
        let config = &json["config"];
        let kind = match feature_type.path.as_ref() {
            "random_selector" => {
                let features = value_list(&config["features"])
                    .iter()
                    .map(|entry| WeightedPlacedFeature {
                        chance: entry["chance"].as_f64().unwrap_or_default() as f32,
                        feature: self.placed_feature_ref(&entry["feature"]),
                    })
                    .collect::<Vec<_>>();
                FeatureKind::RandomSelector {
                    features,
                    default: self.placed_feature_ref(&config["default"]),
                }
            }
            "simple_random_selector" => FeatureKind::SimpleRandomSelector(
                value_list(&config["features"])
                    .iter()
                    .map(|entry| self.placed_feature_ref(entry))
                    .collect(),
            ),
            "random_boolean_selector" => FeatureKind::RandomBooleanSelector {
                feature_true: self.placed_feature_ref(&config["feature_true"]),
                feature_false: self.placed_feature_ref(&config["feature_false"]),
            },
            _ => FeatureKind::Unported(feature_type),
        };

        Box::leak(Box::new(ConfiguredFeature { key, kind }))
    }
}

#[cfg(test)]
mod tests {
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
        assert_eq!(oak.feature.key, Identifier::const_vanilla("oak"));
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
        let FeatureKind::RandomSelector { features, default } = &trees_plains.kind else {
            panic!("trees_plains should be a random selector");
        };
        assert_eq!(features.len(), 2);
        assert_eq!(
            default.feature.key,
            Identifier::const_vanilla("oak_bees_005")
        );
        assert_eq!(
            features[1].feature.key,
            Identifier::const_vanilla("fallen_oak_tree")
        );
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
