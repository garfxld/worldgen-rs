use std::error::Error as StdError;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;

#[derive(Debug)]
pub enum LoadError {
    Read {
        path: PathBuf,
        source: io::Error,
    },
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
}

impl Display for LoadError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, source } => write!(f, "failed to read {}: {source}", path.display()),
            Self::Parse { path, source } => {
                write!(f, "failed to parse {}: {source}", path.display())
            }
        }
    }
}

impl StdError for LoadError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
        }
    }
}

pub fn read(path: &Path) -> Result<String, LoadError> {
    fs::read_to_string(path).map_err(|source| LoadError::Read {
        path: path.to_path_buf(),
        source,
    })
}

pub fn parse_normalized<T: DeserializeOwned>(content: &str) -> Result<T, serde_json::Error> {
    let mut value: serde_json::Value = serde_json::from_str(content)?;
    normalize_type_tags(&mut value);
    serde_json::from_value(value)
}

pub fn read_and_parse_normalized<T: DeserializeOwned>(path: &Path) -> Result<T, LoadError> {
    let content = read(path)?;
    parse_normalized(&content).map_err(|source| LoadError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

fn normalize_type_tags(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::String(type_name)) = map.get_mut("type")
                && !type_name.contains(':')
            {
                *type_name = format!("minecraft:{type_name}");
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

#[must_use]
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

#[must_use]
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
