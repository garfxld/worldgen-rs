use std::simd::Simd;
use std::simd::cmp::SimdPartialOrd;

use crate::util::math::{clamped_lerp, clamped_lerp_simd, wrap, wrap_simd};
use crate::util::noise::PerlinNoise;
use crate::util::rng::RandomSource;

const COORDINATE_SCALE: f64 = 684.412;

#[derive(Debug, Clone)]
pub struct BlendedNoise {
    min_limit_noise: PerlinNoise,
    max_limit_noise: PerlinNoise,
    main_noise: PerlinNoise,
    xz_multiplier: f64,
    y_multiplier: f64,
    xz_factor: f64,
    y_factor: f64,
    smear_scale_multiplier: f64,
    max_value: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct BlendedNoiseSnapshot<'a> {
    pub min_limit_noise: &'a PerlinNoise,
    pub max_limit_noise: &'a PerlinNoise,
    pub main_noise: &'a PerlinNoise,
    pub xz_multiplier: f64,
    pub y_multiplier: f64,
    pub xz_factor: f64,
    pub y_factor: f64,
    pub smear_scale_multiplier: f64,
}

impl BlendedNoise {
    #[must_use]
    pub const fn snapshot(&self) -> BlendedNoiseSnapshot<'_> {
        BlendedNoiseSnapshot {
            min_limit_noise: &self.min_limit_noise,
            max_limit_noise: &self.max_limit_noise,
            main_noise: &self.main_noise,
            xz_multiplier: self.xz_multiplier,
            y_multiplier: self.y_multiplier,
            xz_factor: self.xz_factor,
            y_factor: self.y_factor,
            smear_scale_multiplier: self.smear_scale_multiplier,
        }
    }

    #[must_use]
    pub fn new(
        random: &mut RandomSource,
        xz_scale: f64,
        y_scale: f64,
        xz_factor: f64,
        y_factor: f64,
        smear_scale_multiplier: f64,
    ) -> Self {
        let min_limit_noise = PerlinNoise::create_legacy_for_nether(random, -15, &[1.0; 16]);
        let max_limit_noise = PerlinNoise::create_legacy_for_nether(random, -15, &[1.0; 16]);
        let main_noise = PerlinNoise::create_legacy_for_nether(random, -7, &[1.0; 8]);

        let xz_multiplier = COORDINATE_SCALE * xz_scale;
        let y_multiplier = COORDINATE_SCALE * y_scale;
        let max_value = min_limit_noise.max_broken_value(y_multiplier);

        Self {
            min_limit_noise,
            max_limit_noise,
            main_noise,
            xz_multiplier,
            y_multiplier,
            xz_factor,
            y_factor,
            smear_scale_multiplier,
            max_value,
        }
    }

    #[must_use]
    pub fn compute(&self, block_x: f64, block_y: f64, block_z: f64) -> f64 {
        let limit_x = block_x * self.xz_multiplier;
        let limit_y = block_y * self.y_multiplier;
        let limit_z = block_z * self.xz_multiplier;
        let main_x = limit_x / self.xz_factor;
        let main_y = limit_y / self.y_factor;
        let main_z = limit_z / self.xz_factor;
        let limit_smear = self.y_multiplier * self.smear_scale_multiplier;
        let main_smear = limit_smear / self.y_factor;

        let mut main_noise_value = 0.0;
        let mut pow = 1.0;
        for i in 0..8 {
            if let Some(noise) = self.main_noise.get_octave_noise(i) {
                main_noise_value += noise.noise_with_y_scale(
                    wrap(main_x * pow),
                    wrap(main_y * pow),
                    wrap(main_z * pow),
                    main_smear * pow,
                    main_y * pow,
                ) / pow;
            }
            pow /= 2.0;
        }

        let factor = f64::midpoint(main_noise_value / 10.0, 1.0);
        let is_max = factor >= 1.0;
        let is_min = factor <= 0.0;

        let mut blend_min = 0.0;
        let mut blend_max = 0.0;
        pow = 1.0;
        for i in 0..16 {
            let wx = wrap(limit_x * pow);
            let wy = wrap(limit_y * pow);
            let wz = wrap(limit_z * pow);
            let y_scale_pow = limit_smear * pow;

            if !is_max && let Some(noise) = self.min_limit_noise.get_octave_noise(i) {
                blend_min += noise.noise_with_y_scale(wx, wy, wz, y_scale_pow, limit_y * pow) / pow;
            }

            if !is_min && let Some(noise) = self.max_limit_noise.get_octave_noise(i) {
                blend_max += noise.noise_with_y_scale(wx, wy, wz, y_scale_pow, limit_y * pow) / pow;
            }

            pow /= 2.0;
        }

        clamped_lerp(blend_min / 512.0, blend_max / 512.0, factor) / 128.0
    }

    #[inline]
    #[must_use]
    pub fn compute_simd<const N: usize>(
        &self,
        block_x: f64,
        block_ys: [f64; N],
        block_z: f64,
    ) -> [f64; N] {
        let limit_x = block_x * self.xz_multiplier;
        let limit_ys = Simd::from_array(block_ys) * Simd::splat(self.y_multiplier);
        let limit_z = block_z * self.xz_multiplier;
        let main_x = limit_x / self.xz_factor;
        let main_ys = limit_ys / Simd::splat(self.y_factor);
        let main_z = limit_z / self.xz_factor;
        let limit_smear = self.y_multiplier * self.smear_scale_multiplier;
        let main_smear = limit_smear / self.y_factor;

        let mut main_noise_values = Simd::splat(0.0);
        let mut pow = 1.0;
        for i in 0..8 {
            if let Some(noise) = self.main_noise.get_octave_noise(i) {
                let pow_v = Simd::splat(pow);
                let scaled_ys = main_ys * pow_v;
                main_noise_values += noise.noise_with_y_scale_simd(
                    wrap(main_x * pow),
                    wrap_simd(scaled_ys),
                    wrap(main_z * pow),
                    main_smear * pow,
                    scaled_ys,
                ) / pow_v;
            }
            pow /= 2.0;
        }

        let factors = (main_noise_values / Simd::splat(10.0) + Simd::splat(1.0)) / Simd::splat(2.0);

        let all_max = factors.simd_ge(Simd::splat(1.0)).all();
        let all_min = factors.simd_le(Simd::splat(0.0)).all();

        let mut blend_min = Simd::splat(0.0);
        let mut blend_max = Simd::splat(0.0);
        pow = 1.0;
        for i in 0..16 {
            let pow_v = Simd::splat(pow);
            let scaled_ys = limit_ys * pow_v;
            let wx = wrap(limit_x * pow);
            let wys = wrap_simd(scaled_ys);
            let wz = wrap(limit_z * pow);
            let y_scale_pow = limit_smear * pow;

            if !all_max && let Some(noise) = self.min_limit_noise.get_octave_noise(i) {
                blend_min +=
                    noise.noise_with_y_scale_simd(wx, wys, wz, y_scale_pow, scaled_ys) / pow_v;
            }

            if !all_min && let Some(noise) = self.max_limit_noise.get_octave_noise(i) {
                blend_max +=
                    noise.noise_with_y_scale_simd(wx, wys, wz, y_scale_pow, scaled_ys) / pow_v;
            }

            pow /= 2.0;
        }

        let min_scaled = blend_min / Simd::splat(512.0);
        let max_scaled = blend_max / Simd::splat(512.0);
        let result = clamped_lerp_simd(min_scaled, max_scaled, factors) / Simd::splat(128.0);
        result.to_array()
    }

    pub fn compute_column(&self, block_x: i32, block_ys: &[i32], block_z: i32, out: &mut [f64]) {
        let count = block_ys.len().min(out.len());
        let block_x = f64::from(block_x);
        let block_z = f64::from(block_z);
        let mut processed = 0;

        let chunks_4 = count / 4;
        for chunk in 0..chunks_4 {
            let base = chunk * 4;
            let batch_ys = [
                f64::from(block_ys[base]),
                f64::from(block_ys[base + 1]),
                f64::from(block_ys[base + 2]),
                f64::from(block_ys[base + 3]),
            ];
            out[base..base + 4].copy_from_slice(&self.compute_simd(block_x, batch_ys, block_z));
        }
        processed += chunks_4 * 4;

        if count - processed >= 2 {
            let batch_ys = [
                f64::from(block_ys[processed]),
                f64::from(block_ys[processed + 1]),
            ];
            out[processed..processed + 2]
                .copy_from_slice(&self.compute_simd(block_x, batch_ys, block_z));
            processed += 2;
        }

        for i in processed..count {
            out[i] = self.compute(block_x, f64::from(block_ys[i]), block_z);
        }
    }

    #[inline]
    #[must_use]
    pub const fn max_value(&self) -> f64 {
        self.max_value
    }

    #[inline]
    #[must_use]
    pub fn min_value(&self) -> f64 {
        -self.max_value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::rng::xoroshiro::XoroshiroRandomSource;

    fn rng(seed: u64) -> RandomSource {
        RandomSource::Xoroshiro(XoroshiroRandomSource::from_seed(seed))
    }

    #[test]
    fn deterministic_for_same_seed() {
        let bn1 = BlendedNoise::new(&mut rng(12345), 1.0, 1.0, 80.0, 160.0, 8.0);
        let bn2 = BlendedNoise::new(&mut rng(12345), 1.0, 1.0, 80.0, 160.0, 8.0);
        assert!((bn1.compute(0.0, 64.0, 0.0) - bn2.compute(0.0, 64.0, 0.0)).abs() < 1e-15);
    }

    #[test]
    fn compute_4x_matches_scalar() {
        let bn = BlendedNoise::new(&mut rng(42), 1.0, 1.0, 80.0, 160.0, 8.0);
        let cases: &[(f64, [f64; 4], f64)] = &[
            (0.0, [0.0, 8.0, 16.0, 24.0], 0.0),
            (16.0, [32.0, 40.0, 48.0, 56.0], 16.0),
            (-8.0, [-64.0, -32.0, 0.0, 32.0], 8.0),
        ];

        for &(x, ys, z) in cases {
            let simd = bn.compute_simd(x, ys, z);
            for i in 0..4 {
                let scalar = bn.compute(x, ys[i], z);
                assert!((scalar - simd[i]).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn spatial_variation() {
        let bn = BlendedNoise::new(&mut rng(42), 1.0, 1.0, 80.0, 160.0, 8.0);
        let values: Vec<f64> = (-5..5)
            .map(|x| bn.compute(f64::from(x * 16), 64.0, 0.0))
            .collect();
        let min = values.iter().copied().fold(f64::INFINITY, f64::min);
        let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        assert!(max - min > 1e-6);
    }
}
