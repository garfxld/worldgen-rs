use std::cell::RefCell;
use std::sync::{Arc, OnceLock};
use std::time::Instant;

use rustc_hash::{FxHashMap, FxHashSet};
use shulkr::util::{BlockPosition, Key};

use crate::datapack::Datapack;
use crate::datapack::DatapackError;
use crate::registry::biome::BiomeRef;
use crate::util::pos::ChunkPos;
use crate::util::rng::RandomSource;
use crate::world::chunk::heightmap::HeightmapType;
use crate::world::chunk::proto::Chunk;
use crate::world::worldgen::beardifier::Beardifier;
use crate::world::worldgen::biome::biome_manager::BiomeManager;
use crate::world::worldgen::biome::biome_source::BiomeSource;
use crate::world::worldgen::biome::obfuscate_biome_seed;
use crate::world::worldgen::densityfunction::context::SamplerContext;
use crate::world::worldgen::densityfunction::registry::DensityRegistry;
use crate::world::worldgen::dimension::{
    BiomeSourceSpec, DimensionSpec, GeneratorSpec, dimension_type,
};
use crate::world::worldgen::feature::sorter::{StepFeatureData, build_features_per_step};
use crate::world::worldgen::feature::{PlacedFeature, features};
use crate::world::worldgen::generator::flat::FlatGenerator;
use crate::world::worldgen::material::registry::MaterialRegistry;
use crate::world::worldgen::noise_based_chunk_generator::{NoiseBasedChunkGenerator, NoiseColumn};
use crate::world::worldgen::noise_generator_settings::NoiseGeneratorSettings;
use crate::world::worldgen::noise_spawn_finder::find_spawn_position;
use crate::world::worldgen::random_state::RandomState;
use crate::world::worldgen::structure::structure::Structure;
use crate::world::worldgen::structure::structure_set::StructureSet;
use crate::world::worldgen::worldgen_context::WorldGenContext;
use shulkr::registry::registries;

const WORLDGEN_REGION_RANDOM: &str = "minecraft:worldgen_region_random";
pub const GENERATION_STEPS: usize = 11;

mod debug;
mod feature;
mod flat;
mod structure;

const SIMPLE_GEN_DEPTH: i32 = 384;
const FLAT_SEA_LEVEL: i32 = -63;
const DEBUG_SEA_LEVEL: i32 = 63;

#[derive(Debug, thiserror::Error)]
pub enum GeneratorError {
    #[error(transparent)]
    Datapack(#[from] DatapackError),
    #[error("unknown biome {0}")]
    UnknownBiome(Key),
}

enum GeneratorKind {
    Noise,
    Flat(FlatGenerator),
    Debug,
}

pub struct Generator {
    chunk_generator: NoiseBasedChunkGenerator,
    kind: GeneratorKind,
    random_state: RandomState,
    biome_zoom_seed: i64,
    seed: i64,
    sea_level: i32,
    min_y: i32,
    height: i32,
    features_per_step: Vec<StepFeatureData>,
    possible_biome_ids: FxHashSet<u16>,
    possible_biomes: Vec<BiomeRef>,
    possible_structure_sets: OnceLock<Vec<&'static StructureSet>>,
    structures_by_step: OnceLock<Vec<Vec<(Key, &'static Structure)>>>,
    ring_positions: OnceLock<FxHashMap<Key, Vec<ChunkPos>>>,
    dimension_origin: OnceLock<ChunkPos>,
}

impl Generator {
    pub fn new(pack: &Datapack, spec: DimensionSpec, seed: u64) -> Result<Self, GeneratorError> {
        let total = Instant::now();
        let densities = DensityRegistry::load(pack)?;
        let materials = MaterialRegistry::load(pack, &densities)?;
        let settings = Arc::new(NoiseGeneratorSettings::load(
            pack,
            &densities,
            &materials,
            &spec.noise_settings,
        )?);
        let seed = seed as i64;
        let random_state = RandomState::create(&densities, seed, &settings);
        let dimension_type = dimension_type(pack, &spec.dimension_type)?;
        let (min_y, height) = (dimension_type.min_y, dimension_type.height);
        let biome_source = Arc::new(match spec.biome_source {
            BiomeSourceSpec::MultiNoise(parameters) => BiomeSource::multi_noise(parameters),
            BiomeSourceSpec::TheEnd => BiomeSource::the_end(),
            BiomeSourceSpec::Fixed(key) => BiomeSource::Fixed(
                registries::BIOME
                    .by_key(&key)
                    .ok_or(GeneratorError::UnknownBiome(key))?,
            ),
        });
        let kind = match &spec.generator {
            GeneratorSpec::Noise => GeneratorKind::Noise,
            GeneratorSpec::Debug => GeneratorKind::Debug,
            GeneratorSpec::Flat(flat) => {
                let biome = registries::BIOME
                    .by_key(&flat.biome)
                    .ok_or_else(|| GeneratorError::UnknownBiome(flat.biome.clone()))?;
                GeneratorKind::Flat(FlatGenerator::new(flat, biome))
            }
        };

        let possible_biomes = biome_source.possible_biomes();
        let possible_biome_ids = possible_biomes.iter().map(|biome| biome.id()).collect();
        let features_per_step = build_features_per_step(&possible_biomes, |biome| match &kind {
            GeneratorKind::Flat(flat) => flat.features_for_biome(biome),
            GeneratorKind::Noise | GeneratorKind::Debug => features().features_for_biome(biome),
        });
        let sea_level = match kind {
            GeneratorKind::Noise => settings.sea_level,
            GeneratorKind::Flat(_) => FLAT_SEA_LEVEL,
            GeneratorKind::Debug => DEBUG_SEA_LEVEL,
        };

        tracing::debug!(
            "generator for {} initialized in {:?}",
            spec.noise_settings,
            total.elapsed()
        );

        Ok(Self {
            chunk_generator: NoiseBasedChunkGenerator::new(biome_source, settings),
            kind,
            random_state,
            biome_zoom_seed: obfuscate_biome_seed(seed),
            seed,
            sea_level,
            min_y,
            height,
            features_per_step,
            possible_biome_ids,
            possible_biomes,
            possible_structure_sets: OnceLock::new(),
            structures_by_step: OnceLock::new(),
            ring_positions: OnceLock::new(),
            dimension_origin: OnceLock::new(),
        })
    }

