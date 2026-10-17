use crate::util::interval::Interval;
use crate::util::rng::RandomSource;
use crate::world::worldgen::densityfunction::volume::DensityVolume;

use super::gradient_noise::{GradientNoise, floor, wrap};
use super::perlin_noise::{PerlinNoise, add_to_volume};

const SHIFT_UP_EPSILON: f32 = 1.0E-7;

#[derive(Debug, Clone)]
pub struct SmearedPerlinNoise {
    pub perlin: PerlinNoise,
    pub fudge_y_scale: f64,
}

impl SmearedPerlinNoise {
    pub fn new(random: &mut RandomSource, fudge_y_scale: f64) -> Self {
        Self {
            perlin: PerlinNoise::new(random),
            fudge_y_scale,
        }
    }

    pub fn range_for(fudge_y_scale: f64) -> Interval {
        Interval::of_symmetric((fudge_y_scale.abs() + 2.0) as f32)
    }

    pub fn range(&self) -> Interval {
        Self::range_for(self.fudge_y_scale)
    }

    const fn base(&self) -> &GradientNoise {
        &self.perlin.base
    }

    pub fn get2(&self, x: f64, y: f64) -> f32 {
        self.get(wrap(x), 0.0, wrap(y))
    }

    pub fn get(&self, x_in: f64, y_in: f64, z_in: f64) -> f32 {
        let x = wrap(x_in) + self.base().offset_x;
        let y = wrap(y_in) + self.base().offset_y;
        let z = wrap(z_in) + self.base().offset_z;
        let floor_x = floor(x);
        let floor_y = floor(y);
        let floor_z = floor(z);
        let relative_x = (x - f64::from(floor_x)) as f32;
        let relative_y = y - f64::from(floor_y);
        let relative_z = (z - f64::from(floor_z)) as f32;
        let fudged = (relative_y - compute_fudge_y(self.fudge_y_scale, y_in, relative_y)) as f32;
        self.perlin.sample_and_lerp(
            floor_x,
            floor_y,
            floor_z,
            relative_x,
            fudged,
            relative_z,
            relative_y as f32,
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
        let fudge = self.fudge_y_scale;
        add_to_volume(
            self.base(),
            buffer,
            volume,
            xz_scale,
            y_scale,
            amplitude,
            |original_y, relative_y| {
                (relative_y - compute_fudge_y(fudge, original_y, relative_y)) as f32
            },
        );
    }
}

pub fn compute_fudge_y(fudge_y_scale: f64, original_y: f64, relative_y: f64) -> f64 {
    let fudge_limit = if original_y >= 0.0 && original_y < relative_y {
        original_y
    } else {
        relative_y
    };
    f64::from(floor(
        fudge_limit / fudge_y_scale + f64::from(SHIFT_UP_EPSILON),
    )) * fudge_y_scale
}
