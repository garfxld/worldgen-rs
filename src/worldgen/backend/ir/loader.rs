use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::datapack::Datapack;
pub use crate::datapack::DatapackError;
use crate::datapack::{collect_json_files, path_to_id, read_and_parse_normalized};
use crate::util::ident::Identifier;
use crate::worldgen::backend::ir::DensityProgram;
use crate::worldgen::backend::ir::json::{
    MultiNoiseBiomeParametersJson, NoiseSettingsJson, router_to_entries,
};
use crate::worldgen::backend::ir::types::{DensityFunction, NoiseParameters};

pub struct DatapackSource {
    pack: Datapack,
}

impl DatapackSource {
    pub const fn new(pack: Datapack) -> Self {
        Self { pack }
    }

    pub fn builtin() -> Self {
        Self::new(Datapack::builtin())
    }

    pub const fn pack(&self) -> &Datapack {
        &self.pack
    }

    pub fn density_function_registry(
        &self,
    ) -> Result<BTreeMap<String, DensityFunction>, DatapackError> {
        let mut registry = BTreeMap::new();
        for (namespace, dir) in self.pack.namespaced_dirs("worldgen/density_function") {
            for file in collect_json_files(&dir) {
                let df: DensityFunction = read_and_parse_normalized(&file)?;
                registry.insert(path_to_id(&namespace, &file, &dir), df);
            }
        }
        Ok(registry)
    }

    pub fn noise_settings(&self, id: &Identifier) -> Result<NoiseSettingsJson, DatapackError> {
        let path = self.resolve(id, "worldgen/noise_settings")?;
        read_and_parse_normalized(&path)
    }

    pub fn multi_noise_biome_parameters(
        &self,
        id: &Identifier,
    ) -> Result<MultiNoiseBiomeParametersJson, DatapackError> {
        let path = self.resolve(id, "worldgen/multi_noise_biome_source_parameter_list")?;
        read_and_parse_normalized(&path)
    }

    pub fn noise_parameters(&self) -> Result<BTreeMap<String, NoiseParameters>, DatapackError> {
        let mut params = BTreeMap::new();
        for (namespace, dir) in self.pack.namespaced_dirs("worldgen/noise") {
            for file in collect_json_files(&dir) {
                let noise: NoiseParameters = read_and_parse_normalized(&file)?;
                params.insert(path_to_id(&namespace, &file, &dir), noise);
            }
        }
        Ok(params)
    }

    fn resolve(&self, id: &Identifier, relative: &str) -> Result<PathBuf, DatapackError> {
        let file = format!("{relative}/{}.json", id.path);
        self.pack
            .files(&id.namespace, &file)
            .pop()
            .ok_or_else(|| DatapackError::Read {
                path: PathBuf::from(file),
                source: std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("{id} not present in the datapack"),
                ),
            })
    }

    pub fn program(
        &self,
        settings_id: &Identifier,
        registry: &BTreeMap<String, DensityFunction>,
    ) -> Result<(DensityProgram, NoiseSettingsJson), DatapackError> {
        let settings = self.noise_settings(settings_id)?;
        let program = DensityProgram {
            registry: registry.clone(),
            router_entries: router_to_entries(&settings.noise_router),
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

    fn overworld() -> Identifier {
        Identifier::const_vanilla("overworld")
    }
    use crate::worldgen::backend::ir::analysis::collect_references;

    #[test]
    fn loads_full_vanilla_overworld_noise_settings_with_no_dangling_references() {
        let source = DatapackSource::builtin();
        let registry = source.density_function_registry().unwrap();
        assert!(!registry.is_empty());

        let (program, _settings) = source.program(&overworld(), &registry).unwrap();

        for df in program.router_entries.values() {
            assert_dangling_free(df, &program.registry, &mut Vec::new());
        }
    }

    #[test]
    fn overworld_noise_ids_all_resolve_to_parameters() {
        let source = DatapackSource::builtin();
        let registry = source.density_function_registry().unwrap();
        let (program, _settings) = source.program(&overworld(), &registry).unwrap();
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
