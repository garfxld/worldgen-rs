use std::error::Error as StdError;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde::de::DeserializeOwned;
use shulkr::registry::RegistryFrozen;
use shulkr::util::Key;

use crate::datapack::Lenient;

#[derive(Debug)]
pub enum DatapackError {
    Read {
        path: PathBuf,
        source: io::Error,
    },
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
    Registry(RegistryFrozen),
}

impl From<RegistryFrozen> for DatapackError {
    fn from(error: RegistryFrozen) -> Self {
        Self::Registry(error)
    }
}

impl Display for DatapackError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => write!(f, "failed to read {}: {source}", path.display()),
            Self::Parse { path, source } => {
                write!(f, "failed to parse {}: {source}", path.display())
            }
            Self::Registry(error) => Display::fmt(error, f),
        }
    }
}

impl StdError for DatapackError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
            Self::Registry(error) => Some(error),
        }
    }
}

pub fn read(path: &Path) -> Result<String, DatapackError> {
    fs::read_to_string(path).map_err(|source| DatapackError::Read {
        path: path.to_path_buf(),
        source,
    })
}

pub fn parse_normalized<T: DeserializeOwned>(content: &str) -> Result<T, serde_json::Error> {
    let mut value: serde_json::Value = serde_json::from_str(content)?;
    normalize_type_tags(&mut value);
    T::deserialize(Lenient(value))
}

pub fn lenient_or_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: DeserializeOwned + Default,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(T::deserialize(Lenient(value)).unwrap_or_default())
}

pub fn read_and_parse_normalized<T: DeserializeOwned>(path: &Path) -> Result<T, DatapackError> {
    let content = read(path)?;
    parse_normalized(&content).map_err(|source| DatapackError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

fn normalize_type_tags(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for key in ["type", "predicate_type"] {
                if let Some(serde_json::Value::String(type_name)) = map.get_mut(key)
                    && !type_name.contains(':')
                {
                    *type_name = format!("minecraft:{type_name}");
                }
            }
            for entry in map.values_mut() {
                normalize_type_tags(entry);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                normalize_type_tags(item);
            }
        }
        _ => {}
    }
}

pub fn collect_json_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return files;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files.extend(collect_json_files(&path));
        } else if path.extension().is_some_and(|ext| ext == "json") {
            files.push(path);
        }
    }
    files.sort();
    files
}

pub fn path_to_id(namespace: &str, path: &Path, base_dir: &Path) -> String {
    let relative = path
        .strip_prefix(base_dir)
        .expect("datapack file path should be under its containing directory");
    let id_path = relative
        .with_extension("")
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/");
    format!("{namespace}:{id_path}")
}

const EMPTY_LIST: &[serde_json::Value] = &[];

pub fn parse_id(raw: &str) -> Key {
    Key::of(raw.to_owned())
}

pub fn type_of(value: &serde_json::Value) -> &str {
    value["type"]
        .as_str()
        .unwrap_or_else(|| panic!("missing type field in {value}"))
}

pub fn value_list(value: &serde_json::Value) -> &[serde_json::Value] {
    value.as_array().map_or(EMPTY_LIST, Vec::as_slice)
}

pub fn parse_json<T: DeserializeOwned>(value: &serde_json::Value) -> T {
    T::deserialize(Lenient(value.clone())).unwrap_or_else(|e| panic!("invalid json {value}: {e}"))
}
