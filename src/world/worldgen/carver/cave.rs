use std::f64::consts::PI;

use serde::Deserialize;

use crate::util::math::{cos, sin};
use crate::util::pos::ChunkPos;
use crate::util::rng::RandomSource;
use crate::util::rng::legacy_random::LegacyRandomSource;
use crate::util::value_providers::{FloatProvider, HeightProvider, IntProvider};
use crate::world::chunk::carving_mask::CarvingMask;
use crate::world::worldgen::worldgen_context::WorldGenContext;

use super::{WorldCarver, can_reach, carve_ellipsoid};

const fn default_start_vertical_radius_multiplier() -> FloatProvider {
    FloatProvider::Constant(1.0)
}

#[derive(Debug, Clone, Deserialize)]
pub struct CaveWorldCarver {
    pub probability: f32,
    pub y: HeightProvider,
    pub count: IntProvider,
    pub thickness: FloatProvider,
    #[serde(default)]
    pub weird_thickness_bias: bool,
    pub room_vertical_radius_multiplier: FloatProvider,
    pub horizontal_radius_multiplier: FloatProvider,
    pub vertical_radius_multiplier: FloatProvider,
    #[serde(default = "default_start_vertical_radius_multiplier")]
    pub start_vertical_radius_multiplier: FloatProvider,
    pub floor_level: FloatProvider,
}

fn should_skip(xd: f64, yd: f64, zd: f64, floor_level: f64) -> bool {
    if yd <= floor_level {
        true
    } else {
        xd * xd + yd * yd + zd * zd >= 1.0
    }
}

impl CaveWorldCarver {
    pub fn carve(
        &self,
        context: &WorldGenContext,
        random: &mut RandomSource,
        chunk_pos: ChunkPos,
        source_chunk_pos: ChunkPos,
        output: &mut CarvingMask,
    ) -> bool {
        let max_distance = (WorldCarver::get_range() * 2 - 1) << 4;
        let cave_count = self.count.sample(random);
        for _ in 0..cave_count {
            let x = f64::from(source_chunk_pos.0.x * 16 + random.next_i32_bounded(16));
            let y = f64::from(self.y.sample(random, context));
            let z = f64::from(source_chunk_pos.0.y * 16 + random.next_i32_bounded(16));
            let horizontal_radius_multiplier =
                f64::from(self.horizontal_radius_multiplier.sample(random));
            let vertical_radius_multiplier =
                f64::from(self.vertical_radius_multiplier.sample(random));
            let start_vertical_radius_multiplier =
                f64::from(self.start_vertical_radius_multiplier.sample(random));
            let floor_level = f64::from(self.floor_level.sample(random));
            let skip_checker = |xd, yd, zd, _world_y| should_skip(xd, yd, zd, floor_level);
            let mut tunnels = 1;
            if random.next_i32_bounded(4) == 0 {
                let y_scale = f64::from(self.room_vertical_radius_multiplier.sample(random));
                let thickness = 1.0 + random.next_f32() * 6.0;
                Self::create_room(
                    chunk_pos,
                    x,
                    y,
                    z,
                    thickness,
                    y_scale,
                    output,
                    &skip_checker,
                );
                tunnels += random.next_i32_bounded(4);
            }

            for _ in 0..tunnels {
                let horizontal_rotation = random.next_f32() * (PI * 2.0) as f32;
                let vertical_rotation = (random.next_f32() - 0.5) / 4.0;
                let thickness = self.get_thickness(random);
                let distance = max_distance - random.next_i32_bounded(max_distance / 4);
                let tunnel_seed = random.next_i64();
                Self::create_tunnel(
                    chunk_pos,
                    tunnel_seed,
                    x,
                    y,
                    z,
                    horizontal_radius_multiplier,
                    vertical_radius_multiplier,
                    thickness,
                    horizontal_rotation,
                    vertical_rotation,
                    0,
                    distance,
                    start_vertical_radius_multiplier,
                    output,
                    &skip_checker,
                );
            }
        }
        true
    }

