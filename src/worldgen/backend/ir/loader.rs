use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub use crate::util::datapack::LoadError;
use crate::util::datapack::{collect_json_files, path_to_id, read_and_parse_normalized};
use crate::worldgen::backend::ir::DensityProgram;
use crate::worldgen::backend::ir::json::{NoiseSettingsJson, router_to_entries};
use crate::worldgen::backend::ir::types::DensityFunction;

pub struct DatapackSource {
    root: PathBuf,
    namespace: String,
    extra_namespaces: Vec<(String, PathBuf)>,
}

impl DatapackSource {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            namespace: "minecraft".to_owned(),
            extra_namespaces: Vec::new(),
        }
    }

    #[must_use]
    pub fn add_namespace(
        mut self,
        namespace: impl Into<String>,
        namespace_worldgen_dir: impl Into<PathBuf>,
    ) -> Self {
        self.extra_namespaces
            .push((namespace.into(), namespace_worldgen_dir.into()));
        self
    }

    pub fn density_function_registry(&self) -> Result<BTreeMap<String, DensityFunction>, LoadError> {
        let mut registry = BTreeMap::new();
        Self::read_density_functions(&self.namespace, &self.root, &mut registry)?;
        for (namespace, worldgen_dir) in &self.extra_namespaces {
            Self::read_density_functions(namespace, worldgen_dir, &mut registry)?;
        }
        Ok(registry)
    }

    fn read_density_functions(
        namespace: &str,
        worldgen_dir: &Path,
        registry: &mut BTreeMap<String, DensityFunction>,
    ) -> Result<(), LoadError> {
        let dir = worldgen_dir.join("density_function");
        for file in collect_json_files(&dir) {
            let df: DensityFunction = read_and_parse_normalized(&file)?;
            registry.insert(path_to_id(namespace, &file, &dir), df);
        }
        Ok(())
    }

    pub fn noise_settings(&self, dimension: &str) -> Result<NoiseSettingsJson, LoadError> {
        let path = self.root.join(format!("noise_settings/{dimension}.json"));
        read_and_parse_normalized(&path)
    }

    pub fn program(
        &self,
        dimension: &str,
        prefix: &str,
        registry: &BTreeMap<String, DensityFunction>,
    ) -> Result<(DensityProgram, NoiseSettingsJson), LoadError> {
        let settings = self.noise_settings(dimension)?;
        let program = DensityProgram {
            registry: registry.clone(),
            router_entries: router_to_entries(&settings.noise_router),
            prefix: prefix.to_string(),
            cell_width: settings.noise.size_horizontal * 4,
            legacy_random_source: settings.legacy_random_source,
        };
        Ok((program, settings))
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test loads the checked-in vanilla datapack, which is known to parse"
)]
mod tests {
    use super::*;
    use crate::worldgen::backend::ir::analysis::collect_references;

    fn datapack_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/datapack/worldgen")
    }

    #[test]
    fn loads_full_vanilla_overworld_noise_settings_with_no_dangling_references() {
        let source = DatapackSource::new(datapack_root());
        let registry = source.density_function_registry().unwrap();
        assert!(!registry.is_empty());

        let (program, _settings) = source.program("overworld", "Overworld", &registry).unwrap();

        for df in program.router_entries.values() {
            assert_dangling_free(df, &program.registry, &mut Vec::new());
        }
    }

    fn assert_dangling_free(
        df: &DensityFunction,
        registry: &BTreeMap<String, DensityFunction>,
        visiting: &mut Vec<String>,
    ) {
        for id in collect_references(df) {
            assert!(
                registry.contains_key(&id),
                "dangling density function reference: {id}"
            );
            if visiting.contains(&id) {
                continue;
            }
            if let Some(target) = registry.get(&id) {
                visiting.push(id);
                assert_dangling_free(target, registry, visiting);
                visiting.pop();
            }
        }
    }
}