    pub const fn random_state(&self) -> &RandomState {
        &self.random_state
    }

    pub fn biome_source(&self) -> &BiomeSource {
        self.chunk_generator.biome_source()
    }

    pub const fn chunk_generator(&self) -> &NoiseBasedChunkGenerator {
        &self.chunk_generator
    }

    pub const fn sea_level(&self) -> i32 {
        self.sea_level
    }

    pub const fn biome_zoom_seed(&self) -> i64 {
        self.biome_zoom_seed
    }

    pub fn generation_context(&self, level_min_y: i32, level_height: i32) -> WorldGenContext {
        match self.kind {
            GeneratorKind::Noise => self
                .chunk_generator
                .generation_context(level_min_y, level_height),
            GeneratorKind::Flat(_) | GeneratorKind::Debug => WorldGenContext::new(
                0,
                SIMPLE_GEN_DEPTH,
                self.sea_level,
                level_min_y,
                level_height,
            ),
        }
    }

    pub(crate) fn features_for_biome(&self, biome: BiomeRef) -> &[Vec<&'static PlacedFeature>] {
        match &self.kind {
            GeneratorKind::Flat(flat) => flat.features_for_biome(biome),
            GeneratorKind::Noise | GeneratorKind::Debug => features().features_for_biome(biome),
        }
    }

    pub(crate) const fn is_debug(&self) -> bool {
        matches!(self.kind, GeneratorKind::Debug)
    }

    pub(crate) fn structure_overrides(&self) -> Option<&[Key]> {
        match &self.kind {
            GeneratorKind::Flat(flat) => flat.structure_overrides(),
            GeneratorKind::Noise | GeneratorKind::Debug => None,
        }
    }

    pub(crate) const fn concentric_rings_seed(&self) -> i64 {
        match self.kind {
            GeneratorKind::Flat(_) => 0,
            GeneratorKind::Noise | GeneratorKind::Debug => self.seed,
        }
    }

    pub const fn seed(&self) -> i64 {
        self.seed
    }

    pub fn dimension_origin(&self) -> ChunkPos {
        *self.dimension_origin.get_or_init(|| {
            if !matches!(self.kind, GeneratorKind::Noise) {
                return ChunkPos::new(0, 0);
            }
            let spawn_target = &self.chunk_generator.generator_settings().spawn_target;
            if spawn_target.is_empty() {
                return ChunkPos::new(0, 0);
            }
            let pos = find_spawn_position(spawn_target, &self.random_state);
            ChunkPos::new(pos.x() >> 4, pos.z() >> 4)
        })
    }

    pub const fn min_y(&self) -> i32 {
        self.min_y
    }

    pub const fn height(&self) -> i32 {
        self.height
    }

    pub fn get_base_height(&self, x: i32, z: i32, heightmap: HeightmapType) -> i32 {
        match &self.kind {
            GeneratorKind::Flat(flat) => return flat.first_free_height(self.min_y, heightmap),
            GeneratorKind::Debug => return self.min_y,
            GeneratorKind::Noise => {}
        }
        self.chunk_generator.get_base_height(
            x,
            z,
            heightmap,
            self.min_y,
            self.height,
            &self.random_state,
        )
    }

    pub fn get_base_column(&self, x: i32, z: i32) -> NoiseColumn {
        match &self.kind {
            GeneratorKind::Flat(flat) => {
                let (min_y, column) = flat.base_column(self.min_y);
                return NoiseColumn { min_y, column };
            }
            GeneratorKind::Debug => {
                return NoiseColumn {
                    min_y: self.min_y,
                    column: Vec::new(),
                };
            }
            GeneratorKind::Noise => {}
        }
        self.chunk_generator
            .get_base_column(x, z, self.min_y, self.height, &self.random_state)
    }

