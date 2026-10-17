use crate::util::interval::Interval;
use crate::util::rng::RandomSource;
use crate::world::worldgen::densityfunction::volume::DensityVolume;

use super::gradient_noise::{GradientNoise, floor, grad_dot, lerp3, smoothstep, wrap};

pub const STANDARD_DEVIATION: f64 = 0.270_224_783_124_521_1;

#[derive(Debug, Clone)]
pub struct PerlinNoise {
    pub base: GradientNoise,
}

impl PerlinNoise {
    pub fn new(random: &mut RandomSource) -> Self {
        Self {
            base: GradientNoise::new(random, 256.0),
        }
    }

    pub fn range() -> Interval {
        Interval::of_symmetric(2.0)
    }

    pub fn get2(&self, x: f64, y: f64) -> f32 {
        self.get(wrap(x), 0.0, wrap(y))
    }

    pub fn get(&self, x: f64, y: f64, z: f64) -> f32 {
        let x = wrap(x) + self.base.offset_x;
        let y = wrap(y) + self.base.offset_y;
        let z = wrap(z) + self.base.offset_z;
        let floor_x = floor(x);
        let floor_y = floor(y);
        let floor_z = floor(z);
        let relative_x = (x - f64::from(floor_x)) as f32;
        let relative_y = (y - f64::from(floor_y)) as f32;
        let relative_z = (z - f64::from(floor_z)) as f32;
        self.sample_and_lerp(
            floor_x, floor_y, floor_z, relative_x, relative_y, relative_z, relative_y,
        )
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors PerlinNoise.sampleAndLerp"
    )]
    pub fn sample_and_lerp(
        &self,
        x: i32,
        y: i32,
        z: i32,
        relative_x: f32,
        relative_y: f32,
        relative_z: f32,
        original_relative_y: f32,
    ) -> f32 {
        let p = &self.base;
        let x0 = p.permute(x);
        let x1 = p.permute(x.wrapping_add(1));
        let xy00 = p.permute(x0 + y);
        let xy01 = p.permute(x0 + y + 1);
        let xy10 = p.permute(x1 + y);
        let xy11 = p.permute(x1 + y + 1);
        let d000 = grad_dot(p.permute(xy00 + z), relative_x, relative_y, relative_z);
        let d100 = grad_dot(
            p.permute(xy10 + z),
            relative_x - 1.0,
            relative_y,
            relative_z,
        );
        let d010 = grad_dot(
            p.permute(xy01 + z),
            relative_x,
            relative_y - 1.0,
            relative_z,
        );
        let d110 = grad_dot(
            p.permute(xy11 + z),
            relative_x - 1.0,
            relative_y - 1.0,
            relative_z,
        );
        let d001 = grad_dot(
            p.permute(xy00 + z + 1),
            relative_x,
            relative_y,
            relative_z - 1.0,
        );
        let d101 = grad_dot(
            p.permute(xy10 + z + 1),
            relative_x - 1.0,
            relative_y,
            relative_z - 1.0,
        );
        let d011 = grad_dot(
            p.permute(xy01 + z + 1),
            relative_x,
            relative_y - 1.0,
            relative_z - 1.0,
        );
        let d111 = grad_dot(
            p.permute(xy11 + z + 1),
            relative_x - 1.0,
            relative_y - 1.0,
            relative_z - 1.0,
        );
        let x_alpha = smoothstep(relative_x);
        let y_alpha = smoothstep(original_relative_y);
        let z_alpha = smoothstep(relative_z);
        lerp3(
            x_alpha, y_alpha, z_alpha, d000, d100, d010, d110, d001, d101, d011, d111,
        )
    }

    pub fn add_to_volume(
        &self,
        buffer: &mut [f32],
        volume: &DensityVolume,
        xz_scale: f64,
        y_scale: f64,
        amplitude: f32,
    ) {
        add_to_volume(
            &self.base,
            buffer,
            volume,
            xz_scale,
            y_scale,
            amplitude,
            |_, rel| rel as f32,
        );
    }
}

pub(super) fn add_to_volume(
    p: &GradientNoise,
    buffer: &mut [f32],
    volume: &DensityVolume,
    xz_scale: f64,
    y_scale: f64,
    amplitude: f32,
    fudged_relative_y: impl Fn(f64, f64) -> f32,
) {
    let mut d = [0.0f32; 8];
    let mut g = [0.0f32; 8];
    let mut index = 0;

    for index_z in 0..volume.size_z {
        let z = wrap(f64::from(volume.block_z(index_z)) * xz_scale) + p.offset_z;
        let floor_z = floor(z);
        let relative_z = (z - f64::from(floor_z)) as f32;
        let alpha_z = smoothstep(relative_z);

        for index_x in 0..volume.size_x {
            let x = wrap(f64::from(volume.block_x(index_x)) * xz_scale) + p.offset_x;
            let floor_x = floor(x);
            let relative_x = (x - f64::from(floor_x)) as f32;
            let x0 = p.permute(floor_x);
            let x1 = p.permute(floor_x.wrapping_add(1));
            let alpha_x = smoothstep(relative_x);
            let mut last_floor_y = i32::MIN;

            for index_y in 0..volume.size_y {
                let original_y = f64::from(volume.block_y(index_y)) * y_scale;
                let y = wrap(original_y) + p.offset_y;
                let floor_y = floor(y);
                let relative_y_d = y - f64::from(floor_y);
                let alpha_y = smoothstep(relative_y_d as f32);
                if last_floor_y != floor_y {
                    let xy00 = p.permute(x0 + floor_y);
                    let xy01 = p.permute(x0 + floor_y + 1);
                    let xy10 = p.permute(x1 + floor_y);
                    let xy11 = p.permute(x1 + floor_y + 1);
                    let corners = [
                        (xy00 + floor_z, relative_x, relative_z),
                        (xy10 + floor_z, relative_x - 1.0, relative_z),
                        (xy01 + floor_z, relative_x, relative_z),
                        (xy11 + floor_z, relative_x - 1.0, relative_z),
                        (xy00 + floor_z + 1, relative_x, relative_z - 1.0),
                        (xy10 + floor_z + 1, relative_x - 1.0, relative_z - 1.0),
                        (xy01 + floor_z + 1, relative_x, relative_z - 1.0),
                        (xy11 + floor_z + 1, relative_x - 1.0, relative_z - 1.0),
                    ];
                    for (i, (hash, cx, cz)) in corners.into_iter().enumerate() {
                        let grad = p.permute_to_grad(hash);
                        d[i] = grad.dot_xz(cx, cz);
                        g[i] = grad.y as f32;
                    }
                    last_floor_y = floor_y;
                }

                let ry = fudged_relative_y(original_y, relative_y_d);
                let value = lerp3(
                    alpha_x,
                    alpha_y,
                    alpha_z,
                    d[0] + g[0] * ry,
                    d[1] + g[1] * ry,
                    d[2] + g[2] * (ry - 1.0),
                    d[3] + g[3] * (ry - 1.0),
                    d[4] + g[4] * ry,
                    d[5] + g[5] * ry,
                    d[6] + g[6] * (ry - 1.0),
                    d[7] + g[7] * (ry - 1.0),
                );
                buffer[index] += amplitude * value;
                index += 1;
            }
        }
    }
}
