use std::io::{Error as IoError, ErrorKind};

use serde::Deserialize;

use crate::datapack::{DatapackError, collect_json_files, path_to_id, read_and_parse_normalized};
use crate::util::ident::Identifier;
use crate::worldgen::backend::ir::json::{MultiNoiseBiomeEntryJson, MultiNoiseBiomeParametersJson};
use crate::worldgen::backend::ir::loader::DatapackSource;

pub struct DimensionSpec {
    pub dimension_type: Identifier,
    pub noise_settings: Identifier,
    pub biome_source: BiomeSourceSpec,
}

pub enum BiomeSourceSpec {
    Overworld(MultiNoiseBiomeParametersJson),
    Nether(MultiNoiseBiomeParametersJson),
    End,
}

#[derive(Deserialize)]
struct DimensionJson {
    #[serde(rename = "type")]
    dimension_type: Identifier,
    generator: GeneratorJson,
}

#[derive(Deserialize)]
struct GeneratorJson {
    settings: Identifier,
    biome_source: BiomeSourceJson,
}

#[derive(Deserialize)]
struct BiomeSourceJson {
    #[serde(rename = "type")]
    kind: Identifier,
    preset: Option<Identifier>,
    biomes: Option<Vec<DatapackBiomeEntryJson>>,
}

#[derive(Deserialize)]
struct DatapackBiomeEntryJson {
    biome: String,
    parameters: DatapackClimateJson,
}

#[derive(Deserialize)]
struct DatapackClimateJson {
    temperature: ParameterJson,
    humidity: ParameterJson,
    continentalness: ParameterJson,
    erosion: ParameterJson,
    depth: ParameterJson,
    weirdness: ParameterJson,
    offset: ParameterJson,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ParameterJson {
    Point(f32),
    Range([f32; 2]),
}

impl ParameterJson {
    fn quantized(&self) -> [i64; 2] {
        match self {
            Self::Point(value) => {
                let q = quantize(*value);
                [q, q]
            }
            Self::Range([min, max]) => [quantize(*min), quantize(*max)],
        }
    }

    fn quantized_point(&self) -> i64 {
        self.quantized()[0]
    }
}

fn quantize(value: f32) -> i64 {
    (value * 10000.0) as i64
}

impl From<DatapackBiomeEntryJson> for MultiNoiseBiomeEntryJson {
    fn from(entry: DatapackBiomeEntryJson) -> Self {
        let p = entry.parameters;
        Self {
            biome: entry.biome,
            t: p.temperature.quantized(),
            h: p.humidity.quantized(),
            c: p.continentalness.quantized(),
            e: p.erosion.quantized(),
            d: p.depth.quantized(),
            w: p.weirdness.quantized(),
            o: p.offset.quantized_point(),
        }
    }
}

pub fn resolve(
    source: &DatapackSource,
    dimension: &Identifier,
) -> Result<DimensionSpec, DatapackError> {
    match source
        .pack()
        .files(
            &dimension.namespace,
            &format!("dimension/{}.json", dimension.path),
        )
        .pop()
    {
        Some(path) => from_json(source, dimension, read_and_parse_normalized(&path)?),
        None => builtin(source, dimension),
    }
}

fn from_json(
    source: &DatapackSource,
    dimension: &Identifier,
    json: DimensionJson,
) -> Result<DimensionSpec, DatapackError> {
    let biome_source = json.generator.biome_source;
    let biome_source = if biome_source.kind.path == "the_end" {
        BiomeSourceSpec::End
    } else {
        let parameters = match (biome_source.biomes, biome_source.preset) {
            (Some(biomes), _) => MultiNoiseBiomeParametersJson {
                biomes: biomes.into_iter().map(Into::into).collect(),
            },
            (None, Some(preset)) => source.multi_noise_biome_parameters(&preset)?,
            (None, None) => {
                return Err(DatapackError::Read {
                    path: format!("dimension/{}.json", dimension.path).into(),
                    source: IoError::new(
                        ErrorKind::InvalidData,
                        "multi_noise biome source has neither biomes nor a preset",
                    ),
                });
            }
        };
        multi_noise_spec(dimension, parameters)
    };

    Ok(DimensionSpec {
        dimension_type: json.dimension_type,
        noise_settings: json.generator.settings,
        biome_source,
    })
}

pub fn available(source: &DatapackSource) -> Vec<String> {
    let mut ids: Vec<String> = ["overworld", "the_nether", "the_end"]
        .iter()
        .map(|path| format!("minecraft:{path}"))
        .collect();

    for (namespace, dir) in source.pack().namespaced_dirs("dimension") {
        for file in collect_json_files(&dir) {
            ids.push(path_to_id(&namespace, &file, &dir));
        }
    }

    ids.sort();
    ids.dedup();
    ids
}

fn builtin(
    source: &DatapackSource,
    dimension: &Identifier,
) -> Result<DimensionSpec, DatapackError> {
    let settings = match (dimension.namespace.as_ref(), dimension.path.as_ref()) {
        ("minecraft", "overworld") => "overworld",
        ("minecraft", "the_nether") => "nether",
        ("minecraft", "the_end") => "end",
        _ => {
            return Err(DatapackError::Read {
                path: format!("dimension/{}.json", dimension.path).into(),
                source: IoError::new(
                    ErrorKind::NotFound,
                    format!(
                        "no dimension {dimension} in the datapack; available: {}",
                        available(source).join(", ")
                    ),
                ),
            });
        }
    };
    let noise_settings = Identifier::new(Identifier::VANILLA_NAMESPACE, settings.to_owned());

    let biome_source = if settings == "end" {
        BiomeSourceSpec::End
    } else {
        let preset = Identifier::new(Identifier::VANILLA_NAMESPACE, settings.to_owned());
        multi_noise_spec(dimension, source.multi_noise_biome_parameters(&preset)?)
    };

    Ok(DimensionSpec {
        dimension_type: dimension.clone(),
        noise_settings,
        biome_source,
    })
}

fn multi_noise_spec(
    dimension: &Identifier,
    parameters: MultiNoiseBiomeParametersJson,
) -> BiomeSourceSpec {
    if dimension.path == "the_nether" {
        BiomeSourceSpec::Nether(parameters)
    } else {
        BiomeSourceSpec::Overworld(parameters)
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "tests assert on the Result directly")]
mod tests {
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::process;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::datapack::Datapack;
    use crate::registry::{REGISTRY, Registries};
    use std::sync::Once;

