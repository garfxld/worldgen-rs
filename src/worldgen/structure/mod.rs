pub mod pieces;
pub mod scattered_feature_piece;
pub mod placement;
pub mod pools;
#[expect(clippy::module_inception, reason = "mirrors vanilla's Structure class")]
pub mod structure;
pub mod structure_piece;
pub mod template_structure_piece;
pub mod structure_set;
pub mod structure_start;
pub mod structures;
pub mod templatesystem;

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use glam::IVec3;
use rustc_hash::FxHashMap;
use serde_json::Value;

use crate::util::datapack::{collect_json_files, parse_id, parse_json, path_to_id, read};
use crate::util::ident::Identifier;
use crate::worldgen::feature::features;
use crate::worldgen::structure::placement::structure_placement::{
    ExclusionZone, FrequencyReductionMethod, RandomSpreadType, StructurePlacement,
    StructurePlacementKind,
};
use crate::worldgen::structure::pools::TemplatePoolRegistry;
use crate::worldgen::structure::pools::alias::pool_alias_binding::{
    PoolAliasBinding, WeightedTarget,
};
use crate::worldgen::structure::pools::dimension_padding::DimensionPadding;
use crate::worldgen::structure::pools::structure_pool_element::{
    FeaturePoolElement, SinglePoolElement, StructurePoolElement,
};
use crate::worldgen::structure::pools::structure_template_pool::{
    Projection, StructureTemplatePool,
};
use crate::worldgen::structure::structure::{
    BiomeHolderSet, GenerationStep, Structure, StructureSettings, TerrainAdjustment,
};
use crate::worldgen::structure::structure_set::{StructureSelectionEntry, StructureSet};
use crate::worldgen::structure::structures::ocean_ruin_pieces;
use crate::worldgen::structure::structures::mineshaft_pieces::MineshaftType;
use crate::worldgen::structure::structures::ruined_portal;
use crate::worldgen::structure::structures::ruined_portal_piece::VerticalPlacement;
use crate::worldgen::structure::structures::jigsaw::{
    DEFAULT_DIMENSION_PADDING, DEFAULT_LIQUID_SETTINGS, JigsawStructure, MaxDistance,
};
use crate::worldgen::structure::templatesystem::liquid_settings::LiquidSettings;
use crate::worldgen::structure::templatesystem::structure_processor::StructureProcessor;
use crate::worldgen::structure::templatesystem::structure_template_manager::processor_list;

pub struct StructureRegistry {
    structures: FxHashMap<Identifier, &'static Structure>,
    sets: Vec<&'static StructureSet>,
    template_pools: TemplatePoolRegistry,
}

impl StructureRegistry {
    pub fn structure(&self, key: &Identifier) -> Option<&'static Structure> {
        self.structures.get(key).copied()
    }

    pub fn structures(&self) -> impl Iterator<Item = (&Identifier, &'static Structure)> {
        self.structures.iter().map(|(key, value)| (key, *value))
    }

    pub fn sets(&self) -> &[&'static StructureSet] {
        &self.sets
    }

    pub const fn template_pools(&self) -> &TemplatePoolRegistry {
        &self.template_pools
    }
}

static REGISTRY: LazyLock<StructureRegistry> = LazyLock::new(load_registry);

pub fn registry() -> &'static StructureRegistry {
    &REGISTRY
}

fn worldgen_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/datapack/worldgen")
}

