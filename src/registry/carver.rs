use std::path::Path;

use shulkr::registry::Registry;

use crate::datapack::{
    DatapackError, collect_json_files, parse_id, path_to_id, read_and_parse_normalized,
};
use crate::world::worldgen::carver::WorldCarver;

/// Filled from the active datapack by [crate::registry::load].
pub static CARVERS: Registry<WorldCarver> = Registry::new("minecraft:worldgen/carver");

pub fn load_carvers(
    registry: &Registry<WorldCarver>,
    namespace: &str,
    worldgen_dir: &Path,
) -> Result<(), DatapackError> {
    let dir = worldgen_dir.join("carver");
    for file in collect_json_files(&dir) {
        let carver: WorldCarver = read_and_parse_normalized(&file)?;
        registry.register(parse_id(&path_to_id(namespace, &file, &dir)), carver)?;
    }
    Ok(())
}
