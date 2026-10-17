use crate::registry::biome::BiomeRef;
use crate::world::worldgen::biome::climate::{Sampler, TargetPoint};
use crate::world::worldgen::densityfunction::context::SamplerContext;
use crate::world::worldgen::densityfunction::volume::DensityVolume;
use crate::world::worldgen::random_state::RandomState;

use super::{BiomeSource, distinct_biome_refs};

struct ChunkClimate {
    volume: DensityVolume,
    min_quart: [i32; 3],
    temperature: Vec<f32>,
    vegetation: Vec<f32>,
    continents: Vec<f32>,
    erosion: Vec<f32>,
    depth: Vec<f32>,
    ridges: Vec<f32>,
}

pub struct BiomeResolver<'a> {
    source: &'a BiomeSource,
    sampler: Sampler,
    chunk: Option<ChunkClimate>,
    last_result: Option<usize>,
}

impl BiomeSource {
    pub const fn create_resolver(&self, sampler: Sampler) -> BiomeResolver<'_> {
        BiomeResolver {
            source: self,
            sampler,
            chunk: None,
            last_result: None,
        }
    }

    pub fn create_uncached_resolver(&self, random_state: &RandomState) -> BiomeResolver<'_> {
        self.create_resolver(random_state.create_climate_sampler(SamplerContext::uncached()))
    }

    pub fn create_caching_resolver(&self, random_state: &RandomState) -> BiomeResolver<'_> {
        self.create_resolver(random_state.create_climate_sampler(SamplerContext::cached()))
    }

    pub fn create_resolver_for_chunk(
        &self,
        sampler: Sampler,
        min_quart: [i32; 3],
        quart_size: [i32; 3],
    ) -> BiomeResolver<'_> {
        if !self.is_multi_noise() {
            return self.create_resolver(sampler);
        }
        let volume = DensityVolume::stepped(quart_size, min_quart.map(|q| q << 2), [4, 4, 4]);
        let temperature = sampler.sample_volume(&sampler.temperature, &volume);
        let vegetation = sampler.sample_volume(&sampler.humidity, &volume);
        let continents = sampler.sample_volume(&sampler.continentalness, &volume);
        let erosion = sampler.sample_volume(&sampler.erosion, &volume);
        let depth = sampler.sample_volume(&sampler.depth, &volume);
        let ridges = sampler.sample_volume(&sampler.weirdness, &volume);
        BiomeResolver {
            source: self,
            sampler,
            chunk: Some(ChunkClimate {
                volume,
                min_quart,
                temperature,
                vegetation,
                continents,
                erosion,
                depth,
                ridges,
            }),
            last_result: None,
        }
    }
}

impl BiomeResolver<'_> {
    pub fn get_noise_biome(&mut self, quart_x: i32, quart_y: i32, quart_z: i32) -> BiomeRef {
        if let Some(chunk) = &self.chunk {
            let index = chunk.volume.index_unchecked(
                quart_x - chunk.min_quart[0],
                quart_y - chunk.min_quart[1],
                quart_z - chunk.min_quart[2],
            );
            let target = TargetPoint::from_floats(
                f64::from(chunk.temperature[index]),
                f64::from(chunk.vegetation[index]),
                f64::from(chunk.continents[index]),
                f64::from(chunk.erosion[index]),
                f64::from(chunk.depth[index]),
                f64::from(chunk.ridges[index]),
            );
            return self
                .source
                .multi_noise_biome(&target, &mut self.last_result);
        }
        self.source.get_noise_biome(
            &self.sampler,
            &mut self.last_result,
            quart_x,
            quart_y,
            quart_z,
        )
    }

    pub fn get_biomes_within(&mut self, x: i32, y: i32, z: i32, radius: i32) -> Vec<BiomeRef> {
        let x0 = (x - radius) >> 2;
        let y0 = (y - radius) >> 2;
        let z0 = (z - radius) >> 2;
        let x1 = (x + radius) >> 2;
        let y1 = (y + radius) >> 2;
        let z1 = (z + radius) >> 2;
        let mut biomes = Vec::new();
        for noise_z in z0..=z1 {
            for noise_x in x0..=x1 {
                for noise_y in y0..=y1 {
                    biomes.push(self.get_noise_biome(noise_x, noise_y, noise_z));
                }
            }
        }
        distinct_biome_refs(biomes)
    }
}
