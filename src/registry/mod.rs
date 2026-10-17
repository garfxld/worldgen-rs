pub mod biome;
pub mod block_tags;

pub mod carver;
pub mod fluid;

use shulkr::registry::registries;

use crate::datapack::{Datapack, DatapackError};

pub fn load(pack: &Datapack) -> Result<(), DatapackError> {
    registries::register_all();
    load_tags_and_biomes(pack)?;

    for (namespace, dir) in pack.namespaced_dirs("worldgen") {
        carver::load_carvers(&carver::CARVERS, &namespace, &dir)?;
    }

    Ok(())
}

pub fn load_tags_and_biomes(pack: &Datapack) -> Result<(), DatapackError> {
    let block_tags = pack.tags("tags/block")?;
    registries::BLOCK
        .set_tags(block_tags.build(|key| registries::BLOCK.get_id(key).is_some()))
        .expect("tags are loaded before the registries are frozen");
    let fluid_tags = pack.tags("tags/fluid")?;
    registries::FLUID
        .set_tags(fluid_tags.build(|key| registries::FLUID.get_id(key).is_some()))
        .expect("tags are loaded before the registries are frozen");

    for (namespace, dir) in pack.namespaced_dirs("worldgen") {
        biome::load_biomes(&registries::BIOME, &namespace, &dir)?;
    }
    let biome_tags = pack.tags("tags/worldgen/biome")?;
    registries::BIOME
        .set_tags(biome_tags.build(|key| registries::BIOME.get_id(key).is_some()))
        .expect("tags are loaded before the registries are frozen");
    Ok(())
}
