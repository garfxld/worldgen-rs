use std::f32::consts::{FRAC_PI_2, PI, TAU};

use crate::registry::carver::CaveCarverConfig;
use crate::util::math::{cos, sin};
use crate::util::pos::{BlockPos, ChunkPos};
use crate::util::rng::legacy_random::LegacyRandomSource;
use crate::util::value_providers::{FloatProvider, HeightProvider};

use crate::worldgen::carver::{
    CarveParams, CarveRun, CarveSkipChecker, CarverStyle, cached_replaceable_states, can_reach,
    horizontal_tunnel_radius,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaveKind {
    Overworld,
    Nether,
}

impl CaveKind {
    const fn cave_bound(self) -> i32 {
        match self {
            Self::Overworld => 15,
            Self::Nether => 10,
        }
    }

    const fn y_scale(self) -> f64 {
        match self {
            Self::Overworld => 1.0,
            Self::Nether => 5.0,
        }
    }

    const fn style(self) -> CarverStyle {
        match self {
            Self::Overworld => CarverStyle::Overworld,
            Self::Nether => CarverStyle::Nether,
        }
    }

    fn thickness(self, random: &mut LegacyRandomSource) -> f32 {
        match self {
            Self::Overworld => {
                let mut thickness = random.next_f32() * 2.0 + random.next_f32();
                if random.next_i32_bounded(10) == 0 {
                    thickness *= random.next_f32() * random.next_f32() * 3.0 + 1.0;
                }
                thickness
            }
            Self::Nether => (random.next_f32() * 2.0 + random.next_f32()) * 2.0,
        }
    }
}

const CARVER_RANGE: i32 = 4;
const MAX_TUNNEL_DISTANCE: i32 = (CARVER_RANGE * 2 - 1) * 16;

#[derive(Debug, Clone, Copy)]
struct TunnelState {
    x: f64,
    y: f64,
    z: f64,
    horizontal_rotation: f32,
    vertical_rotation: f32,
}

#[derive(Debug, Clone, Copy)]
struct TunnelParams {
    tunnel_seed: i64,
    horizontal_radius_multiplier: f64,
    vertical_radius_multiplier: f64,
    thickness: f32,
    step: i32,
    dist: i32,
    y_scale: f64,
}

impl<F> CarveRun<'_, '_, F>
where
    F: FnMut(BlockPos) -> u16,
{
    pub fn carve_cave(
        &mut self,
        config: &CaveCarverConfig,
        kind: CaveKind,
        source_pos: ChunkPos,
        random: &mut LegacyRandomSource,
    ) {
        let bound = kind.cave_bound();
        let inner = random.next_i32_bounded(bound);
        let mid = random.next_i32_bounded(inner + 1);
        let cave_count = random.next_i32_bounded(mid + 1);

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
            style: kind.style(),
        };

        for _ in 0..cave_count {
            let x = f64::from(source_min_x + random.next_i32_bounded(16));
            let y = f64::from(sample_height(
                config.base.y,
                random,
                self.ctx.min_y,
                self.ctx.gen_depth,
            ));
            let z = f64::from(source_min_z + random.next_i32_bounded(16));

            let horizontal_radius_multiplier =
                f64::from(sample_float(config.horizontal_radius_multiplier, random));
            let vertical_radius_multiplier =
                f64::from(sample_float(config.vertical_radius_multiplier, random));
            let floor_level = f64::from(sample_float(config.floor_level, random));

            let skip_checker = move |xd: f64, yd: f64, zd: f64, _world_y: i32| {
                yd <= floor_level || xd * xd + yd * yd + zd * zd >= 1.0
            };

            let mut tunnels = 1i32;
            if random.next_i32_bounded(4) == 0 {
                let y_scale = f64::from(sample_float(config.base.y_scale, random));
                let thickness = 1.0 + random.next_f32() * 6.0;
                self.create_room(&params, x, y, z, thickness, y_scale, &skip_checker);
                tunnels += random.next_i32_bounded(4);
            }

            for _ in 0..tunnels {
                let state = TunnelState {
                    x,
                    y,
                    z,
                    horizontal_rotation: random.next_f32() * TAU,
                    vertical_rotation: (random.next_f32() - 0.5) / 4.0,
                };
                let tunnel = TunnelParams {
                    tunnel_seed: 0,
                    horizontal_radius_multiplier,
                    vertical_radius_multiplier,
                    thickness: kind.thickness(random),
                    step: 0,
                    dist: MAX_TUNNEL_DISTANCE - random.next_i32_bounded(MAX_TUNNEL_DISTANCE / 4),
                    y_scale: kind.y_scale(),
                };
                let tunnel = TunnelParams {
                    tunnel_seed: random.next_i64(),
                    ..tunnel
                };
                self.create_tunnel(&params, state, tunnel, skip_checker);
            }
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors vanilla CaveWorldCarver.createRoom"
    )]
    fn create_room<S: CarveSkipChecker>(
        &mut self,
        params: &CarveParams<'_>,
        x: f64,
        y: f64,
        z: f64,
        thickness: f32,
        y_scale: f64,
        skip_checker: S,
    ) {
        let horizontal_radius = 1.5 + f64::from(sin(FRAC_PI_2)) * f64::from(thickness);
        let vertical_radius = horizontal_radius * y_scale;
        self.carve_ellipsoid(
            params,
            x + 1.0,
            y,
            z,
            horizontal_radius,
            vertical_radius,
            skip_checker,
        );
    }

    fn create_tunnel<S>(
        &mut self,
        params: &CarveParams<'_>,
        mut state: TunnelState,
        tunnel: TunnelParams,
        skip_checker: S,
    ) where
        S: CarveSkipChecker + Copy,
    {
        let mut random = LegacyRandomSource::from_seed(tunnel.tunnel_seed as u64);
        let split_point = random.next_i32_bounded(tunnel.dist / 2) + tunnel.dist / 4;
        let steep = random.next_i32_bounded(6) == 0;
        let mut y_rota: f32 = 0.0;
        let mut x_rota: f32 = 0.0;

        for current_step in tunnel.step..tunnel.dist {
            let progress_arg = PI * current_step as f32 / tunnel.dist as f32;
            let horizontal_radius = horizontal_tunnel_radius(progress_arg, tunnel.thickness);
            let vertical_radius = horizontal_radius * tunnel.y_scale;
            let cos_x = cos(state.vertical_rotation);
            state.x += f64::from(cos(state.horizontal_rotation) * cos_x);
            state.y += f64::from(sin(state.vertical_rotation));
            state.z += f64::from(sin(state.horizontal_rotation) * cos_x);
            state.vertical_rotation *= if steep { 0.92 } else { 0.7 };
            state.vertical_rotation += x_rota * 0.1;
            state.horizontal_rotation += y_rota * 0.1;
            x_rota *= 0.9;
            y_rota *= 0.75;
            x_rota += (random.next_f32() - random.next_f32()) * random.next_f32() * 2.0;
            y_rota += (random.next_f32() - random.next_f32()) * random.next_f32() * 4.0;

            if current_step == split_point && tunnel.thickness > 1.0 {
                let sub_seed_a = random.next_i64();
                let sub_thickness_a = random.next_f32() * 0.5 + 0.5;
                let sub_state_a = TunnelState {
                    horizontal_rotation: state.horizontal_rotation - FRAC_PI_2,
                    vertical_rotation: state.vertical_rotation / 3.0,
                    ..state
                };
                let sub_seed_b = random.next_i64();
                let sub_thickness_b = random.next_f32() * 0.5 + 0.5;
                let sub_state_b = TunnelState {
                    horizontal_rotation: state.horizontal_rotation + FRAC_PI_2,
                    vertical_rotation: state.vertical_rotation / 3.0,
                    ..state
                };
                let sub_tunnel_a = TunnelParams {
                    tunnel_seed: sub_seed_a,
                    thickness: sub_thickness_a,
                    step: current_step,
                    y_scale: 1.0,
                    ..tunnel
                };
                let sub_tunnel_b = TunnelParams {
                    tunnel_seed: sub_seed_b,
                    thickness: sub_thickness_b,
                    step: current_step,
                    y_scale: 1.0,
                    ..tunnel
                };
                self.create_tunnel(params, sub_state_a, sub_tunnel_a, skip_checker);
                self.create_tunnel(params, sub_state_b, sub_tunnel_b, skip_checker);
                return;
            }

            if random.next_i32_bounded(4) == 0 {
                continue;
            }

            if !can_reach(
                self.chunk_min_x,
                self.chunk_min_z,
                state.x,
                state.z,
                current_step,
                tunnel.dist,
                tunnel.thickness,
            ) {
                return;
            }

            self.carve_ellipsoid(
                params,
                state.x,
                state.y,
                state.z,
                horizontal_radius * tunnel.horizontal_radius_multiplier,
                vertical_radius * tunnel.vertical_radius_multiplier,
                skip_checker,
            );
        }
    }
}

fn sample_height(
    provider: HeightProvider,
    random: &mut LegacyRandomSource,
    min_y: i32,
    height: i32,
) -> i32 {
    super::with_random_source(random, |r| provider.sample(r, min_y, height))
}

fn sample_float(provider: FloatProvider, random: &mut LegacyRandomSource) -> f32 {
    super::with_random_source(random, |r| provider.sample(r))
}
