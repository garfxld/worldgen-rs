mod biome_resolver;

use rustc_hash::FxHashSet;
use shulkr::util::Key;

use crate::registry::biome::BiomeRef;
use crate::world::worldgen::biome::climate::{ParameterList, Sampler, TargetPoint};
use shulkr::registry::registries;

pub use biome_resolver::BiomeResolver;

pub enum BiomeSource {
    MultiNoise(ParameterList<BiomeRef>),
    TheEnd([BiomeRef; 5]),
    Fixed(BiomeRef),
}

impl BiomeSource {
    pub const fn multi_noise(parameters: ParameterList<BiomeRef>) -> Self {
        Self::MultiNoise(parameters)
    }

    pub fn the_end() -> Self {
        Self::TheEnd(END_BIOME_PATHS.map(|path| {
            registries::BIOME
                .by_key(&Key::const_vanilla(path))
                .unwrap_or_else(|| panic!("missing end biome minecraft:{path}"))
        }))
    }

    const fn is_multi_noise(&self) -> bool {
        matches!(self, Self::MultiNoise(_))
    }

    pub fn possible_biomes(&self) -> Vec<BiomeRef> {
        match self {
            Self::MultiNoise(parameters) => distinct_biome_refs(
                parameters
                    .values()
                    .iter()
                    .map(|(_, biome)| *biome)
                    .collect(),
            ),
            Self::TheEnd(biomes) => biomes.to_vec(),
            Self::Fixed(biome) => vec![*biome],
        }
    }

    fn multi_noise_biome(&self, target: &TargetPoint, last_result: &mut Option<usize>) -> BiomeRef {
        match self {
            Self::MultiNoise(parameters) => parameters.find_value_cached(target, last_result),
            Self::TheEnd(_) | Self::Fixed(_) => {
                unreachable!("only multi-noise biome sources have climate parameters")
            }
        }
    }

    fn get_noise_biome(
        &self,
        sampler: &Sampler,
        last_result: &mut Option<usize>,
        quart_x: i32,
        quart_y: i32,
        quart_z: i32,
    ) -> BiomeRef {
        match self {
            Self::Fixed(biome) => biome,
            Self::MultiNoise(_) => {
                let target = sampler.sample(quart_x, quart_y, quart_z);
                self.multi_noise_biome(&target, last_result)
            }
            Self::TheEnd([end, highlands, midlands, islands, barrens]) => {
                let block_x = quart_x << 2;
                let block_y = quart_y << 2;
                let block_z = quart_z << 2;
                let chunk_x = block_x >> 4;
                let chunk_z = block_z >> 4;
                if i64::from(chunk_x) * i64::from(chunk_x) + i64::from(chunk_z) * i64::from(chunk_z)
                    <= 4096
                {
                    return end;
                }
                let weird_block_x = ((block_x >> 4) * 2 + 1) * 8;
                let weird_block_z = ((block_z >> 4) * 2 + 1) * 8;
                let height_value = f64::from(sampler.sample_value(
                    &sampler.erosion,
                    weird_block_x,
                    block_y,
                    weird_block_z,
                ));
                if height_value > 0.25 {
                    highlands
                } else if height_value >= -0.0625 {
                    midlands
                } else if height_value < -0.21875 {
                    islands
                } else {
                    barrens
                }
            }
        }
    }
}

fn distinct_biome_refs(biomes: Vec<BiomeRef>) -> Vec<BiomeRef> {
    let mut seen = FxHashSet::default();
    let mut distinct = Vec::with_capacity(biomes.len());

    for biome in biomes {
        if seen.insert(biome.key()) {
            distinct.push(biome);
        }
    }

    distinct
}

const END_BIOME_PATHS: [&str; 5] = [
    "the_end",
    "end_highlands",
    "end_midlands",
    "small_end_islands",
    "end_barrens",
];

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test loads the checked-in vanilla datapack"
)]
mod tests {
    use crate::world::worldgen;
    use std::sync::Once;

    use super::*;
    use crate::datapack::Datapack;
    use crate::world::worldgen::biome::multi_noise::preset_parameter_list;

    fn init_global_registry() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            worldgen::ensure_registry_initialized(&Datapack::builtin())
                .expect("vanilla datapack should load");
        });
    }

    #[test]
    fn overworld_and_nether_have_disjoint_biome_pools() {
        init_global_registry();
        let overworld = BiomeSource::multi_noise(
            preset_parameter_list(&Key::const_vanilla("overworld"))
                .unwrap()
                .unwrap(),
        );
        let nether = BiomeSource::multi_noise(
            preset_parameter_list(&Key::const_vanilla("nether"))
                .unwrap()
                .unwrap(),
        );
        let keys = |source: &BiomeSource| -> FxHashSet<Key> {
            source
                .possible_biomes()
                .iter()
                .map(|biome| biome.key().clone())
                .collect()
        };
        let overworld_biomes = keys(&overworld);
        let nether_biomes = keys(&nether);
        assert!(overworld_biomes.contains(&Key::const_vanilla("plains")));
        assert!(nether_biomes.contains(&Key::const_vanilla("nether_wastes")));
        assert!(!overworld_biomes.contains(&Key::const_vanilla("nether_wastes")));
    }

    #[test]
    fn the_end_has_five_biomes() {
        init_global_registry();
        assert_eq!(BiomeSource::the_end().possible_biomes().len(), 5);
    }
}
