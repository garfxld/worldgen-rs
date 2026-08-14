use std::ops;
use std::simd::cmp::{SimdPartialEq, SimdPartialOrd};
use std::simd::f64x4;
use std::simd::num::SimdFloat;
use std::simd::{Mask, Simd, SimdCast, SimdElement, StdFloat};

use crate::util::noise::PerlinNoise;
use crate::util::rng::name_hash::NameHash;
use crate::util::rng::{RandomFactory, RandomSource};

pub const INPUT_FACTOR: f64 = 1.0181268882175227;

const TARGET_DEVIATION: f64 = 0.16666666666666666;

#[derive(Debug, Clone)]
pub struct NormalNoise {
    first: PerlinNoise,
    second: PerlinNoise,
    value_factor: f64,
    max_value: f64,
}

impl NormalNoise {
    pub fn create_from_random(
        random: &mut RandomSource,
        first_octave: i32,
        amplitudes: &[f64],
    ) -> Self {
        let first = PerlinNoise::create_from_random(random, first_octave, amplitudes);
        let second = PerlinNoise::create_from_random(random, first_octave, amplitudes);

        Self::finish(first, second, amplitudes)
    }

    pub fn create(
        splitter: &RandomFactory,
        noise_id: &str,
        first_octave: i32,
        amplitudes: &[f64],
    ) -> Self {
        let mut random = splitter.with_hash_of(&NameHash::new(noise_id));
        Self::create_from_random(&mut random, first_octave, amplitudes)
    }

    pub fn create_legacy_nether_biome(
        random: &mut RandomSource,
        first_octave: i32,
        amplitudes: &[f64],
    ) -> Self {
        let first = PerlinNoise::create_legacy_for_nether(random, first_octave, amplitudes);
        let second = PerlinNoise::create_legacy_for_nether(random, first_octave, amplitudes);

        Self::finish(first, second, amplitudes)
    }

    fn finish(first: PerlinNoise, second: PerlinNoise, amplitudes: &[f64]) -> Self {
        let mut min_octave = i32::MAX;
        let mut max_octave = i32::MIN;
        for (index, &amplitude) in amplitudes.iter().enumerate() {
            if amplitude != 0.0 {
                min_octave = min_octave.min(index as i32);
                max_octave = max_octave.max(index as i32);
            }
        }

        if min_octave == i32::MAX {
            return Self {
                first,
                second,
                value_factor: 0.0,
                max_value: 0.0,
            };
        }

        let value_factor = TARGET_DEVIATION / expected_deviation(max_octave - min_octave);
        let max_value = (first.max_value() + second.max_value()) * value_factor;

        Self {
            first,
            second,
            value_factor,
            max_value,
        }
    }

    pub const fn first(&self) -> &PerlinNoise {
        &self.first
    }

    pub const fn second(&self) -> &PerlinNoise {
        &self.second
    }

    pub const fn value_factor(&self) -> f64 {
        self.value_factor
    }

    #[inline]
    pub fn get_value(&self, x: f64, y: f64, z: f64) -> f64 {
        let x2 = x * INPUT_FACTOR;
        let y2 = y * INPUT_FACTOR;
        let z2 = z * INPUT_FACTOR;
        (self.first.get_value(x, y, z) + self.second.get_value(x2, y2, z2)) * self.value_factor
    }

    #[inline]
    pub fn get_value_simd<F, const N: usize>(
        &self,
        x: Simd<F, N>,
        y: Simd<F, N>,
        z: Simd<F, N>,
    ) -> Simd<F, N>
    where
        F: SimdElement + SimdCast,
        Simd<F, N>: SimdFloat<Cast<i32> = Simd<i32, N>>
            + SimdPartialOrd
            + SimdPartialEq<Mask = Mask<<F as SimdElement>::Mask, N>>
            + ops::Add<Output = Simd<F, N>>
            + ops::Sub<Output = Simd<F, N>>
            + ops::Mul<Output = Simd<F, N>>
            + ops::Div<Output = Simd<F, N>>
            + ops::Neg<Output = Simd<F, N>>
            + StdFloat,
    {
        let x2 = x * Simd::splat(INPUT_FACTOR).cast::<F>();
        let y2 = y * Simd::splat(INPUT_FACTOR).cast::<F>();
        let z2 = z * Simd::splat(INPUT_FACTOR).cast::<F>();
        (self.first.get_value_simd(x, y, z) + self.second.get_value_simd(x2, y2, z2))
            * Simd::splat(self.value_factor).cast()
    }