    fn get_thickness(&self, random: &mut RandomSource) -> f32 {
        let mut thickness = self.thickness.sample(random);
        if self.weird_thickness_bias && random.next_i32_bounded(10) == 0 {
            thickness *= random.next_f32() * random.next_f32() * 3.0 + 1.0;
        }
        thickness
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors CaveWorldCarver.createRoom"
    )]
    fn create_room(
        chunk_pos: ChunkPos,
        x: f64,
        y: f64,
        z: f64,
        thickness: f32,
        y_scale: f64,
        output: &mut CarvingMask,
        skip_checker: &dyn Fn(f64, f64, f64, i32) -> bool,
    ) {
        let horizontal_radius = 1.5 + f64::from(sin(f64::from((PI / 2.0) as f32)) * thickness);
        let vertical_radius = horizontal_radius * y_scale;
        carve_ellipsoid(
            chunk_pos,
            x + 1.0,
            y,
            z,
            horizontal_radius,
            vertical_radius,
            output,
            skip_checker,
        );
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors CaveWorldCarver.createTunnel"
    )]
    fn create_tunnel(
        chunk_pos: ChunkPos,
        tunnel_seed: i64,
        mut x: f64,
        mut y: f64,
        mut z: f64,
        horizontal_radius_multiplier: f64,
        vertical_radius_multiplier: f64,
        thickness: f32,
        mut horizontal_rotation: f32,
        mut vertical_rotation: f32,
        step: i32,
        dist: i32,
        y_scale: f64,
        output: &mut CarvingMask,
        skip_checker: &dyn Fn(f64, f64, f64, i32) -> bool,
    ) {
        let mut random = RandomSource::Legacy(LegacyRandomSource::from_seed(tunnel_seed as u64));
        let split_point = random.next_i32_bounded(dist / 2) + dist / 4;
        let steep = random.next_i32_bounded(6) == 0;
        let mut y_rota = 0.0_f32;
        let mut x_rota = 0.0_f32;

        for current_step in step..dist {
            let horizontal_radius = 1.5
                + f64::from(
                    sin(f64::from(PI as f32 * current_step as f32 / dist as f32)) * thickness,
                );
            let vertical_radius = horizontal_radius * y_scale;
            let cos_x = cos(f64::from(vertical_rotation));
            x += f64::from(cos(f64::from(horizontal_rotation)) * cos_x);
            y += f64::from(sin(f64::from(vertical_rotation)));
            z += f64::from(sin(f64::from(horizontal_rotation)) * cos_x);
            vertical_rotation *= if steep { 0.92 } else { 0.7 };
            vertical_rotation += x_rota * 0.1;
            horizontal_rotation += y_rota * 0.1;
            x_rota *= 0.9;
            y_rota *= 0.75;
            x_rota += (random.next_f32() - random.next_f32()) * random.next_f32() * 2.0;
            y_rota += (random.next_f32() - random.next_f32()) * random.next_f32() * 4.0;
            if current_step == split_point && thickness > 1.0 {
                let seed = random.next_i64();
                let branch_thickness = random.next_f32() * 0.5 + 0.5;
                Self::create_tunnel(
                    chunk_pos,
                    seed,
                    x,
                    y,
                    z,
                    horizontal_radius_multiplier,
                    vertical_radius_multiplier,
                    branch_thickness,
                    horizontal_rotation - (PI / 2.0) as f32,
                    vertical_rotation / 3.0,
                    current_step,
                    dist,
                    1.0,
                    output,
                    skip_checker,
                );
                let seed = random.next_i64();
                let branch_thickness = random.next_f32() * 0.5 + 0.5;
                Self::create_tunnel(
                    chunk_pos,
                    seed,
                    x,
                    y,
                    z,
                    horizontal_radius_multiplier,
                    vertical_radius_multiplier,
                    branch_thickness,
                    horizontal_rotation + (PI / 2.0) as f32,
                    vertical_rotation / 3.0,
                    current_step,
                    dist,
                    1.0,
                    output,
                    skip_checker,
                );
                return;
            }

            if random.next_i32_bounded(4) != 0 {
                if !can_reach(chunk_pos, x, z, current_step, dist, thickness) {
                    return;
                }
                carve_ellipsoid(
                    chunk_pos,
                    x,
                    y,
                    z,
                    horizontal_radius * horizontal_radius_multiplier,
                    vertical_radius * vertical_radius_multiplier,
                    output,
                    skip_checker,
                );
            }
        }
    }
}
