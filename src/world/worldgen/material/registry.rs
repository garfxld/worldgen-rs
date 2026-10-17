use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, OnceLock};

use rustc_hash::FxHashSet;
use serde::de::Error as DeError;
use serde_json::Value;
use shulkr::registry::registries;
use shulkr::util::Key;
use shulkr::world::block::BlockState;

use crate::datapack::{Datapack, DatapackError, collect_json_files, path_to_id, read};
use crate::util::value_providers::VerticalAnchor;
use crate::world::worldgen::densityfunction::parse::normalize_id;
use crate::world::worldgen::densityfunction::registry::DensityRegistry;

use super::condition::MaterialCondition;
use super::rule::{MaterialRule, OreVeinRule};

#[derive(Default)]
pub struct MaterialRegistry {
    pub rules: BTreeMap<String, Arc<OnceLock<MaterialRule>>>,
    pub conditions: BTreeMap<String, Arc<OnceLock<MaterialCondition>>>,
}

fn field<'a>(v: &'a Value, name: &str) -> Result<&'a Value, String> {
    v.get(name).ok_or_else(|| format!("missing field {name}"))
}

fn int(v: &Value, name: &str) -> Result<i32, String> {
    field(v, name)?
        .as_i64()
        .map(|n| n as i32)
        .ok_or_else(|| format!("{name} is not an integer"))
}

fn int_range(v: &Value, name: &str, min: i32, max: i32) -> Result<i32, String> {
    let value = int(v, name)?;
    if (min..=max).contains(&value) {
        Ok(value)
    } else {
        Err(format!("{name} {value} outside of range [{min}:{max}]"))
    }
}

fn boolean(v: &Value, name: &str) -> Result<bool, String> {
    field(v, name)?
        .as_bool()
        .ok_or_else(|| format!("{name} is not a boolean"))
}

fn double(v: &Value, name: &str) -> Result<f64, String> {
    field(v, name)?
        .as_f64()
        .ok_or_else(|| format!("{name} is not a number"))
}

fn string<'a>(v: &'a Value, name: &str) -> Result<&'a str, String> {
    field(v, name)?
        .as_str()
        .ok_or_else(|| format!("{name} is not a string"))
}

fn block_state(v: &Value, name: &str) -> Result<BlockState, String> {
    serde_json::from_value(field(v, name)?.clone()).map_err(|e| format!("{name}: {e}"))
}

fn anchor(v: &Value, name: &str) -> Result<VerticalAnchor, String> {
    serde_json::from_value(field(v, name)?.clone()).map_err(|e| format!("{name}: {e}"))
}

fn type_of(v: &Value) -> Result<String, String> {
    Ok(normalize_id(string(v, "type")?))
}

fn biome_ids(v: &Value) -> Result<FxHashSet<u16>, String> {
    let biomes = &registries::BIOME;
    let lookup = |id: &str| -> Result<Vec<u16>, String> {
        if let Some(tag) = id.strip_prefix('#') {
            Ok(biomes
                .iter_tag(&Key::of(normalize_id(tag)))
                .filter_map(|(key, _)| biomes.get_id(key))
                .collect())
        } else {
            let key = Key::of(normalize_id(id));
            biomes
                .by_key(&key)
                .map(|biome| vec![biome.id()])
                .ok_or_else(|| format!("unknown biome {key}"))
        }
    };
    let mut ids = FxHashSet::default();
    match v {
        Value::String(id) => ids.extend(lookup(id)?),
        Value::Array(entries) => {
            for entry in entries {
                let id = entry.as_str().ok_or("biome entry is not a string")?;
                if id.starts_with('#') {
                    return Err(format!("tag {id} not allowed in a biome list"));
                }
                ids.extend(lookup(id)?);
            }
        }
        _ => return Err("biome_is is not a string or list".to_owned()),
    }
    Ok(ids)
}

fn parse_error(path: &Path, message: String) -> DatapackError {
    DatapackError::Parse {
        path: path.to_path_buf(),
        source: DeError::custom(message),
    }
}

impl MaterialRegistry {
    pub fn load(pack: &Datapack, densities: &DensityRegistry) -> Result<Self, DatapackError> {
        let mut registry = Self::default();
        let mut rule_sources = Vec::new();
        for (namespace, dir) in pack.namespaced_dirs("worldgen/material_rule") {
            for file in collect_json_files(&dir) {
                let id = path_to_id(&namespace, &file, &dir);
                registry
                    .rules
                    .entry(id.clone())
                    .or_insert_with(|| Arc::new(OnceLock::new()));
                rule_sources.push((id, file));
            }
        }
        let mut condition_sources = Vec::new();
        for (namespace, dir) in pack.namespaced_dirs("worldgen/material_condition") {
            for file in collect_json_files(&dir) {
                let id = path_to_id(&namespace, &file, &dir);
                registry
                    .conditions
                    .entry(id.clone())
                    .or_insert_with(|| Arc::new(OnceLock::new()));
                condition_sources.push((id, file));
            }
        }

        let read_value = |file: &Path| -> Result<Value, DatapackError> {
            serde_json::from_str(&read(file)?).map_err(|source| DatapackError::Parse {
                path: file.to_path_buf(),
                source,
            })
        };
        let mut parsed_conditions = BTreeMap::new();
        for (id, file) in condition_sources {
            let condition = registry
                .parse_condition(&read_value(&file)?)
                .map_err(|e| parse_error(&file, e))?;
            parsed_conditions.insert(id, condition);
        }
        let mut parsed_rules = BTreeMap::new();
        for (id, file) in rule_sources {
            let rule = registry
                .parse_rule(densities, &read_value(&file)?)
                .map_err(|e| parse_error(&file, e))?;
            parsed_rules.insert(id, rule);
        }
        for (id, condition) in parsed_conditions {
            let _ = registry.conditions[&id].set(condition);
        }
        for (id, rule) in parsed_rules {
            let _ = registry.rules[&id].set(rule);
        }
        Ok(registry)
    }

