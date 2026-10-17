use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use serde_json::Value;

use crate::datapack::{Datapack, DatapackError, collect_json_files, path_to_id, read};
use crate::world::worldgen::synth::normal_noise::{NormalNoise, Parameters};

use super::function::Df;
use super::parse::ParseContext;
use serde::de;
use std::path::Path;

#[derive(Default)]
pub struct DensityRegistry {
    pub noises: BTreeMap<String, Arc<NormalNoise>>,
    pub holders: BTreeMap<String, Arc<OnceLock<Df>>>,
}

fn parse_error(path: &Path, message: String) -> DatapackError {
    DatapackError::Parse {
        path: path.to_path_buf(),
        source: de::Error::custom(message),
    }
}

impl DensityRegistry {
    pub fn load(pack: &Datapack) -> Result<Self, DatapackError> {
        let mut registry = Self::default();
        for (namespace, dir) in pack.namespaced_dirs("worldgen/noise") {
            for file in collect_json_files(&dir) {
                let content = read(&file)?;
                let params: Parameters =
                    serde_json::from_str(&content).map_err(|source| DatapackError::Parse {
                        path: file.clone(),
                        source,
                    })?;
                registry.noises.insert(
                    path_to_id(&namespace, &file, &dir),
                    Arc::new(NormalNoise::new(params)),
                );
            }
        }

        let mut sources = Vec::new();
        for (namespace, dir) in pack.namespaced_dirs("worldgen/density_function") {
            for file in collect_json_files(&dir) {
                let id = path_to_id(&namespace, &file, &dir);
                registry
                    .holders
                    .entry(id.clone())
                    .or_insert_with(|| Arc::new(OnceLock::new()));
                sources.push((id, file));
            }
        }

        let mut parsed = BTreeMap::new();
        for (id, file) in sources {
            let content = read(&file)?;
            let value: Value =
                serde_json::from_str(&content).map_err(|source| DatapackError::Parse {
                    path: file.clone(),
                    source,
                })?;
            let df = registry.parse(&value).map_err(|e| parse_error(&file, e))?;
            parsed.insert(id, df);
        }
        for (id, df) in parsed {
            let _ = registry.holders[&id].set(df);
        }
        Ok(registry)
    }

    pub fn parse(&self, value: &Value) -> Result<Df, String> {
        ParseContext {
            holders: &self.holders,
            noises: &self.noises,
        }
        .parse(value)
    }

    pub fn holder(&self, key: &str) -> Result<Df, String> {
        ParseContext {
            holders: &self.holders,
            noises: &self.noises,
        }
        .holder(key)
    }
}
