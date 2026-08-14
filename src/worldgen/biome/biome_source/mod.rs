use rustc_hash::FxHashSet;

use crate::registry::REGISTRY;
use crate::registry::biome::BiomeRef;
use crate::util::ident::Identifier;
use crate::util::noise::EndIslands;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::worldgen::biome::climate::ParameterList;
use crate::worldgen::biome::climate_sampler::ClimateSampler;
use crate::worldgen::density::{ColumnCache, Noises};

#[non_exhaustive]
pub enum BiomeSourceKind {
    Overworld(MultiNoiseBiomeSource),
    Nether(MultiNoiseBiomeSource),
    End(Box<EndBiomeSource>),
}

impl BiomeSourceKind {
    pub const fn overworld(seed: u64, noises: Noises, parameters: ParameterList<BiomeRef>) -> Self {
        Self::Overworld(MultiNoiseBiomeSource::new(seed, noises, parameters))
    }

    pub const fn nether(seed: u64, noises: Noises, parameters: ParameterList<BiomeRef>) -> Self {
        Self::Nether(MultiNoiseBiomeSource::new(seed, noises, parameters))
    }

    pub fn end(seed: u64) -> Self {
        Self::End(Box::new(EndBiomeSource::new(seed)))
    }

    pub fn possible_biome_refs(&self) -> Vec<BiomeRef> {
        let biomes: Vec<BiomeRef> = match self {
            Self::Overworld(source) | Self::Nether(source) => source
                .parameters
                .values()
                .iter()
                .map(|(_, biome)| *biome)
                .collect(),
            Self::End(source) => source.possible_biomes().to_vec(),
        };

        distinct_biome_refs(biomes)
    }

    pub fn possible_biomes(&self) -> FxHashSet<Identifier> {
        self.possible_biome_refs()
            .into_iter()
            .map(|biome| biome.key.clone())
            .collect()
    }

    pub fn chunk_sampler(&self) -> ChunkBiomeSampler<'_> {
        match self {
            Self::Overworld(source) | Self::Nether(source) => {
                ChunkBiomeSampler::MultiNoise(Box::new(source.chunk_sampler()))
            }
            Self::End(source) => ChunkBiomeSampler::End(Box::new(source.chunk_sampler())),
        }
    }

    pub fn find_biome_horizontal(
        &self,
        origin_x: i32,
        origin_z: i32,
        search_radius: i32,
        allowed: &dyn Fn(BiomeRef) -> bool,
        rng: &mut RandomSource,
    ) -> Option<(i32, i32)> {
        let mut sampler = self.chunk_sampler();
        let noise_center_x = origin_x >> 2;
        let noise_center_z = origin_z >> 2;
        let noise_radius = search_radius >> 2;

        let mut result: Option<(i32, i32)> = None;
        let mut found = 0;
        for z in -noise_radius..=noise_radius {
            for x in -noise_radius..=noise_radius {
                let nx = noise_center_x + x;
                let nz = noise_center_z + z;
                if allowed(sampler.sample(nx, 0, nz)) {
                    if result.is_none() || rng.next_i32_bounded(found + 1) == 0 {
                        result = Some((nx << 2, nz << 2));
                    }
                    found += 1;
                }
            }
        }
        result
    }

    pub fn initial_spawn_search_origin(&self) -> BlockPos {
        match self {
            Self::Overworld(source) => source.climate_sampler.find_spawn_position(),
            Self::Nether(_) | Self::End(_) => BlockPos::new(0, 0, 0),
        }
    }
}

fn distinct_biome_refs(biomes: Vec<BiomeRef>) -> Vec<BiomeRef> {
    let mut seen = FxHashSet::default();
    let mut distinct = Vec::with_capacity(biomes.len());

    for biome in biomes {
        if seen.insert(&biome.key) {
            distinct.push(biome);
        }
    }

    distinct
}

pub struct MultiNoiseBiomeSource {
    seed: u64,
    climate_sampler: ClimateSampler,
    parameters: ParameterList<BiomeRef>,
}

impl MultiNoiseBiomeSource {
    pub const fn new(seed: u64, noises: Noises, parameters: ParameterList<BiomeRef>) -> Self {
        Self {
            seed,
            climate_sampler: ClimateSampler::new(noises),
            parameters,
        }
    }

