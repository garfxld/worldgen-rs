use smallvec::SmallVec;

use crate::chunk::proto::{CarversPhase, GenerationChunk};
use crate::registry::REGISTRY;
use crate::registry::carver::ConfiguredCarverKind;
use crate::util::pos::{BlockPos, ChunkPos};
use crate::util::rng::legacy_random::LegacyRandomSource;
use crate::worldgen::carver::{
    CarveRun, CarverBlockIds, CarvingContext, PreliminarySurfaceCorners, SourceChunk, cave,
};
use crate::worldgen::density::{Aquifer, ColumnCache};
use crate::worldgen::generator::Generator;
use crate::worldgen::generator::fuzzed_biome::fuzzed_biome_at_block;

const CARVER_SOURCE_CHUNK_COUNT: usize = 17 * 17;

impl Generator {
    pub fn apply_carvers(&self, chunk: GenerationChunk<'_, CarversPhase>) {
        chunk.prime_world_surface_heightmap();

        let pos = chunk.pos();
        let chunk_min_x = pos.0.x * 16;
        let chunk_min_z = pos.0.y * 16;
        let noises = &self.noises;
        let min_y = noises.min_y();
        let height = noises.height();

        let mut column_cache = ColumnCache::new(noises);
        if self.aquifers_enabled {
            column_cache.init_grid(chunk_min_x, chunk_min_z, noises);
        }
        let mut aquifer = Aquifer::new(
            chunk_min_x,
            chunk_min_z,
            min_y,
            height,
            noises.aquifer_random(),
            self.aquifers_enabled,
            self.default_fluid_id,
            noises,
            column_cache,
        );

        let psl_corners = PreliminarySurfaceCorners {
            nw: aquifer.preliminary_surface_level(noises, chunk_min_x, chunk_min_z),
            ne: aquifer.preliminary_surface_level(noises, chunk_min_x + 16, chunk_min_z),
            sw: aquifer.preliminary_surface_level(noises, chunk_min_x, chunk_min_z + 16),
            se: aquifer.preliminary_surface_level(noises, chunk_min_x + 16, chunk_min_z + 16),
        };

        let mut ctx = CarvingContext {
            min_y,
            gen_depth: height,
            surface_system: &self.surface_system,
            surface_rules: &self.surface_rules,
            aquifer: &mut aquifer,
            default_block_id: self.default_block_id,
            psl_corners,
            chunk_min_x,
            chunk_min_z,
        };

        let ids = CarverBlockIds::load();

        let mut biome_sampler = self.biome_source.chunk_sampler();
        let mut source_biomes: SmallVec<[SourceChunk; CARVER_SOURCE_CHUNK_COUNT]> = SmallVec::new();
        for dx in -8i32..=8 {
            for dz in -8i32..=8 {
                let sx = pos.0.x + dx;
                let sz = pos.0.y + dz;
                let qx = (sx * 16) >> 2;
                let qz = (sz * 16) >> 2;
                let biome = biome_sampler.sample(qx, 0, qz);
                source_biomes.push(SourceChunk {
                    pos: ChunkPos::new(sx, sz),
                    biome,
                });
            }
        }

        let mut random = LegacyRandomSource::from_seed(0);
        let seed_i64 = self.seed;

        let biome_zoom_seed = self.biome_zoom_seed;
        let mut biome_getter = |pos: BlockPos| -> u16 {
            fuzzed_biome_at_block(biome_zoom_seed, pos, |q_pos| {
                biome_sampler.sample(q_pos.x, q_pos.y, q_pos.z).id() as u16
            })
        };

        chunk.with_carving_mask(|mask| {
            let mut run = CarveRun {
                ctx: &mut ctx,
                noises,
                chunk,
                chunk_min_x,
                chunk_min_z,
                biome_getter: &mut biome_getter,
                mask,
                ids,
            };

            run_all(&mut run, &source_biomes, seed_i64, &mut random);
        });
    }
}

fn run_all<F>(
    run: &mut CarveRun<'_, '_, F>,
    source_biomes: &[SourceChunk],
    seed_i64: i64,
    random: &mut LegacyRandomSource,
) where
    F: FnMut(BlockPos) -> u16,
{
    for source in source_biomes {
        for (index, carver_key) in source.biome.carvers.iter().enumerate() {
            let Some(carver) = REGISTRY.carvers.by_key(carver_key) else {
                panic!(
                    "biome {} references unknown configured carver {}",
                    source.biome.key, carver_key
                );
            };
            let index_i64 = index as i64;
            random.set_large_feature_seed(
                seed_i64.wrapping_add(index_i64),
                source.pos.0.x,
                source.pos.0.y,
            );

            let probability = carver.base().probability;
            if random.next_f32() > probability {
                continue;
            }

            match &carver.kind {
                ConfiguredCarverKind::Cave(cfg) => {
                    run.carve_cave(cfg, cave::CaveKind::Overworld, source.pos, random);
                }
                ConfiguredCarverKind::NetherCave(cfg) => {
                    run.carve_cave(cfg, cave::CaveKind::Nether, source.pos, random);
                }
                ConfiguredCarverKind::Canyon(cfg) => {
                    run.carve_canyon(cfg, source.pos, random);
                }
            }
        }
    }
}
