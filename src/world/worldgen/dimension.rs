use std::io::{Error as IoError, ErrorKind};
use std::path::Path;

use serde::Deserialize;
use serde::de::Error as DeError;
use serde_json::Value;
use shulkr::util::Key;

use crate::datapack::{
    Datapack, DatapackError, collect_json_files, path_to_id, read, read_and_parse_normalized,
};
use crate::registry::biome::BiomeRef;
use crate::world::worldgen::biome::climate::{ParameterList, ParameterPoint, quantize_coord};
use crate::world::worldgen::biome::multi_noise::{parameter_list, preset_parameter_list};
use crate::world::worldgen::noise_generator_settings::parameter;

pub struct DimensionSpec {
    pub dimension_type: Key,
    pub noise_settings: Key,
    pub biome_source: BiomeSourceSpec,
}

pub enum BiomeSourceSpec {
    MultiNoise(ParameterList<BiomeRef>),
    TheEnd,
}

#[derive(Deserialize)]
struct DimensionTypeJson {
    min_y: i32,
    height: i32,
}

fn not_found(relative: String, message: String) -> DatapackError {
    DatapackError::Read {
        path: relative.into(),
        source: IoError::new(ErrorKind::NotFound, message),
    }
}

fn parse_error(path: &Path, message: String) -> DatapackError {
    DatapackError::Parse {
        path: path.to_path_buf(),
        source: DeError::custom(message),
    }
}

pub fn dimension_type_height(pack: &Datapack, id: &Key) -> Result<(i32, i32), DatapackError> {
    let relative = format!("dimension_type/{}.json", id.path());
    let path = pack
        .files(id.namespace(), &relative)
        .pop()
        .ok_or_else(|| not_found(relative, format!("{id} not present in the datapack")))?;
    let json: DimensionTypeJson = read_and_parse_normalized(&path)?;
    Ok((json.min_y, json.height))
}

fn field<'a>(v: &'a Value, name: &str) -> Result<&'a Value, String> {
    v.get(name).ok_or_else(|| format!("missing field {name}"))
}

fn string<'a>(v: &'a Value, name: &str) -> Result<&'a str, String> {
    field(v, name)?
        .as_str()
        .ok_or_else(|| format!("{name} is not a string"))
}

