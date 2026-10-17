use std::f64::consts::PI;

use serde::Deserialize;

use crate::util::math::{cos, sin};
use crate::util::pos::ChunkPos;
use crate::util::rng::RandomSource;
use crate::util::rng::legacy_random::LegacyRandomSource;
use crate::util::value_providers::{FloatProvider, HeightProvider};
use crate::world::chunk::carving_mask::CarvingMask;
use crate::world::worldgen::worldgen_context::WorldGenContext;

use super::{WorldCarver, can_reach, carve_ellipsoid};

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Shape {
    pub distance_factor: FloatProvider,
    pub thickness: FloatProvider,
    pub width_smoothness: i32,
    pub horizontal_radius_factor: FloatProvider,
    pub vertical_radius_default_factor: f32,
    pub vertical_radius_center_factor: f32,
    pub y_scale: FloatProvider,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CanyonWorldCarver {
    pub probability: f32,
    pub y: HeightProvider,
    pub vertical_rotation: FloatProvider,
    pub shape: Shape,
}

impl CanyonWorldCarver {
    pub fn carve(
        &self,
        context: &WorldGenContext,
        random: &mut RandomSource,
        chunk_pos: ChunkPos,
        source_chunk_pos: ChunkPos,
        output: &mut CarvingMask,
    ) -> bool {
        let max_distance = (WorldCarver::get_range() * 2 - 1) * 16;
        let x = f64::from(source_chunk_pos.0.x * 16 + random.next_i32_bounded(16));
        let y = self.y.sample(random, context);
        let z = f64::from(source_chunk_pos.0.y * 16 + random.next_i32_bounded(16));
        let horizontal_rotation = random.next_f32() * (PI * 2.0) as f32;
        let vertical_rotation = self.vertical_rotation.sample(random);
        let y_scale = f64::from(self.shape.y_scale.sample(random));
        let thickness = self.shape.thickness.sample(random);
        let distance = (max_distance as f32 * self.shape.distance_factor.sample(random)) as i32;
        let tunnel_seed = random.next_i64();
        self.do_carve(
            context,
            chunk_pos,
            tunnel_seed,
            x,
            f64::from(y),
            z,
            thickness,
            horizontal_rotation,
            vertical_rotation,
            0,
            distance,
            y_scale,
            output,
        );
        true
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors CanyonWorldCarver.doCarve"
    )]
    fn do_carve(
        &self,
        context: &WorldGenContext,
        chunk_pos: ChunkPos,
        tunnel_seed: i64,
        mut x: f64,
        mut y: f64,
        mut z: f64,
        thickness: f32,
        mut horizontal_rotation: f32,
        mut vertical_rotation: f32,
        step: i32,
        distance: i32,
        y_scale: f64,
        output: &mut CarvingMask,
    ) {
        let mut random = RandomSource::Legacy(LegacyRandomSource::from_seed(tunnel_seed as u64));
        let width_factor_per_height = self.init_width_factors(context, &mut random);
        let mut y_rota = 0.0_f32;
        let mut x_rota = 0.0_f32;
        let min_gen_y = context.get_min_gen_y();
        let skip_checker = |xd: f64, yd: f64, zd: f64, y: i32| {
            let y_index = y - min_gen_y;
            (xd * xd + zd * zd) * f64::from(width_factor_per_height[(y_index - 1) as usize])
                + yd * yd / 6.0
                >= 1.0
        };

        for current_step in step..distance {
            let mut horizontal_radius = 1.5
                + f64::from(
                    sin(f64::from(current_step as f32 * PI as f32 / distance as f32)) * thickness,
                );
            let mut vertical_radius = horizontal_radius * y_scale;
            horizontal_radius *= f64::from(self.shape.horizontal_radius_factor.sample(&mut random));
            vertical_radius = self.update_vertical_radius(
                &mut random,
                vertical_radius,
                distance as f32,
                current_step as f32,
            );
            let xc = cos(f64::from(vertical_rotation));
            let xs = sin(f64::from(vertical_rotation));
            x += f64::from(cos(f64::from(horizontal_rotation)) * xc);
            y += f64::from(xs);
            z += f64::from(sin(f64::from(horizontal_rotation)) * xc);
            vertical_rotation *= 0.7;
            vertical_rotation += x_rota * 0.05;
            horizontal_rotation += y_rota * 0.05;
            x_rota *= 0.8;
            y_rota *= 0.5;
            x_rota += (random.next_f32() - random.next_f32()) * random.next_f32() * 2.0;
            y_rota += (random.next_f32() - random.next_f32()) * random.next_f32() * 4.0;
            if random.next_i32_bounded(4) != 0 {
                if !can_reach(chunk_pos, x, z, current_step, distance, thickness) {
                    return;
                }
                carve_ellipsoid(
                    chunk_pos,
                    x,
                    y,
                    z,
                    horizontal_radius,
                    vertical_radius,
                    output,
                    &skip_checker,
                );
            }
        }
    }

    fn init_width_factors(&self, context: &WorldGenContext, random: &mut RandomSource) -> Vec<f32> {
        let depth = context.get_gen_depth();
        let mut width_factor_per_height = vec![0.0_f32; depth as usize];
        let mut width_factor = 1.0_f32;
        for y_index in 0..depth {
            if y_index == 0 || random.next_i32_bounded(self.shape.width_smoothness) == 0 {
                width_factor = 1.0 + random.next_f32() * random.next_f32();
            }
            width_factor_per_height[y_index as usize] = width_factor * width_factor;
        }
        width_factor_per_height
    }

    fn update_vertical_radius(
        &self,
        random: &mut RandomSource,
        vertical_radius: f64,
        distance: f32,
        current_step: f32,
    ) -> f64 {
        let vertical_multiplier = 1.0 - (0.5 - current_step / distance).abs() * 2.0;
        let factor = self.shape.vertical_radius_default_factor
            + self.shape.vertical_radius_center_factor * vertical_multiplier;
        let random_between = random.next_f32() * (1.0 - 0.75) + 0.75;
        f64::from(factor) * vertical_radius * f64::from(random_between)
    }
}