    #[inline]
    pub fn get_value_xz(&self, x: f64, z: f64) -> f64 {
        let x2 = x * INPUT_FACTOR;
        let z2 = z * INPUT_FACTOR;
        (self.first.get_value_xz(x, z) + self.second.get_value_xz(x2, z2)) * self.value_factor
    }

    #[inline]
    pub fn get_value_xy(&self, x: f64, y: f64) -> f64 {
        let x2 = x * INPUT_FACTOR;
        let y2 = y * INPUT_FACTOR;
        (self.first.get_value_xy(x, y) + self.second.get_value_xy(x2, y2)) * self.value_factor
    }

    #[inline]
    pub fn get_value_y_4x(&self, x: f64, ys: f64x4, z: f64) -> f64x4 {
        let x2 = x * INPUT_FACTOR;
        let ys2 = ys * f64x4::splat(INPUT_FACTOR);
        let z2 = z * INPUT_FACTOR;
        (self
            .first
            .get_value_with_y_params_4x(x, ys, z, 0.0, 0.0, false)
            + self
                .second
                .get_value_with_y_params_4x(x2, ys2, z2, 0.0, 0.0, false))
            * f64x4::splat(self.value_factor)
    }

    #[inline]
    pub fn get_value_y_simd<const N: usize>(
        &self,
        x: f64,
        ys: Simd<f64, N>,
        z: f64,
    ) -> Simd<f64, N> {
        let x2 = x * INPUT_FACTOR;
        let ys2 = ys * Simd::splat(INPUT_FACTOR);
        let z2 = z * INPUT_FACTOR;
        (self
            .first
            .get_value_with_y_params_simd::<N>(x, ys, z, 0.0, 0.0, false)
            + self
                .second
                .get_value_with_y_params_simd::<N>(x2, ys2, z2, 0.0, 0.0, false))
            * Simd::splat(self.value_factor)
    }

    #[inline]
    pub const fn max_value(&self) -> f64 {
        self.max_value
    }
}

#[inline]
fn expected_deviation(octave_span: i32) -> f64 {
    0.1 * (1.0 + 1.0 / f64::from(octave_span + 1))
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
        let mut random = rng(12345);
        let splitter = random.next_positional();
        let amplitudes = [1.0, 1.0, 1.0];
        let noise1 = NormalNoise::create(&splitter, "test_noise", -3, &amplitudes);
        let noise2 = NormalNoise::create(&splitter, "test_noise", -3, &amplitudes);

        assert!(
            (noise1.get_value(100.0, 64.0, 100.0) - noise2.get_value(100.0, 64.0, 100.0)).abs()
                < 1e-15
        );
    }

    #[test]
    fn zero_axis_helpers_match_full_noise() {
        let mut random = rng(98_765);
        let splitter = random.next_positional();
        let noise = NormalNoise::create(&splitter, "zero_axis", -6, &[1.0, 0.0, 1.0, 1.0, 0.5]);

        for &(a, b) in &[(0.0, 0.0), (1.25, -30.75), (-1000.0, 4096.5)] {
            assert!((noise.get_value_xz(a, b) - noise.get_value(a, 0.0, b)).abs() < 1e-15);
            assert!((noise.get_value_xy(a, b) - noise.get_value(a, b, 0.0)).abs() < 1e-15);
        }
    }

    #[test]
    fn spatial_variation() {
        let mut random = rng(42);
        let splitter = random.next_positional();
        let noise = NormalNoise::create(&splitter, "test_noise", -4, &[1.0, 1.0, 1.0, 1.0]);

        let values: Vec<f64> = (0..10)
            .map(|i| noise.get_value(f64::from(i) * 50.0, 64.0, f64::from(i) * 50.0))
            .collect();
        let min = values.iter().copied().fold(f64::INFINITY, f64::min);
        let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        assert!(max - min > 0.01);
    }

    #[test]
    fn expected_deviation_matches_formula() {
        assert!((expected_deviation(0) - 0.2).abs() < 1e-10);
        assert!((expected_deviation(1) - 0.15).abs() < 1e-10);
    }
}
