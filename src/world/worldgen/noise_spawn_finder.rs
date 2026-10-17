use std::f64::consts::TAU;

use shulkr::util::BlockPosition;

use crate::world::worldgen::biome::climate::quantize_coord;
use crate::world::worldgen::densityfunction::context::SamplerContext;
use crate::world::worldgen::densityfunction::eval::sample_value;
use crate::world::worldgen::noise_generator_settings::SpawnTargetPoint;
use crate::world::worldgen::random_state::RandomState;

pub fn find_spawn_position(
    target_points: &[SpawnTargetPoint],
    random_state: &RandomState,
) -> BlockPosition {
    let mut context = SamplerContext::cached();
    let mut finder = NoiseSpawnFinder {
        target_points,
        random_state,
        context: &mut context,
        result: Result {
            location: BlockPosition::ZERO,
            fitness: 0,
        },
    };
    finder.result = finder.get_spawn_position_and_fitness(0, 0);
    finder.radial_search(2048.0, 512.0);
    finder.radial_search(512.0, 32.0);
    finder.result.location
}

#[derive(Clone, Copy)]
struct Result {
    location: BlockPosition,
    fitness: i64,
}

struct NoiseSpawnFinder<'a, 'c> {
    target_points: &'a [SpawnTargetPoint],
    random_state: &'a RandomState,
    context: &'a mut SamplerContext<'c>,
    result: Result,
}

impl NoiseSpawnFinder<'_, '_> {
    fn radial_search(&mut self, max_radius: f32, radius_increment: f32) {
        let mut angle = 0.0f32;
        let mut radius = radius_increment;
        let search_origin = self.result.location;

        while radius <= max_radius {
            let x = search_origin.x() + (f64::from(angle).sin() * f64::from(radius)) as i32;
            let z = search_origin.z() + (f64::from(angle).cos() * f64::from(radius)) as i32;
            let candidate = self.get_spawn_position_and_fitness(x, z);
            if candidate.fitness < self.result.fitness {
                self.result = candidate;
            }

            angle += radius_increment / radius;
            if f64::from(angle) > TAU {
                angle = 0.0;
                radius += radius_increment;
            }
        }
    }

    fn get_spawn_position_and_fitness(&mut self, block_x: i32, block_z: i32) -> Result {
        let quart_block_x = (block_x >> 2) << 2;
        let quart_block_z = (block_z >> 2) << 2;
        let mut min_fitness = i64::MAX;

        for point in self.target_points {
            min_fitness =
                min_fitness.min(self.sample_fitness(point, quart_block_x, 0, quart_block_z));
        }

        let distance_bias_to_world_origin = i64::from(block_x)
            .wrapping_mul(i64::from(block_x))
            .wrapping_add(i64::from(block_z).wrapping_mul(i64::from(block_z)));
        let fitness_with_distance = min_fitness
            .wrapping_mul(2048 * 2048)
            .wrapping_add(distance_bias_to_world_origin);
        Result {
            location: BlockPosition::new(block_x, 0, block_z),
            fitness: fitness_with_distance,
        }
    }

    fn sample_fitness(
        &mut self,
        point: &SpawnTargetPoint,
        block_x: i32,
        block_y: i32,
        block_z: i32,
    ) -> i64 {
        let mut fitness = 0i64;
        for (function, parameter) in &point.parameters {
            let sampler = self.random_state.get_sampler(function);
            let value = sample_value(&sampler, self.context, block_x, block_y, block_z);
            let value = quantize_coord(f64::from(value));
            let distance = parameter.distance(value);
            fitness = fitness.wrapping_add(distance.wrapping_mul(distance));
        }
        fitness
    }
}