    fn init_global_registry() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let mut registries = Registries::new_vanilla().unwrap();
            registries.freeze();
            let _ = REGISTRY.init(registries);
        });
    }

    #[test]
    fn builtin_vanilla_dimensions_resolve_without_a_dimension_json() {
        init_global_registry();
        let source = DatapackSource::builtin();

        for (dimension, settings) in [
            ("overworld", "overworld"),
            ("the_nether", "nether"),
            ("the_end", "end"),
        ] {
            let id = Identifier::new(Identifier::VANILLA_NAMESPACE, dimension.to_owned());
            let spec = resolve(&source, &id).unwrap();
            assert_eq!(spec.noise_settings.path, settings);
            assert_eq!(spec.dimension_type, id);
        }
    }

    #[test]
    fn unknown_dimension_without_json_is_an_error() {
        init_global_registry();
        let source = DatapackSource::builtin();
        let id = Identifier::const_vanilla("not_a_dimension");
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
        let ids = available(&DatapackSource::builtin());
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
        let source = DatapackSource::builtin();
        let spec = resolve(&source, &Identifier::const_vanilla("the_end")).unwrap();
        assert!(matches!(spec.biome_source, BiomeSourceSpec::End));
    }

    #[test]
    fn float_climate_parameters_quantize_like_vanilla() {
        let entry = DatapackBiomeEntryJson {
            biome: "minecraft:plains".to_owned(),
            parameters: DatapackClimateJson {
                temperature: ParameterJson::Range([-1.004_785_9, -0.45]),
                humidity: ParameterJson::Range([-1.0, 1.0]),
                continentalness: ParameterJson::Range([-1.2, -0.455]),
                erosion: ParameterJson::Range([-1.0, -0.78]),
                depth: ParameterJson::Range([-0.005, 0.0]),
                weirdness: ParameterJson::Range([-1.0, 1.0]),
                offset: ParameterJson::Point(0.0),
            },
        };

        let converted: MultiNoiseBiomeEntryJson = entry.into();
        assert_eq!(converted.t, [-10047, -4500]);
        assert_eq!(converted.c, [-12000, -4550]);
        assert_eq!(converted.d, [-50, 0]);
        assert_eq!(converted.o, 0);
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
        let source = DatapackSource::new(pack);
        let spec = resolve(&source, &Identifier::const_vanilla("overworld")).unwrap();

        assert_eq!(spec.noise_settings.path, "nether");
        match spec.biome_source {
            BiomeSourceSpec::Overworld(parameters) => assert_eq!(parameters.biomes.len(), 1),
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
