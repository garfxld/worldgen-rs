use crate::registry::REGISTRY;
use crate::registry::biome::BiomeRef;
use crate::worldgen::backend::ir::json::MultiNoiseBiomeParametersJson;
use crate::worldgen::biome::climate::{Parameter, ParameterList, ParameterPoint};

pub fn build_parameter_list(json: &MultiNoiseBiomeParametersJson) -> ParameterList<BiomeRef> {
    let values = json
        .biomes
        .iter()
        .map(|entry| {
            let id = entry
                .biome
                .parse()
                .unwrap_or_else(|e| panic!("invalid biome id {}: {e}", entry.biome));
            let biome = REGISTRY
                .biomes
                .by_key(&id)
                .unwrap_or_else(|| panic!("unknown biome {}", entry.biome));

            let point = ParameterPoint::new(
                Parameter::new(entry.t[0], entry.t[1]),
                Parameter::new(entry.h[0], entry.h[1]),
                Parameter::new(entry.c[0], entry.c[1]),
                Parameter::new(entry.e[0], entry.e[1]),
                Parameter::new(entry.d[0], entry.d[1]),
                Parameter::new(entry.w[0], entry.w[1]),
                entry.o,
            );
            (point, biome)
        })
        .collect();

    ParameterList::new(values)
}