fn read_json(path: &Path) -> Value {
    let content = read(path).unwrap_or_else(|error| panic!("failed to read: {error}"));
    serde_json::from_str(&content)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

fn load_registry() -> StructureRegistry {
    let root = worldgen_dir();
    let template_pools = load_template_pools(&root.join("template_pool"));
    let structures = load_structures(&root.join("structure"));
    let sets = load_structure_sets(&root.join("structure_set"), &structures);

    StructureRegistry {
        structures,
        sets,
        template_pools: TemplatePoolRegistry::new(template_pools),
    }
}

fn load_template_pools(dir: &Path) -> FxHashMap<Identifier, &'static StructureTemplatePool> {
    let mut pools = FxHashMap::default();
    for file in collect_json_files(dir) {
        let key: Identifier = path_to_id("minecraft", &file, dir)
            .parse()
            .expect("template pool path should be a valid identifier");
        let json = read_json(&file);
        let fallback = parse_id(json["fallback"].as_str().unwrap_or("minecraft:empty"));

        let elements = json["elements"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default()
            .iter()
            .map(|entry| {
                let element: &'static StructurePoolElement =
                    Box::leak(Box::new(parse_pool_element(&entry["element"])));
                let weight = entry["weight"].as_i64().unwrap_or(1) as i32;
                (element, weight)
            })
            .collect();

        let pool: &'static StructureTemplatePool =
            Box::leak(Box::new(StructureTemplatePool::new(fallback, elements)));
        pools.insert(key, pool);
    }
    pools
}

fn parse_pool_element(json: &Value) -> StructurePoolElement {
    let element_type = parse_id(json["element_type"].as_str().unwrap_or_default());
    let projection = parse_projection(&json["projection"]);
    match element_type.path.as_ref() {
        "single_pool_element" | "legacy_single_pool_element" => {
            let single = SinglePoolElement {
                template: parse_id(json["location"].as_str().unwrap_or_default()),
                processors: parse_processors(&json["processors"]),
                projection,
                override_liquid_settings: json["override_liquid_settings"]
                    .as_str()
                    .map(parse_liquid_settings),
            };
            if element_type.path.as_ref() == "single_pool_element" {
                StructurePoolElement::Single(single)
            } else {
                StructurePoolElement::LegacySingle(single)
            }
        }
        "feature_pool_element" => {
            let key = parse_id(json["feature"].as_str().unwrap_or_default());
            let feature = features()
                .placed(&key)
                .unwrap_or_else(|| panic!("unknown placed feature {key} in feature pool element"));
            StructurePoolElement::Feature(FeaturePoolElement {
                feature,
                projection,
            })
        }
        "list_pool_element" => StructurePoolElement::List {
            elements: json["elements"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_default()
                .iter()
                .map(parse_pool_element)
                .collect(),
            projection,
        },
        "empty_pool_element" => StructurePoolElement::Empty,
        other => panic!("unknown structure pool element type {other}"),
    }
}

fn parse_projection(json: &Value) -> Projection {
    match json.as_str() {
        Some("terrain_matching") => Projection::TerrainMatching,
        _ => Projection::Rigid,
    }
}

const fn parse_liquid_settings(value: &str) -> LiquidSettings {
    match value {
        "ignore_waterlogging" => LiquidSettings::IgnoreWaterlogging,
        _ => LiquidSettings::ApplyWaterlogging,
    }
}

fn parse_processors(json: &Value) -> Vec<StructureProcessor> {
    match json {
        Value::String(key) => {
            let key = parse_id(key);
            processor_list(&key)
                .map(|list| list.processors.clone())
                .unwrap_or_else(|| panic!("unknown processor list {key}"))
        }
        Value::Object(_) => json["processors"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default()
            .iter()
            .map(parse_json)
            .collect(),
        _ => Vec::new(),
    }
}

fn load_structures(dir: &Path) -> FxHashMap<Identifier, &'static Structure> {
    let mut structures = FxHashMap::default();
    for file in collect_json_files(dir) {
        let key: Identifier = path_to_id("minecraft", &file, dir)
            .parse()
            .expect("structure path should be a valid identifier");
        let json = read_json(&file);
        let structure: &'static Structure = Box::leak(Box::new(parse_structure(&json)));
        structures.insert(key, structure);
    }
    structures
}

fn parse_structure(json: &Value) -> Structure {
    let structure_type = parse_id(json["type"].as_str().unwrap_or_default());

    let settings = StructureSettings {
        biomes: parse_biomes(&json["biomes"]),
        step: parse_json(&json["step"]),
        terrain_adaptation: json
            .get("terrain_adaptation")
            .map_or(TerrainAdjustment::None, parse_json),
    };

    match structure_type.path.as_ref() {
        "jigsaw" => {}
        "swamp_hut" => return Structure::SwampHut(settings),
        "desert_pyramid" => return Structure::DesertPyramid(settings),
        "igloo" => return Structure::Igloo(settings),
        "jungle_temple" => return Structure::JungleTemple(settings),
        "buried_treasure" => return Structure::BuriedTreasure(settings),
        "end_city" => return Structure::EndCity(settings),
        "woodland_mansion" => return Structure::WoodlandMansion(settings),
        "fortress" => return Structure::Fortress(settings),
        "stronghold" => return Structure::Stronghold(settings),
        "ocean_monument" => return Structure::OceanMonument(settings),
        "nether_fossil" => {
            return Structure::NetherFossil {
                settings,
                height: parse_json(&json["height"]),
            };
        }
        "mineshaft" => {
            return Structure::Mineshaft {
                settings,
                mineshaft_type: MineshaftType::parse(
                    json["mineshaft_type"].as_str().unwrap_or_default(),
                ),
            };
        }
        "ruined_portal" => {
            return Structure::RuinedPortal {
                settings,
                setups: json["setups"]
                    .as_array()
                    .map(Vec::as_slice)
                    .unwrap_or_default()
                    .iter()
                    .map(parse_ruined_portal_setup)
                    .collect(),
            };
        }
        "ocean_ruin" => {
            return Structure::OceanRuin {
                settings,
                biome_temp: match json["biome_temp"].as_str() {
                    Some("cold") => ocean_ruin_pieces::Temperature::Cold,
                    _ => ocean_ruin_pieces::Temperature::Warm,
                },
                large_probability: json["large_probability"].as_f64().unwrap_or_default() as f32,
                cluster_probability: json["cluster_probability"].as_f64().unwrap_or_default()
                    as f32,
            };
        }
        "shipwreck" => {
            return Structure::Shipwreck {
                settings,
                is_beached: json["is_beached"].as_bool().unwrap_or_default(),
            };
        }
        _ => return Structure::Unported(structure_type),
    }

    Structure::Jigsaw(JigsawStructure {
        settings,
        start_pool: parse_id(json["start_pool"].as_str().unwrap_or_default()),
        start_jigsaw_name: json["start_jigsaw_name"].as_str().map(parse_id),
        max_depth: json["size"].as_i64().unwrap_or_default() as i32,
        start_height: parse_json(&json["start_height"]),
        use_expansion_hack: json["use_expansion_hack"].as_bool().unwrap_or_default(),
        project_start_to_heightmap: json
            .get("project_start_to_heightmap")
            .map(parse_json),
        max_distance_from_center: parse_max_distance(&json["max_distance_from_center"]),
        pool_aliases: json
            .get("pool_aliases")
            .and_then(Value::as_array)
            .map(|entries| entries.iter().map(parse_pool_alias).collect())
            .unwrap_or_default(),
        dimension_padding: json
            .get("dimension_padding")
            .map_or(DEFAULT_DIMENSION_PADDING, parse_dimension_padding),
        liquid_settings: json["liquid_settings"]
            .as_str()
            .map_or(DEFAULT_LIQUID_SETTINGS, parse_liquid_settings),
    })
}

fn parse_ruined_portal_setup(json: &Value) -> ruined_portal::Setup {
    ruined_portal::Setup {
        placement: VerticalPlacement::parse(json["placement"].as_str().unwrap_or_default()),
        air_pocket_probability: json["air_pocket_probability"].as_f64().unwrap_or_default() as f32,
        mossiness: json["mossiness"].as_f64().unwrap_or_default() as f32,
        overgrown: json["overgrown"].as_bool().unwrap_or_default(),
        vines: json["vines"].as_bool().unwrap_or_default(),
        can_be_cold: json["can_be_cold"].as_bool().unwrap_or_default(),
        replace_with_blackstone: json["replace_with_blackstone"].as_bool().unwrap_or_default(),
        weight: json["weight"].as_f64().unwrap_or(1.0) as f32,
    }
}

fn parse_biomes(json: &Value) -> BiomeHolderSet {
    match json {
        Value::String(entry) => match entry.strip_prefix('#') {
            Some(tag) => BiomeHolderSet::Tag(parse_id(tag)),
            None => BiomeHolderSet::Entries(vec![parse_id(entry)]),
        },
        Value::Array(entries) => BiomeHolderSet::Entries(
            entries
                .iter()
                .filter_map(|entry| entry.as_str().map(parse_id))
                .collect(),
        ),
        _ => BiomeHolderSet::Entries(Vec::new()),
    }
}

fn parse_max_distance(json: &Value) -> MaxDistance {
    match json {
        Value::Number(value) => MaxDistance::of(value.as_i64().unwrap_or(80) as i32),
        Value::Object(_) => MaxDistance {
            horizontal: json["horizontal"].as_i64().unwrap_or(80) as i32,
            vertical: json["vertical"].as_i64().unwrap_or(384) as i32,
        },
        _ => MaxDistance::of(80),
    }
}

fn parse_dimension_padding(json: &Value) -> DimensionPadding {
    match json {
        Value::Number(value) => DimensionPadding::of(value.as_i64().unwrap_or_default() as i32),
        Value::Object(_) => DimensionPadding::new(
            json["bottom"].as_i64().unwrap_or_default() as i32,
            json["top"].as_i64().unwrap_or_default() as i32,
        ),
        _ => DimensionPadding::ZERO,
    }
}

fn parse_pool_alias(json: &Value) -> PoolAliasBinding {
    let alias_type = parse_id(json["type"].as_str().unwrap_or_default());
    match alias_type.path.as_ref() {
        "direct" => PoolAliasBinding::Direct {
            alias: parse_id(json["alias"].as_str().unwrap_or_default()),
            target: parse_id(json["target"].as_str().unwrap_or_default()),
        },
        "random" => PoolAliasBinding::Random {
            alias: parse_id(json["alias"].as_str().unwrap_or_default()),
            targets: json["targets"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_default()
                .iter()
                .map(|entry| WeightedTarget {
                    value: parse_id(entry["data"].as_str().unwrap_or_default()),
                    weight: entry["weight"].as_i64().unwrap_or(1) as i32,
                })
                .collect(),
        },
        "random_group" => PoolAliasBinding::RandomGroup {
            groups: json["groups"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_default()
                .iter()
                .map(|entry| WeightedTarget {
                    value: entry["data"]
                        .as_array()
                        .map(Vec::as_slice)
                        .unwrap_or_default()
                        .iter()
                        .map(parse_pool_alias)
                        .collect(),
                    weight: entry["weight"].as_i64().unwrap_or(1) as i32,
                })
                .collect(),
        },
        other => panic!("unknown pool alias binding type {other}"),
    }
}

fn load_structure_sets(
    dir: &Path,
    structures: &FxHashMap<Identifier, &'static Structure>,
) -> Vec<&'static StructureSet> {
    let mut sets = Vec::new();
    for file in collect_json_files(dir) {
        let key: Identifier = path_to_id("minecraft", &file, dir)
            .parse()
            .expect("structure set path should be a valid identifier");
        let json = read_json(&file);

        let entries = json["structures"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default()
            .iter()
            .filter_map(|entry| {
                let structure_key = parse_id(entry["structure"].as_str().unwrap_or_default());
                let structure = structures.get(&structure_key)?;
                Some(StructureSelectionEntry {
                    key: structure_key,
                    structure,
                    weight: entry["weight"].as_i64().unwrap_or(1) as i32,
                })
            })
            .collect();

        let set: &'static StructureSet = Box::leak(Box::new(StructureSet {
            key,
            structures: entries,
            placement: parse_placement(&json["placement"]),
        }));
        sets.push(set);
    }
    sets.sort_by_key(|set| set.key.to_string());
    sets
}

fn parse_placement(json: &Value) -> StructurePlacement {
    let placement_type = parse_id(json["type"].as_str().unwrap_or_default());
    let kind = match placement_type.path.as_ref() {
        "random_spread" => StructurePlacementKind::RandomSpread {
            spacing: json["spacing"].as_i64().unwrap_or(1) as i32,
            separation: json["separation"].as_i64().unwrap_or_default() as i32,
            spread_type: match json["spread_type"].as_str() {
                Some("triangular") => RandomSpreadType::Triangular,
                _ => RandomSpreadType::Linear,
            },
        },
        "concentric_rings" => StructurePlacementKind::ConcentricRings {
            distance: json["distance"].as_i64().unwrap_or(1) as i32,
            spread: json["spread"].as_i64().unwrap_or(1) as i32,
            count: json["count"].as_i64().unwrap_or_default() as i32,
            preferred_biomes: parse_biomes(&json["preferred_biomes"]),
        },
        other => panic!("unknown structure placement type {other}"),
    };

    StructurePlacement {
        locate_offset: json["locate_offset"]
            .as_array()
            .map(|values| {
                IVec3::new(
                    values.first().and_then(Value::as_i64).unwrap_or_default() as i32,
                    values.get(1).and_then(Value::as_i64).unwrap_or_default() as i32,
                    values.get(2).and_then(Value::as_i64).unwrap_or_default() as i32,
                )
            })
            .unwrap_or(IVec3::ZERO),
        frequency_reduction_method: match json["frequency_reduction_method"].as_str() {
            Some("legacy_type_1") => FrequencyReductionMethod::LegacyType1,
            Some("legacy_type_2") => FrequencyReductionMethod::LegacyType2,
            Some("legacy_type_3") => FrequencyReductionMethod::LegacyType3,
            _ => FrequencyReductionMethod::Default,
        },
        frequency: json["frequency"].as_f64().unwrap_or(1.0) as f32,
        salt: json["salt"].as_i64().unwrap_or_default() as i32,
        exclusion_zone: json.get("exclusion_zone").map(|zone| ExclusionZone {
            other_set: parse_id(zone["other_set"].as_str().unwrap_or_default()),
            chunk_count: zone["chunk_count"].as_i64().unwrap_or(1) as i32,
        }),
        kind,
    }
}

const fn _generation_step_is_used(_: GenerationStep) {}