    pub const fn seed(&self) -> u64 {
        self.seed
    }

    pub const fn climate_sampler(&self) -> &ClimateSampler {
        &self.climate_sampler
    }

    fn chunk_sampler(&self) -> MultiNoiseChunkBiomeSampler<'_> {
        MultiNoiseChunkBiomeSampler {
            source: self,
            column_cache: ColumnCache::new(self.climate_sampler.noises()),
            biome_cache: None,
        }
    }
}

pub struct MultiNoiseChunkBiomeSampler<'a> {
    source: &'a MultiNoiseBiomeSource,
    column_cache: ColumnCache,
    biome_cache: Option<usize>,
}

impl MultiNoiseChunkBiomeSampler<'_> {
    fn sample(&mut self, quart_x: i32, quart_y: i32, quart_z: i32) -> BiomeRef {
        let target =
            self.source
                .climate_sampler
                .sample(quart_x, quart_y, quart_z, &mut self.column_cache);
        self.source
            .parameters
            .find_value_cached(&target, &mut self.biome_cache)
    }

    fn init_grid(&mut self, chunk_block_x: i32, chunk_block_z: i32) {
        self.source.climate_sampler.init_column_grid(
            &mut self.column_cache,
            chunk_block_x,
            chunk_block_z,
        );
    }
}

const END_BIOME_PATHS: [&str; 5] = [
    "the_end",
    "end_highlands",
    "end_midlands",
    "small_end_islands",
    "end_barrens",
];

pub struct EndBiomeSource {
    seed: u64,
    end_islands: EndIslands,
    biomes: [BiomeRef; 5],
}

impl EndBiomeSource {
    pub fn new(seed: u64) -> Self {
        let biomes = END_BIOME_PATHS.map(|path| {
            REGISTRY
                .biomes
                .by_key(&Identifier::const_vanilla(path))
                .unwrap_or_else(|| panic!("missing end biome minecraft:{path}"))
        });

        Self {
            seed,
            end_islands: EndIslands::new(seed),
            biomes,
        }
    }

    pub const fn seed(&self) -> u64 {
        self.seed
    }

    pub const fn possible_biomes(&self) -> &[BiomeRef; 5] {
        &self.biomes
    }

    const fn chunk_sampler(&self) -> EndChunkBiomeSampler<'_> {
        EndChunkBiomeSampler {
            source: self,
            cached_erosion: None,
        }
    }
}

pub struct EndChunkBiomeSampler<'a> {
    source: &'a EndBiomeSource,
    cached_erosion: Option<(i32, i32, f64)>,
}

impl EndChunkBiomeSampler<'_> {
    fn get_erosion(&mut self, chunk_x: i32, chunk_z: i32) -> f64 {
        if let Some((cx, cz, erosion)) = self.cached_erosion
            && cx == chunk_x
            && cz == chunk_z
        {
            return erosion;
        }
        let weird_block_x = f64::from((chunk_x * 2 + 1) * 8);
        let weird_block_z = f64::from((chunk_z * 2 + 1) * 8);
        let erosion = self
            .source
            .end_islands
            .sample(weird_block_x, 0.0, weird_block_z);
        self.cached_erosion = Some((chunk_x, chunk_z, erosion));
        erosion
    }

    fn sample(&mut self, quart_x: i32, _quart_y: i32, quart_z: i32) -> BiomeRef {
        let block_x = quart_x << 2;
        let block_z = quart_z << 2;
        let chunk_x = block_x >> 4;
        let chunk_z = block_z >> 4;

        let [
            the_end,
            end_highlands,
            end_midlands,
            small_end_islands,
            end_barrens,
        ] = self.source.biomes;

        if i64::from(chunk_x) * i64::from(chunk_x) + i64::from(chunk_z) * i64::from(chunk_z) <= 4096
        {
            return the_end;
        }

        let erosion = self.get_erosion(chunk_x, chunk_z);

        if erosion > 0.25 {
            end_highlands
        } else if erosion >= -0.0625 {
            end_midlands
        } else if erosion < -0.21875 {
            small_end_islands
        } else {
            end_barrens
        }
    }
}

