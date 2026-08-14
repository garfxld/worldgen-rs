use std::f32::consts::{PI, TAU};

use crate::registry::carver::CanyonCarverConfig;
use crate::util::math::{cos, sin};
use crate::util::pos::{BlockPos, ChunkPos};
use crate::util::rng::legacy_random::LegacyRandomSource;

use crate::worldgen::carver::{
    CarveParams, CarveRun, CarverStyle, cached_replaceable_states, can_reach,
    horizontal_tunnel_radius,
};

const CARVER_RANGE: i32 = 4;
const MAX_TUNNEL_DISTANCE: i32 = (CARVER_RANGE * 2 - 1) * 16;

#[derive(Debug, Clone, Copy)]
struct CanyonState {
    x: f64,
    y: f64,
    z: f64,
    horizontal_rotation: f32,
    vertical_rotation: f32,
}

#[derive(Debug, Clone, Copy)]
struct CanyonTunnel {
    tunnel_seed: i64,
    thickness: f32,
    distance: i32,
    y_scale: f64,
}

impl<F> CarveRun<'_, '_, F>
where
    F: FnMut(BlockPos) -> u16,
{
    pub fn carve_canyon(
        &mut self,
        config: &CanyonCarverConfig,
        source_pos: ChunkPos,
        random: &mut LegacyRandomSource,
    ) {
        let source_min_x = source_pos.0.x * 16;
        let source_min_z = source_pos.0.y * 16;

        let lava_level_y = config
            .base
            .lava_level
            .resolve_y(self.ctx.min_y, self.ctx.gen_depth);
        let params = CarveParams {
            replaceable_tag: &config.base.replaceable_tag,
            replaceable_states: cached_replaceable_states(&config.base.replaceable_tag),
            lava_level_y,
            style: CarverStyle::Overworld,
        };

        let state = CanyonState {
            x: f64::from(source_min_x + random.next_i32_bounded(16)),
            y: f64::from(super::with_random_source(random, |r| {
                config.base.y.sample(r, self.ctx.min_y, self.ctx.gen_depth)
            })),
            z: f64::from(source_min_z + random.next_i32_bounded(16)),
            horizontal_rotation: random.next_f32() * TAU,
            vertical_rotation: super::with_random_source(random, |r| {
                config.vertical_rotation.sample(r)
            }),
        };

        let y_scale = f64::from(super::with_random_source(random, |r| {
            config.base.y_scale.sample(r)
        }));
        let thickness = super::with_random_source(random, |r| config.shape.thickness.sample(r));
        let distance = (MAX_TUNNEL_DISTANCE as f32
            * super::with_random_source(random, |r| config.shape.distance_factor.sample(r)))
            as i32;
        let tunnel_seed = random.next_i64();

        let tunnel = CanyonTunnel {
            tunnel_seed,
            thickness,
            distance,
            y_scale,
        };

        self.do_carve_canyon(&params, config, state, tunnel);
    }

    fn do_carve_canyon(
        &mut self,
        params: &CarveParams<'_>,
        config: &CanyonCarverConfig,
        mut state: CanyonState,
        tunnel: CanyonTunnel,
    ) {
        let mut random = LegacyRandomSource::from_seed(tunnel.tunnel_seed as u64);
        let width_factors = config
            .shape
            .init_width_factors(self.ctx.gen_depth, &mut random);
        let mut y_rota: f32 = 0.0;
        let mut x_rota: f32 = 0.0;

        for current_step in 0..tunnel.distance {
            let progress = PI * current_step as f32 / tunnel.distance as f32;
            let mut horizontal_radius = horizontal_tunnel_radius(progress, tunnel.thickness);
            let mut vertical_radius = horizontal_radius * tunnel.y_scale;
            horizontal_radius *= f64::from(super::with_random_source(&mut random, |r| {
                config.shape.horizontal_radius_factor.sample(r)
            }));
            vertical_radius = config.shape.update_vertical_radius(
                &mut random,
                vertical_radius,
                tunnel.distance as f32,
                current_step as f32,
            );

            let xc = cos(f64::from(state.vertical_rotation));
            let xs = sin(f64::from(state.vertical_rotation));
            state.x += f64::from(cos(f64::from(state.horizontal_rotation)) * xc);
            state.y += f64::from(xs);
            state.z += f64::from(sin(f64::from(state.horizontal_rotation)) * xc);
            state.vertical_rotation *= 0.7;
            state.vertical_rotation += x_rota * 0.05;
            state.horizontal_rotation += y_rota * 0.05;
            x_rota *= 0.8;
            y_rota *= 0.5;
            x_rota += (random.next_f32() - random.next_f32()) * random.next_f32() * 2.0;
            y_rota += (random.next_f32() - random.next_f32()) * random.next_f32() * 4.0;

            if random.next_i32_bounded(4) == 0 {
                continue;
            }

            if !can_reach(
                self.chunk_min_x,
                self.chunk_min_z,
                state.x,
                state.z,
                current_step,
                tunnel.distance,
                tunnel.thickness,
            ) {
                return;
            }

            let min_y = self.ctx.min_y;
            let skip_checker = |xd: f64, yd: f64, zd: f64, world_y: i32| {
                should_skip_canyon(&width_factors, min_y, xd, yd, zd, world_y)
            };

            self.carve_ellipsoid(
                params,
                state.x,
                state.y,
                state.z,
                horizontal_radius,
                vertical_radius,
                skip_checker,
            );
        }
    }
}

fn should_skip_canyon(
    width_factors: &[f32],
    min_y: i32,
    xd: f64,
    yd: f64,
    zd: f64,
    world_y: i32,
) -> bool {
    let y_index = (world_y - min_y - 1) as usize;
    let factor = width_factors[y_index];
    (xd * xd + zd * zd) * f64::from(factor) + yd * yd / 6.0 >= 1.0
}
