use std::f64::consts::PI;
use std::sync::Arc;

use crate::util::pos::BlockPos;
use crate::worldgen::backend::jit::{RouterEntry, WellKnownRouters};
use crate::worldgen::biome::climate::{Parameter, ParameterPoint, TargetPoint, quantize_coord};
use crate::worldgen::density::{ColumnCache, Noises};

const MAX_SPAWN_SEARCH_RADIUS: f32 = 2048.0;

pub struct ClimateSampler {
    noises: Arc<Noises>,
    routers: WellKnownRouters,
}

impl ClimateSampler {
    pub fn new(noises: Arc<Noises>) -> Self {
        let routers = *noises.compiled().routers();
        Self { noises, routers }
    }

    pub fn noises(&self) -> &Noises {
        &self.noises
    }

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

        let sample = |entry: Option<RouterEntry>, name: &str| -> f32 {
            let entry = entry.unwrap_or_else(|| panic!("no router entry named {name}"));
            cache.router_at(entry, block_x, block_y, block_z, &self.noises) as f32
        };
        let temperature = sample(self.routers.temperature, "temperature");
        let humidity = sample(self.routers.vegetation, "vegetation");
        let continentalness = sample(self.routers.continentalness, "continentalness");
        let erosion = sample(self.routers.erosion, "erosion");
        let depth = sample(self.routers.depth, "depth");
        let weirdness = sample(self.routers.ridges, "ridges");

        TargetPoint::new(
            quantize_coord(f64::from(temperature)),
            quantize_coord(f64::from(humidity)),
            quantize_coord(f64::from(continentalness)),
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

    pub fn find_spawn_position(&self) -> BlockPos {
        let targets = overworld_spawn_targets();
        let mut best = self.spawn_candidate(&targets, 0, 0);
        best = self.radial_spawn_search(&targets, best, MAX_SPAWN_SEARCH_RADIUS, 512.0);
        self.radial_spawn_search(&targets, best, 512.0, 32.0).pos
    }

    fn radial_spawn_search(
        &self,
        targets: &[ParameterPoint; 2],
        mut best: SpawnCandidate,
        max_radius: f32,
        radius_step: f32,
    ) -> SpawnCandidate {
        let origin = best.pos;
        let mut angle = 0.0_f32;
        let mut radius = radius_step;

        while radius <= max_radius {
            let offset_x = (f64::from(angle).sin() * f64::from(radius)) as i32;
            let offset_z = (f64::from(angle).cos() * f64::from(radius)) as i32;
            let candidate =
                self.spawn_candidate(targets, origin.x() + offset_x, origin.z() + offset_z);
            if candidate.fitness < best.fitness {
                best = candidate;
            }

            angle += radius_step / radius;
            if f64::from(angle) > PI * 2.0 {
                angle = 0.0;
                radius += radius_step;
            }
        }

        best
    }

    fn spawn_candidate(
        &self,
        targets: &[ParameterPoint; 2],
        block_x: i32,
        block_z: i32,
    ) -> SpawnCandidate {
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
        let min_fitness = targets
            .iter()
            .map(|point| point.fitness(&zero_depth_target))
            .min()
            .unwrap_or(i64::MAX);
        let distance_bias =
            i64::from(block_x) * i64::from(block_x) + i64::from(block_z) * i64::from(block_z);

        SpawnCandidate {
            pos: BlockPos::new(block_x, 0, block_z),
            fitness: min_fitness * 2048_i64 * 2048_i64 + distance_bias,
        }
    }
}

#[derive(Clone, Copy)]
struct SpawnCandidate {
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
