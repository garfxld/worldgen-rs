use std::f32::consts::TAU;

use crate::util::pos::BlockPos;
use crate::worldgen::biome::climate::{Parameter, ParameterPoint, TargetPoint, quantize_coord};
use crate::worldgen::density::{ColumnCache, Noises};

pub struct ClimateSampler {
    noises: Noises,
}

impl ClimateSampler {
    #[must_use]
    pub const fn new(noises: Noises) -> Self {
        Self { noises }
    }

    #[must_use]
    pub const fn noises(&self) -> &Noises {
        &self.noises
    }

    #[must_use]
    pub fn sample(
        &self,
        quart_x: i32,
        quart_y: i32,
        quart_z: i32,
        cache: &mut ColumnCache,
    ) -> TargetPoint {
        let block_x = quart_x << 2;
        let block_y = quart_y << 2;
        let block_z = quart_z << 2;

        cache.ensure(block_x, block_z, &self.noises);

        let temp = cache.router("temperature", block_x, block_y, block_z, &self.noises) as f32;
        let humidity = cache.router("vegetation", block_x, block_y, block_z, &self.noises) as f32;
        let cont = cache.router("continentalness", block_x, block_y, block_z, &self.noises) as f32;
        let erosion = cache.router("erosion", block_x, block_y, block_z, &self.noises) as f32;
        let depth = cache.router("depth", block_x, block_y, block_z, &self.noises) as f32;
        let weirdness = cache.router("ridges", block_x, block_y, block_z, &self.noises) as f32;

        TargetPoint::new(
            quantize_coord(f64::from(temp)),
            quantize_coord(f64::from(humidity)),
            quantize_coord(f64::from(cont)),
            quantize_coord(f64::from(erosion)),
            quantize_coord(f64::from(depth)),
            quantize_coord(f64::from(weirdness)),
        )
    }

    pub fn init_column_grid(
        &self,
        cache: &mut ColumnCache,
        chunk_block_x: i32,
        chunk_block_z: i32,
    ) {
        cache.init_grid(chunk_block_x, chunk_block_z, &self.noises);
    }

    #[must_use]
    pub fn find_spawn_position(&self) -> BlockPos {
        let spawn_targets = overworld_spawn_targets();
        let mut result = self.spawn_position_and_fitness(&spawn_targets, 0, 0);
        result = self.radial_spawn_search(&spawn_targets, result, 2048.0, 512.0);
        self.radial_spawn_search(&spawn_targets, result, 512.0, 32.0)
            .pos
    }

    fn radial_spawn_search(
        &self,
        spawn_targets: &[ParameterPoint; 2],
        mut result: SpawnSearchResult,
        max_radius: f32,
        radius_increment: f32,
    ) -> SpawnSearchResult {
        let mut angle = 0.0_f32;
        let mut radius = radius_increment;
        let origin = result.pos;

        while radius <= max_radius {
            let x = origin.x() + (angle.sin() * radius) as i32;
            let z = origin.z() + (angle.cos() * radius) as i32;
            let candidate = self.spawn_position_and_fitness(spawn_targets, x, z);
            if candidate.fitness < result.fitness {
                result = candidate;
            }

            angle += radius_increment / radius;
            if angle > TAU {
                angle = 0.0;
                radius += radius_increment;
            }
        }

        result
    }

    fn spawn_position_and_fitness(
        &self,
        spawn_targets: &[ParameterPoint; 2],
        block_x: i32,
        block_z: i32,
    ) -> SpawnSearchResult {
        let mut cache = ColumnCache::new(&self.noises);
        let target = self.sample(block_x >> 2, 0, block_z >> 2, &mut cache);
        let zero_depth_target = TargetPoint::new(
            target.temperature,
            target.humidity,
            target.continentalness,
            target.erosion,
            0,
            target.weirdness,
        );
        let min_fitness = spawn_targets
            .iter()
            .map(|point| point.fitness(&zero_depth_target))
            .min()
            .unwrap_or(i64::MAX);
        let distance_bias =
            i64::from(block_x) * i64::from(block_x) + i64::from(block_z) * i64::from(block_z);

        SpawnSearchResult {
            pos: BlockPos::new(block_x, 0, block_z),
            fitness: min_fitness * 2048_i64 * 2048_i64 + distance_bias,
        }
    }
}

#[derive(Clone, Copy)]
struct SpawnSearchResult {
    pos: BlockPos,
    fitness: i64,
}

fn overworld_spawn_targets() -> [ParameterPoint; 2] {
    let full_range = Parameter::span(-1.0, 1.0);
    let inland_continentalness = Parameter::span(-0.11, 0.55);
    let continentalness = Parameter::span_params(&inland_continentalness, &full_range);
    let surface_depth = Parameter::point(0.0);

    [
        ParameterPoint::new(
            full_range,
            full_range,
            continentalness,
            full_range,
            surface_depth,
            Parameter::span(-1.0, -0.16),
            0,
        ),
        ParameterPoint::new(
            full_range,
            full_range,
            continentalness,
            full_range,
            surface_depth,
            Parameter::span(0.16, 1.0),
            0,
        ),
    ]
}