    pub fn get_first_free_height(&self, x: i32, z: i32, heightmap: HeightmapType) -> i32 {
        self.get_base_height(x, z, heightmap)
    }

    pub fn get_first_occupied_height(&self, x: i32, z: i32, heightmap: HeightmapType) -> i32 {
        self.get_base_height(x, z, heightmap) - 1
    }

    pub fn region_random(&self, pos: ChunkPos) -> RandomSource {
        self.random_state
            .get_or_create_random_factory(WORLDGEN_REGION_RANDOM)
            .at(pos.0.x * 16, 0, pos.0.y * 16)
    }

    pub fn create_biomes(&self, chunk: &Chunk) {
        let pos = chunk.pos();
        let min_y = chunk.min_y();
        let mut resolver = self.biome_source().create_resolver_for_chunk(
            self.random_state
                .create_climate_sampler(SamplerContext::cached()),
            [pos.0.x * 4, min_y >> 2, pos.0.y * 4],
            [4, chunk.height() >> 2, 4],
        );
        for (section_index, section) in chunk.sections().sections.iter().enumerate() {
            let quart_min_y = (min_y >> 2) + section_index as i32 * 4;
            let mut section = section.write();
            for x in 0..4 {
                for y in 0..4 {
                    for z in 0..4 {
                        let biome = resolver.get_noise_biome(
                            pos.0.x * 4 + x,
                            quart_min_y + y,
                            pos.0.y * 4 + z,
                        );
                        section
                            .biomes_mut()
                            .set(x as usize, y as usize, z as usize, biome.id());
                    }
                }
            }
        }
    }

    pub fn build_terrain(
        &self,
        chunk: &Chunk,
        beardifier: &Beardifier,
        noise_biome: &dyn Fn(i32, i32, i32) -> BiomeRef,
        possible_biomes: &FxHashSet<u16>,
    ) {
        match &self.kind {
            GeneratorKind::Flat(flat) => {
                flat.build_terrain(chunk);
                chunk.prime_heightmaps(HeightmapType::final_types());
                return;
            }
            GeneratorKind::Debug => {
                chunk.prime_heightmaps(HeightmapType::worldgen_types());
                chunk.prime_heightmaps(HeightmapType::final_types());
                return;
            }
            GeneratorKind::Noise => {}
        }
        let noise_settings = self
            .chunk_generator
            .generator_settings()
            .noise_settings
            .clamp_to_height_accessor(chunk.min_y(), chunk.min_y() + chunk.height() - 1);
        if noise_settings.height > 0 {
            let mut noise_chunk = self.chunk_generator.create_noise_chunk(
                &self.random_state,
                beardifier,
                chunk.pos().0.x * 16,
                chunk.pos().0.y * 16,
                noise_settings,
            );
            self.chunk_generator
                .do_fill(&self.random_state, &mut noise_chunk, chunk);
            let biome_manager = BiomeManager::new(noise_biome, self.biome_zoom_seed);
            self.chunk_generator.build_surface(
                chunk,
                &mut noise_chunk,
                &self.random_state,
                &|pos: BlockPosition| biome_manager.get_biome(pos),
                Some(possible_biomes),
            );
            self.chunk_generator.generate_carvers(
                chunk,
                &mut noise_chunk,
                &self.random_state,
                self.biome_zoom_seed,
            );
        }
        chunk.prime_heightmaps(HeightmapType::final_types());
    }

    pub fn find_biome_horizontal(
        &self,
        origin_x: i32,
        origin_y: i32,
        origin_z: i32,
        search_radius: i32,
        allowed: &dyn Fn(BiomeRef) -> bool,
        random: &mut RandomSource,
    ) -> Option<(BlockPosition, BiomeRef)> {
        let resolver = RefCell::new(
            self.biome_source()
                .create_caching_resolver(&self.random_state),
        );
        let noise_center_x = origin_x >> 2;
        let noise_center_z = origin_z >> 2;
        let noise_radius = search_radius >> 2;
        let noise_y = origin_y >> 2;
        let mut result = None;
        let mut found = 0;
        for z in -noise_radius..=noise_radius {
            for x in -noise_radius..=noise_radius {
                let noise_x = noise_center_x + x;
                let noise_z = noise_center_z + z;
                let biome = resolver
                    .borrow_mut()
                    .get_noise_biome(noise_x, noise_y, noise_z);
                if allowed(biome) {
                    if result.is_none() || random.next_i32_bounded(found + 1) == 0 {
                        result = Some((
                            BlockPosition::new(noise_x << 2, origin_y, noise_z << 2),
                            biome,
                        ));
                    }
                    found += 1;
                }
            }
        }
        result
    }
}