fn multi_noise_preset(pack: &Datapack, id: &Key) -> Result<ParameterList<BiomeRef>, String> {
    let relative = format!(
        "worldgen/multi_noise_biome_source_parameter_list/{}.json",
        id.path()
    );
    let path = pack
        .files(id.namespace(), &relative)
        .pop()
        .ok_or_else(|| format!("{id} not present in the datapack"))?;
    let value: Value = serde_json::from_str(&read(&path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let preset = Key::of(string(&value, "preset")?.to_owned());
    preset_parameter_list(&preset).ok_or_else(|| format!("Unknown preset: {preset}"))?
}

fn parameter_point(v: &Value) -> Result<ParameterPoint, String> {
    let offset = field(v, "offset")?
        .as_f64()
        .ok_or("offset is not a number")? as f32;
    if !(0.0..=1.0).contains(&offset) {
        return Err(format!("offset {offset} outside of range [0:1]"));
    }
    Ok(ParameterPoint::new(
        parameter(field(v, "temperature")?)?,
        parameter(field(v, "humidity")?)?,
        parameter(field(v, "continentalness")?)?,
        parameter(field(v, "erosion")?)?,
        parameter(field(v, "depth")?)?,
        parameter(field(v, "weirdness")?)?,
        quantize_coord(f64::from(offset)),
    ))
}

fn biome_source(pack: &Datapack, v: &Value) -> Result<BiomeSourceSpec, String> {
    let kind = Key::of(string(v, "type")?.to_owned());
    match (kind.namespace(), kind.path()) {
        ("minecraft", "the_end") => Ok(BiomeSourceSpec::TheEnd),
        ("minecraft", "multi_noise") => {
            if let Some(biomes) = v.get("biomes") {
                let entries = biomes
                    .as_array()
                    .ok_or("biomes is not a list")?
                    .iter()
                    .map(|entry| {
                        Ok((
                            parameter_point(field(entry, "parameters")?)?,
                            string(entry, "biome")?.to_owned(),
                        ))
                    })
                    .collect::<Result<_, String>>()?;
                Ok(BiomeSourceSpec::MultiNoise(parameter_list(entries)?))
            } else {
                let preset = Key::of(string(v, "preset")?.to_owned());
                Ok(BiomeSourceSpec::MultiNoise(multi_noise_preset(
                    pack, &preset,
                )?))
            }
        }
        _ => Err(format!("unsupported biome source type {kind}")),
    }
}

pub fn resolve(pack: &Datapack, dimension: &Key) -> Result<DimensionSpec, DatapackError> {
    let relative = format!("dimension/{}.json", dimension.path());
    let Some(path) = pack.files(dimension.namespace(), &relative).pop() else {
        return builtin(pack, dimension);
    };
    let value: Value =
        serde_json::from_str(&read(&path)?).map_err(|source| DatapackError::Parse {
            path: path.clone(),
            source,
        })?;
    let parse = || -> Result<DimensionSpec, String> {
        let generator = field(&value, "generator")?;
        Ok(DimensionSpec {
            dimension_type: Key::of(string(&value, "type")?.to_owned()),
            noise_settings: Key::of(string(generator, "settings")?.to_owned()),
            biome_source: biome_source(pack, field(generator, "biome_source")?)?,
        })
    };
    parse().map_err(|message| parse_error(&path, message))
}

pub fn available(pack: &Datapack) -> Vec<String> {
    let mut ids: Vec<String> = ["overworld", "the_nether", "the_end"]
        .iter()
        .map(|path| format!("minecraft:{path}"))
        .collect();

    for (namespace, dir) in pack.namespaced_dirs("dimension") {
        for file in collect_json_files(&dir) {
            ids.push(path_to_id(&namespace, &file, &dir));
        }
    }

    ids.sort();
    ids.dedup();
    ids
}

fn builtin(pack: &Datapack, dimension: &Key) -> Result<DimensionSpec, DatapackError> {
    let (settings, preset) = match (dimension.namespace(), dimension.path()) {
        ("minecraft", "overworld") => ("overworld", Some("overworld")),
        ("minecraft", "the_nether") => ("nether", Some("nether")),
        ("minecraft", "the_end") => ("end", None),
        _ => {
            return Err(not_found(
                format!("dimension/{}.json", dimension.path()),
                format!(
                    "no dimension {dimension} in the datapack; available: {}",
                    available(pack).join(", ")
                ),
            ));
        }
    };
    let biome_source = match preset {
        Some(preset) => BiomeSourceSpec::MultiNoise(
            multi_noise_preset(pack, &Key::const_vanilla(preset)).map_err(|message| {
                not_found(format!("dimension/{}.json", dimension.path()), message)
            })?,
        ),
        None => BiomeSourceSpec::TheEnd,
    };
    Ok(DimensionSpec {
        dimension_type: dimension.clone(),
        noise_settings: Key::const_vanilla(settings),
        biome_source,
    })
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "tests assert on the Result directly")]
mod tests {
    use crate::datapack;
    use crate::world::worldgen;
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::process;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use std::sync::Once;

    fn init_global_registry() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            worldgen::ensure_registry_initialized(&datapack::Datapack::builtin())
                .expect("vanilla datapack should load");
        });
    }

    #[test]
    fn builtin_vanilla_dimensions_resolve_without_a_dimension_json() {
        init_global_registry();
        let source = Datapack::builtin();

        for (dimension, settings) in [
            ("overworld", "overworld"),
            ("the_nether", "nether"),
            ("the_end", "end"),
        ] {
            let id = Key::new(Key::VANILLA_NAMESPACE, dimension.to_owned());
            let spec = resolve(&source, &id).unwrap();
            assert_eq!(spec.noise_settings.path(), settings);
            assert_eq!(spec.dimension_type, id);
        }
    }

    #[test]
    fn unknown_dimension_without_json_is_an_error() {
        init_global_registry();
        let source = Datapack::builtin();
        let id = Key::const_vanilla("not_a_dimension");
        let error = match resolve(&source, &id) {
            Err(error) => error.to_string(),
            Ok(_) => panic!("unknown dimension should not resolve"),
        };
        assert!(
            error.contains("minecraft:the_nether"),
            "error should list the available dimensions, got: {error}"
        );
    }

    #[test]
    fn available_lists_the_builtin_vanilla_dimensions() {
        init_global_registry();
        let ids = available(&Datapack::builtin());
        assert_eq!(
            ids,
            vec![
                "minecraft:overworld".to_owned(),
                "minecraft:the_end".to_owned(),
                "minecraft:the_nether".to_owned(),
            ]
        );
    }

    #[test]
    fn the_end_uses_the_end_biome_source() {
        init_global_registry();
        let source = Datapack::builtin();
        let spec = resolve(&source, &Key::const_vanilla("the_end")).unwrap();
        assert!(matches!(spec.biome_source, BiomeSourceSpec::TheEnd));
    }

    #[test]
    fn float_climate_parameters_quantize_like_vanilla() {
        let point = parameter_point(&serde_json::json!({
            "temperature": [-1.004_785_9, -0.45],
            "humidity": [-1.0, 1.0],
            "continentalness": [-1.2, -0.455],
            "erosion": [-1.0, -0.78],
            "depth": [-0.005, 0.0],
            "weirdness": [-1.0, 1.0],
            "offset": 0.0
        }))
        .unwrap();
        assert_eq!(
            [point.temperature.min, point.temperature.max],
            [-10047, -4500]
        );
        assert_eq!(
            [point.continentalness.min, point.continentalness.max],
            [-12000, -4550]
        );
        assert_eq!([point.depth.min, point.depth.max], [-50, 0]);
        assert_eq!(point.offset, 0);
    }

    #[test]
    fn a_dimension_json_overrides_the_builtin_stem() {
        init_global_registry();
        let dir = tempdir();
        let dimension_dir = dir.join("data/minecraft/dimension");
        fs::create_dir_all(&dimension_dir).unwrap();
        fs::write(
            dimension_dir.join("overworld.json"),
            br#"{
                "type": "minecraft:overworld",
                "generator": {
                    "type": "minecraft:noise",
                    "settings": "minecraft:nether",
                    "biome_source": {
                        "type": "minecraft:multi_noise",
                        "biomes": [
                            {
                                "biome": "minecraft:plains",
                                "parameters": {
                                    "temperature": 0.0, "humidity": 0.0,
                                    "continentalness": 0.0, "erosion": 0.0,
                                    "depth": 0.0, "weirdness": 0.0, "offset": 0.0
                                }
                            }
                        ]
                    }
                }
            }"#,
        )
        .unwrap();

        let pack = Datapack::directory(&dir)
            .unwrap()
            .layered_over(Datapack::builtin());
        let spec = resolve(&pack, &Key::const_vanilla("overworld")).unwrap();

        assert_eq!(spec.noise_settings.path(), "nether");
        match spec.biome_source {
            BiomeSourceSpec::MultiNoise(parameters) => assert_eq!(parameters.values().len(), 1),
            _ => panic!("expected an inline multi_noise biome source"),
        }
    }

    fn tempdir() -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let dir = env::temp_dir().join(format!(
            "worldgen-rs-dimension-test-{}-{}",
            process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