pub enum ChunkBiomeSampler<'a> {
    MultiNoise(Box<MultiNoiseChunkBiomeSampler<'a>>),
    End(Box<EndChunkBiomeSampler<'a>>),
}

impl ChunkBiomeSampler<'_> {
    #[inline]
    pub fn sample(&mut self, quart_x: i32, quart_y: i32, quart_z: i32) -> BiomeRef {
        match self {
            Self::MultiNoise(s) => s.sample(quart_x, quart_y, quart_z),
            Self::End(s) => s.sample(quart_x, quart_y, quart_z),
        }
    }

    pub fn init_grid(&mut self, chunk_block_x: i32, chunk_block_z: i32) {
        match self {
            Self::MultiNoise(s) => s.init_grid(chunk_block_x, chunk_block_z),
            Self::End(_) => {}
        }
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test loads the checked-in vanilla datapack"
)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Once;

    use super::*;
    use crate::registry::Registries;
    use crate::worldgen::backend::ir::loader::DatapackSource;
    use crate::worldgen::biome::multi_noise::build_parameter_list;

    fn datapack_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/datapack/worldgen")
    }

    fn init_global_registry() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let registries = Registries::new_vanilla().expect("vanilla datapack should load");
            let _ = REGISTRY.init(registries);
        });
    }

    fn build(dimension: &str, prefix: &str, seed: u64) -> BiomeSourceKind {
        let source = DatapackSource::new(datapack_root());
        let registry = source.density_function_registry().unwrap();
        let (program, settings) = source.program(dimension, prefix, &registry).unwrap();
        let params = source.noise_parameters().unwrap();
        let noises = Noises::new(&program, &settings, &params, seed).unwrap();
        let biome_params = source.multi_noise_biome_parameters(dimension).unwrap();
        let parameters = build_parameter_list(&biome_params);
        if dimension == "nether" {
            BiomeSourceKind::nether(seed, noises, parameters)
        } else {
            BiomeSourceKind::overworld(seed, noises, parameters)
        }
    }

    #[test]
    fn overworld_biome_sampling_agrees_lazy_and_grid() {
        init_global_registry();
        let source = build("overworld", "Overworld", 42);

        let mut lazy = source.chunk_sampler();
        let mut gridded = source.chunk_sampler();
        gridded.init_grid(0, 0);

        for (qx, qz) in [(0, 0), (1, 0), (0, 1), (3, 3)] {
            let a = lazy.sample(qx, 15, qz).key.clone();
            let b = gridded.sample(qx, 15, qz).key.clone();
            assert_eq!(a, b, "mismatch at quart ({qx}, {qz})");
        }
    }

    #[test]
    fn overworld_and_nether_have_disjoint_biome_pools() {
        init_global_registry();
        let overworld = build("overworld", "Overworld", 1);
        let nether = build("nether", "Nether", 1);

        let overworld_biomes = overworld.possible_biomes();
        let nether_biomes = nether.possible_biomes();

        assert!(!overworld_biomes.is_empty());
        assert!(!nether_biomes.is_empty());
        assert!(overworld_biomes.contains(&Identifier::const_vanilla("plains")));
        assert!(nether_biomes.contains(&Identifier::const_vanilla("nether_wastes")));
        assert!(!overworld_biomes.contains(&Identifier::const_vanilla("nether_wastes")));
    }

    #[test]
    fn end_biome_source_places_the_end_at_origin() {
        init_global_registry();
        let end = BiomeSourceKind::end(7);
        let mut sampler = end.chunk_sampler();
        let biome = sampler.sample(0, 0, 0);
        assert_eq!(biome.key, Identifier::const_vanilla("the_end"));
        assert_eq!(end.possible_biome_refs().len(), 5);
    }

    #[test]
    fn overworld_spawn_search_returns_a_position() {
        init_global_registry();
        let overworld = build("overworld", "Overworld", 42);
        let origin = overworld.initial_spawn_search_origin();
        assert_eq!(origin.y(), 0);

        let nether = build("nether", "Nether", 42);
        assert_eq!(nether.initial_spawn_search_origin(), BlockPos::new(0, 0, 0));
    }
}