    pub fn rule(&self, key: &str) -> Result<MaterialRule, String> {
        let key = normalize_id(key);
        self.rules
            .get(&key)
            .map(|holder| MaterialRule::Holder(holder.clone()))
            .ok_or_else(|| format!("unknown material rule {key}"))
    }

    fn condition(&self, key: &str) -> Result<MaterialCondition, String> {
        let key = normalize_id(key);
        self.conditions
            .get(&key)
            .map(|holder| MaterialCondition::Holder(holder.clone()))
            .ok_or_else(|| format!("unknown material condition {key}"))
    }

    pub fn parse_rule(
        &self,
        densities: &DensityRegistry,
        v: &Value,
    ) -> Result<MaterialRule, String> {
        if let Some(key) = v.as_str() {
            return self.rule(key);
        }
        Ok(match type_of(v)?.as_str() {
            "minecraft:block" => MaterialRule::Block(block_state(v, "result_state")?),
            "minecraft:bandlands" => MaterialRule::Bandlands,
            "minecraft:sequence" => MaterialRule::Sequence(
                field(v, "sequence")?
                    .as_array()
                    .ok_or("sequence is not a list")?
                    .iter()
                    .map(|rule| self.parse_rule(densities, rule))
                    .collect::<Result<_, _>>()?,
            ),
            "minecraft:condition" => MaterialRule::Condition {
                if_true: self.parse_condition(field(v, "if_true")?)?,
                then_run: Box::new(self.parse_rule(densities, field(v, "then_run")?)?),
            },
            "minecraft:ore_vein" => {
                let raw_ore_chance = double(v, "raw_ore_chance")? as f32;
                if !(0.0..=1.0).contains(&raw_ore_chance) {
                    return Err(format!(
                        "raw_ore_chance {raw_ore_chance} outside of range [0:1]"
                    ));
                }
                MaterialRule::OreVein(Box::new(OreVeinRule {
                    ore_block: block_state(v, "ore_block")?,
                    raw_ore_block: block_state(v, "raw_ore_block")?,
                    filler_block: block_state(v, "filler_block")?,
                    raw_ore_chance,
                    density: densities.parse(field(v, "density")?)?,
                    richness: densities.parse(field(v, "richness")?)?,
                    filler_gap: densities.parse(field(v, "filler_gap")?)?,
                }))
            }
            other => return Err(format!("unknown material rule type {other}")),
        })
    }

    pub fn parse_condition(&self, v: &Value) -> Result<MaterialCondition, String> {
        if let Some(key) = v.as_str() {
            return self.condition(key);
        }
        Ok(match type_of(v)?.as_str() {
            "minecraft:biome" => {
                MaterialCondition::Biome(Arc::new(biome_ids(field(v, "biome_is")?)?))
            }
            "minecraft:noise_threshold" => MaterialCondition::NoiseThreshold {
                noise: normalize_id(string(v, "noise")?),
                min_threshold: double(v, "min_threshold")?,
                max_threshold: double(v, "max_threshold")?,
                is_3d: match v.get("is_3d") {
                    Some(value) => value.as_bool().ok_or("is_3d is not a boolean")?,
                    None => false,
                },
            },
            "minecraft:vertical_gradient" => MaterialCondition::VerticalGradient {
                random_name: normalize_id(string(v, "random_name")?),
                true_at_and_below: anchor(v, "true_at_and_below")?,
                false_at_and_above: anchor(v, "false_at_and_above")?,
            },
            "minecraft:y_above" => MaterialCondition::YAbove {
                anchor: anchor(v, "anchor")?,
                surface_depth_multiplier: int_range(v, "surface_depth_multiplier", -20, 20)?,
                add_stone_depth: boolean(v, "add_stone_depth")?,
            },
            "minecraft:water" => MaterialCondition::Water {
                offset: int(v, "offset")?,
                surface_depth_multiplier: int_range(v, "surface_depth_multiplier", -20, 20)?,
                add_stone_depth: boolean(v, "add_stone_depth")?,
            },
            "minecraft:temperature" => MaterialCondition::Temperature,
            "minecraft:steep" => MaterialCondition::Steep,
            "minecraft:not" => {
                MaterialCondition::Not(Box::new(self.parse_condition(field(v, "invert")?)?))
            }
            "minecraft:hole" => MaterialCondition::Hole,
            "minecraft:above_preliminary_surface" => MaterialCondition::AbovePreliminarySurface,
            "minecraft:stone_depth" => MaterialCondition::StoneDepth {
                offset: int(v, "offset")?,
                add_surface_depth: boolean(v, "add_surface_depth")?,
                secondary_depth_range: int(v, "secondary_depth_range")?,
                ceiling: match string(v, "surface_type")? {
                    "ceiling" => true,
                    "floor" => false,
                    other => return Err(format!("unknown surface_type {other}")),
                },
            },
            other => return Err(format!("unknown material condition type {other}")),
        })
    }
}
