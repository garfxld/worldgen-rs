use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub use crate::util::datapack::DatapackError;
use crate::util::datapack::{collect_json_files, path_to_id, read_and_parse_normalized};
use crate::worldgen::backend::ir::DensityProgram;
use crate::worldgen::backend::ir::json::{
    MultiNoiseBiomeParametersJson, NoiseSettingsJson, router_to_entries,
};
use crate::worldgen::backend::ir::types::{DensityFunction, NoiseParameters};

pub struct DatapackSource {
    root: PathBuf,
    namespace: String,
    extra_namespaces: Vec<(String, PathBuf)>,
}

impl DatapackSource {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            namespace: "minecraft".to_owned(),
            extra_namespaces: Vec::new(),
        }
    }

    pub fn add_namespace(
        mut self,
        namespace: impl Into<String>,
        namespace_worldgen_dir: impl Into<PathBuf>,
    ) -> Self {
        self.extra_namespaces
            .push((namespace.into(), namespace_worldgen_dir.into()));
        self
    }

    pub fn density_function_registry(
        &self,
    ) -> Result<BTreeMap<String, DensityFunction>, DatapackError> {
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
    ) -> Result<(), DatapackError> {
        let dir = worldgen_dir.join("density_function");
        for file in collect_json_files(&dir) {
            let df: DensityFunction = read_and_parse_normalized(&file)?;
            registry.insert(path_to_id(namespace, &file, &dir), df);
        }
        Ok(())
    }

    pub fn noise_settings(&self, dimension: &str) -> Result<NoiseSettingsJson, DatapackError> {
        let path = self.root.join(format!("noise_settings/{dimension}.json"));
        read_and_parse_normalized(&path)
    }

    pub fn multi_noise_biome_parameters(
        &self,
        dimension: &str,
    ) -> Result<MultiNoiseBiomeParametersJson, DatapackError> {
        let path = self.root.join(format!(
            "multi_noise_biome_source_parameter_list/{dimension}.json"
        ));
        read_and_parse_normalized(&path)
    }

    pub fn noise_parameters(&self) -> Result<BTreeMap<String, NoiseParameters>, DatapackError> {
        let mut params = BTreeMap::new();
        Self::read_noise_parameters(&self.namespace, &self.root, &mut params)?;
        for (namespace, worldgen_dir) in &self.extra_namespaces {
            Self::read_noise_parameters(namespace, worldgen_dir, &mut params)?;
        }
        Ok(params)
    }

    fn read_noise_parameters(
        namespace: &str,
        worldgen_dir: &Path,
        params: &mut BTreeMap<String, NoiseParameters>,
    ) -> Result<(), DatapackError> {
        let dir = worldgen_dir.join("noise");
        for file in collect_json_files(&dir) {
            let noise: NoiseParameters = read_and_parse_normalized(&file)?;
            params.insert(path_to_id(namespace, &file, &dir), noise);
        }
        Ok(())
    }

    pub fn program(
        &self,
        dimension: &str,
        prefix: &str,
        registry: &BTreeMap<String, DensityFunction>,
    ) -> Result<(DensityProgram, NoiseSettingsJson), DatapackError> {
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

    #[test]
    fn overworld_noise_ids_all_resolve_to_parameters() {
        let source = DatapackSource::new(datapack_root());
        let registry = source.density_function_registry().unwrap();
        let (program, _settings) = source.program("overworld", "Overworld", &registry).unwrap();
        let params = source.noise_parameters().unwrap();
        assert!(!params.is_empty());

        let analysis = program.analyze();
        for id in &analysis.noise_ids {
            assert!(params.contains_key(id), "no noise parameters for {id}");
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
