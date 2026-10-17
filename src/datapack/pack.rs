use std::env;
use std::fs::{self, File};
use std::io::{Error as IoError, ErrorKind, Read as _};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use crate::datapack::{
    DatapackError, collect_json_files, parse_id, path_to_id, read_and_parse_normalized,
};
use shulkr::registry::{TagFile, TagLoader};
use shulkr::util::Key;

const VANILLA_NAMESPACE: &str = Key::VANILLA_NAMESPACE;

#[derive(Debug, Clone)]
pub struct Datapack {
    layers: Vec<Layer>,
    _workdirs: Vec<Arc<TempDatapackDir>>,
}

static ACTIVE: OnceLock<Datapack> = OnceLock::new();

pub fn set_active(pack: Datapack) -> Result<(), Datapack> {
    ACTIVE.set(pack)
}

pub fn active() -> &'static Datapack {
    ACTIVE.get_or_init(Datapack::builtin)
}

#[derive(Debug, Clone)]
struct Layer {
    root: PathBuf,
    shape: Shape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    Namespaced,
    BareNamespace,
}

impl Layer {
    fn namespaces(&self) -> Vec<String> {
        match self.shape {
            Shape::BareNamespace => vec![VANILLA_NAMESPACE.to_owned()],
            Shape::Namespaced => {
                let mut found = Vec::new();
                let Ok(entries) = fs::read_dir(self.root.join("data")) else {
                    return found;
                };
                for entry in entries.flatten() {
                    if entry.path().is_dir() {
                        found.push(entry.file_name().to_string_lossy().into_owned());
                    }
                }
                found.sort();
                found
            }
        }
    }

    fn namespace_dir(&self, namespace: &str) -> Option<PathBuf> {
        let dir = match self.shape {
            Shape::BareNamespace => {
                if namespace != VANILLA_NAMESPACE {
                    return None;
                }
                self.root.clone()
            }
            Shape::Namespaced => self.root.join("data").join(namespace),
        };
        dir.is_dir().then_some(dir)
    }
}

fn detect_shape(root: &Path) -> Shape {
    if root.join("data").is_dir() {
        Shape::Namespaced
    } else {
        Shape::BareNamespace
    }
}

fn builtin_root() -> PathBuf {
    PathBuf::from(shulkr_data::DATAPACK_DIR)
}

impl Datapack {
    pub fn builtin() -> Self {
        Self::from_root(builtin_root())
    }

    pub fn directory(path: impl AsRef<Path>) -> Result<Self, DatapackError> {
        let path = path.as_ref();
        if !path.is_dir() {
            return Err(DatapackError::Read {
                path: path.to_path_buf(),
                source: IoError::new(ErrorKind::NotFound, "datapack path is not a directory"),
            });
        }
        Ok(Self::from_root(path.to_path_buf()))
    }

    pub fn zip(path: impl AsRef<Path>) -> Result<Self, DatapackError> {
        let path = path.as_ref();
        let read_error = |source: IoError| DatapackError::Read {
            path: path.to_path_buf(),
            source,
        };
        let invalid = |message: String| DatapackError::Read {
            path: path.to_path_buf(),
            source: IoError::new(ErrorKind::InvalidData, message),
        };

        let file = File::open(path).map_err(read_error)?;
        let mut archive = zip::ZipArchive::new(file).map_err(|e| invalid(e.to_string()))?;

        let workdir = TempDatapackDir::create().map_err(read_error)?;
        for index in 0..archive.len() {
            let mut entry = archive
                .by_index(index)
                .map_err(|e| invalid(e.to_string()))?;
            let Some(relative) = entry.enclosed_name() else {
                continue;
            };
            let dest = workdir.0.join(relative);
            if entry.is_dir() {
                fs::create_dir_all(&dest).map_err(read_error)?;
                continue;
            }
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).map_err(read_error)?;
            }
            let mut contents = Vec::new();
            entry.read_to_end(&mut contents).map_err(read_error)?;
            fs::write(&dest, contents).map_err(read_error)?;
        }

        let root = workdir.0.clone();
        Ok(Self {
            layers: vec![Layer {
                shape: detect_shape(&root),
                root,
            }],
            _workdirs: vec![Arc::new(workdir)],
        })
    }

    pub fn layered_over(self, base: Self) -> Self {
        let mut layers = base.layers;
        layers.extend(self.layers);
        let mut workdirs = base._workdirs;
        workdirs.extend(self._workdirs);
        Self {
            layers,
            _workdirs: workdirs,
        }
    }

    pub fn same_layers(&self, other: &Self) -> bool {
        self.layers.len() == other.layers.len()
            && self
                .layers
                .iter()
                .zip(&other.layers)
                .all(|(a, b)| a.root == b.root)
    }

    pub fn namespaces(&self) -> Vec<String> {
        let mut all: Vec<String> = self.layers.iter().flat_map(Layer::namespaces).collect();
        all.sort();
        all.dedup();
        all
    }

    pub fn dirs(&self, namespace: &str, relative: &str) -> Vec<PathBuf> {
        self.layers
            .iter()
            .filter_map(|layer| layer.namespace_dir(namespace))
            .map(|dir| dir.join(relative))
            .filter(|dir| dir.is_dir())
            .collect()
    }

    pub fn tags(&self, directory: &str) -> Result<TagLoader, DatapackError> {
        let mut loader = TagLoader::new();
        for (namespace, dir) in self.namespaced_dirs(directory) {
            for file in collect_json_files(&dir) {
                let tag: TagFile = read_and_parse_normalized(&file)?;
                loader.add(parse_id(&path_to_id(&namespace, &file, &dir)), tag);
            }
        }
        Ok(loader)
    }

    pub fn files(&self, namespace: &str, relative: &str) -> Vec<PathBuf> {
        self.layers
            .iter()
            .filter_map(|layer| layer.namespace_dir(namespace))
            .map(|dir| dir.join(relative))
            .filter(|file| file.is_file())
            .collect()
    }

    pub fn namespaced_dirs(&self, relative: &str) -> Vec<(String, PathBuf)> {
        let mut result = Vec::new();
        for namespace in self.namespaces() {
            for dir in self.dirs(&namespace, relative) {
                result.push((namespace.clone(), dir));
            }
        }
        result
    }

    fn from_root(root: PathBuf) -> Self {
        Self {
            layers: vec![Layer {
                shape: detect_shape(&root),
                root,
            }],
            _workdirs: Vec::new(),
        }
    }
}

#[derive(Debug)]
struct TempDatapackDir(PathBuf);

impl TempDatapackDir {
    fn create() -> Result<Self, IoError> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);

        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = env::temp_dir().join(format!("worldgen-rs-datapack-{}-{unique}", process::id()));
        fs::create_dir_all(&dir)?;
        Ok(Self(dir))
    }
}

impl Drop for TempDatapackDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "tests assert on the Result directly")]
mod tests {
    use std::fs;
    use std::io::Write as _;

    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    use super::{Datapack, TempDatapackDir};

    #[test]
    fn builtin_exposes_the_vanilla_namespace() {
        let pack = Datapack::builtin();
        assert_eq!(pack.namespaces(), vec!["minecraft".to_owned()]);
        assert!(
            !pack
                .dirs("minecraft", "worldgen/density_function")
                .is_empty()
        );
    }

    #[test]
    fn directory_rejects_a_missing_path() {
        let scratch = TempDatapackDir::create().unwrap();
        assert!(Datapack::directory(scratch.0.join("does-not-exist")).is_err());
    }

    #[test]
    fn zip_extracts_a_namespaced_datapack() {
        let scratch = TempDatapackDir::create().unwrap();
        let zip_path = scratch.0.join("pack.zip");

        let file = fs::File::create(&zip_path).unwrap();
        let mut writer = ZipWriter::new(file);
        writer
            .start_file(
                "data/example/worldgen/noise_settings/custom.json",
                SimpleFileOptions::default(),
            )
            .unwrap();
        writer.write_all(b"{}").unwrap();
        writer.finish().unwrap();

        let pack = Datapack::zip(&zip_path).unwrap();
        assert_eq!(pack.namespaces(), vec!["example".to_owned()]);
        assert_eq!(
            pack.files("example", "worldgen/noise_settings/custom.json")
                .len(),
            1
        );
    }

    #[test]
    fn zip_rejects_a_non_zip_file() {
        let scratch = TempDatapackDir::create().unwrap();
        let not_a_zip = scratch.0.join("not-a-zip.zip");
        fs::write(&not_a_zip, b"not a zip archive").unwrap();

        assert!(Datapack::zip(&not_a_zip).is_err());
    }

    #[test]
    fn layering_puts_the_override_last_and_keeps_both_namespaces() {
        let base_dir = TempDatapackDir::create().unwrap();
        fs::create_dir_all(base_dir.0.join("data/minecraft/worldgen/noise_settings")).unwrap();
        fs::write(
            base_dir
                .0
                .join("data/minecraft/worldgen/noise_settings/overworld.json"),
            b"base",
        )
        .unwrap();

        let over_dir = TempDatapackDir::create().unwrap();
        fs::create_dir_all(over_dir.0.join("data/minecraft/worldgen/noise_settings")).unwrap();
        fs::create_dir_all(over_dir.0.join("data/custom/worldgen")).unwrap();
        fs::write(
            over_dir
                .0
                .join("data/minecraft/worldgen/noise_settings/overworld.json"),
            b"override",
        )
        .unwrap();

        let merged = Datapack::directory(&over_dir.0)
            .unwrap()
            .layered_over(Datapack::directory(&base_dir.0).unwrap());

        assert_eq!(
            merged.namespaces(),
            vec!["custom".to_owned(), "minecraft".to_owned()]
        );

        let resolved = merged
            .files("minecraft", "worldgen/noise_settings/overworld.json")
            .pop()
            .unwrap();
        assert_eq!(fs::read_to_string(resolved).unwrap(), "override");
    }
}
