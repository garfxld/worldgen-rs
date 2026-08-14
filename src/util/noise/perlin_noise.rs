use std::ops;
use std::simd::cmp::{SimdPartialEq, SimdPartialOrd};
use std::simd::num::SimdFloat;
use std::simd::{Mask, Simd, SimdCast, SimdElement, StdFloat, f64x4};

use crate::util::math::{wrap, wrap_simd};
use crate::util::noise::ImprovedNoise;
use crate::util::rng::name_hash::NameHash;
use crate::util::rng::{RandomFactory, RandomSource};

#[derive(Debug, Clone)]
pub struct PerlinNoise {
    noise_levels: Vec<Option<ImprovedNoise>>,
    amplitudes: Vec<f64>,
    active_octaves: Vec<ActiveOctave>,
    lowest_freq_value_factor: f64,
    max_value: f64,
}

#[derive(Debug, Clone)]
struct ActiveOctave {
    noise: ImprovedNoise,
    input_factor: f64,
    output_factor: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct OctaveSnapshot<'a> {
    pub noise: &'a ImprovedNoise,
    pub input_factor: f64,
    pub output_factor: f64,
}

impl PerlinNoise {
    pub fn active_octaves(&self) -> impl ExactSizeIterator<Item = OctaveSnapshot<'_>> {
        self.active_octaves.iter().map(|octave| OctaveSnapshot {
            noise: &octave.noise,
            input_factor: octave.input_factor,
            output_factor: octave.output_factor,
        })
    }

    pub fn create(splitter: &RandomFactory, first_octave: i32, amplitudes: &[f64]) -> Self {
        let octave_count = amplitudes.len();
        let zero_octave_index = (-first_octave) as usize;

        let mut noise_levels = vec![None; octave_count];
        for (index, level) in noise_levels.iter_mut().enumerate() {
            if amplitudes[index] != 0.0 {
                let octave = first_octave + index as i32;
                let mut octave_random =
                    splitter.with_hash_of(&NameHash::new(&format!("octave_{octave}")));
                *level = Some(ImprovedNoise::new(&mut octave_random));
            }
        }

        Self::from_parts(noise_levels, amplitudes, zero_octave_index)
    }

    pub fn create_from_random(
        random: &mut RandomSource,
        first_octave: i32,
        amplitudes: &[f64],
    ) -> Self {
        Self::create(&random.next_positional(), first_octave, amplitudes)
    }

    pub fn create_legacy_for_nether(
        random: &mut RandomSource,
        first_octave: i32,
        amplitudes: &[f64],
    ) -> Self {
        let octave_count = amplitudes.len();
        let zero_octave_index = (-first_octave) as usize;

        let mut noise_levels = vec![None; octave_count];

        let zero_octave = ImprovedNoise::new(random);
        if zero_octave_index < octave_count && amplitudes[zero_octave_index] != 0.0 {
            noise_levels[zero_octave_index] = Some(zero_octave);
        }

        for index in (0..zero_octave_index).rev() {
            if index < octave_count && amplitudes[index] != 0.0 {
                noise_levels[index] = Some(ImprovedNoise::new(random));
            } else {
                random.consume_count(262);
            }
        }

        Self::from_parts(noise_levels, amplitudes, zero_octave_index)
    }

    fn from_parts(
        noise_levels: Vec<Option<ImprovedNoise>>,
        amplitudes: &[f64],
        zero_octave_index: usize,
    ) -> Self {
        let octave_count = amplitudes.len();

        let lowest_freq_input_factor = 2.0_f64.powi(-(zero_octave_index as i32));
        let lowest_freq_value_factor =
            2.0_f64.powi((octave_count - 1) as i32) / (2.0_f64.powi(octave_count as i32) - 1.0);
        let max_value = Self::edge_value(amplitudes, lowest_freq_value_factor, 2.0);

        let mut active_octaves = Vec::with_capacity(noise_levels.len());
        let mut input_factor = lowest_freq_input_factor;
        let mut value_factor = lowest_freq_value_factor;
        for (index, level) in noise_levels.iter().enumerate() {
            if let Some(noise) = level {
                active_octaves.push(ActiveOctave {
                    noise: noise.clone(),
                    input_factor,
                    output_factor: amplitudes[index] * value_factor,
                });
            }
            input_factor *= 2.0;
            value_factor /= 2.0;
        }

        Self {
            noise_levels,
            amplitudes: amplitudes.to_vec(),
            active_octaves,
            lowest_freq_value_factor,
            max_value,
        }
    }

    fn edge_value(amplitudes: &[f64], lowest_freq_value_factor: f64, noise_value: f64) -> f64 {
        let mut value = 0.0;
        let mut value_factor = lowest_freq_value_factor;

        for &amplitude in amplitudes {
            if amplitude != 0.0 {
                value += amplitude * noise_value * value_factor;
            }
            value_factor /= 2.0;
        }

        value
    }

    #[inline]
    pub fn get_value(&self, x: f64, y: f64, z: f64) -> f64 {
        let mut value = 0.0;

        for octave in &self.active_octaves {
            let factor = octave.input_factor;
            let noise_value =
                octave
                    .noise
                    .noise(wrap(x * factor), wrap(y * factor), wrap(z * factor));
            value += octave.output_factor * noise_value;
        }

        value
    }

    #[inline]
    pub fn get_value_xz(&self, x: f64, z: f64) -> f64 {
        let mut value = 0.0;

        for octave in &self.active_octaves {
            let factor = octave.input_factor;
            let noise_value = octave.noise.noise_xz(wrap(x * factor), wrap(z * factor));
            value += octave.output_factor * noise_value;
        }

        value
    }

    #[inline]
    pub fn get_value_xy(&self, x: f64, y: f64) -> f64 {
        let mut value = 0.0;

        for octave in &self.active_octaves {
            let factor = octave.input_factor;
            let noise_value = octave.noise.noise_xy(wrap(x * factor), wrap(y * factor));
            value += octave.output_factor * noise_value;
        }

        value
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
        let mut value = Simd::splat(0.0).cast();

        for octave in &self.active_octaves {
            let factor = Simd::splat(octave.input_factor).cast();
            let noise_value = octave.noise.noise_simd(
                wrap_simd(x * factor),
                wrap_simd(y * factor),
                wrap_simd(z * factor),
            );
            value += Simd::splat(octave.output_factor).cast() * noise_value;
        }

        value
    }

    pub fn get_value_with_y_params(
        &self,
        x: f64,
        y: f64,
        z: f64,
        y_scale: f64,
        y_fudge: f64,
        y_flat_hack: bool,
    ) -> f64 {
        let mut value = 0.0;

        for octave in &self.active_octaves {
            let factor = octave.input_factor;
            let noise = &octave.noise;
            let noise_value = noise.noise_with_y_scale(
                wrap(x * factor),
                if y_flat_hack {
                    -noise.yo
                } else {
                    wrap(y * factor)
                },
                wrap(z * factor),
                y_scale * factor,
                y_fudge * factor,
            );
            value += octave.output_factor * noise_value;
        }

        value
    }

    pub fn get_value_with_y_params_4x(
        &self,
        x: f64,
        ys: f64x4,
        z: f64,
        y_scale: f64,
        y_fudge: f64,
        y_flat_hack: bool,
    ) -> f64x4 {
        let mut value = f64x4::splat(0.0);

        for octave in &self.active_octaves {
            let factor = octave.input_factor;
            let noise = &octave.noise;
            let x_w = wrap(x * factor);
            let z_w = wrap(z * factor);
            let ys_w = if y_flat_hack {
                f64x4::splat(-noise.yo)
            } else {
                wrap_simd(ys * f64x4::splat(factor))
            };
            let y_fudges = f64x4::splat(y_fudge * factor);
            let noise_value =
                noise.noise_with_y_scale_simd(x_w, ys_w, z_w, y_scale * factor, y_fudges);
            value += f64x4::splat(octave.output_factor) * noise_value;
        }

        value
    }

    pub fn get_value_with_y_params_simd<const N: usize>(
        &self,
        x: f64,
        ys: Simd<f64, N>,
        z: f64,
        y_scale: f64,
        y_fudge: f64,
        y_flat_hack: bool,
    ) -> Simd<f64, N> {
        let mut value = Simd::splat(0.0);

        for octave in &self.active_octaves {
            let factor = octave.input_factor;
            let noise = &octave.noise;
            let x_w = wrap(x * factor);
            let z_w = wrap(z * factor);
            let ys_w = if y_flat_hack {
                Simd::splat(-noise.yo)
            } else {
                wrap_simd(ys * Simd::splat(factor))
            };
            let y_fudges = Simd::splat(y_fudge * factor);
            let noise_value =
                noise.noise_with_y_scale_simd(x_w, ys_w, z_w, y_scale * factor, y_fudges);
            value += Simd::splat(octave.output_factor) * noise_value;
        }

        value
    }

    #[inline]
    pub const fn max_value(&self) -> f64 {
        self.max_value
    }

    pub fn max_broken_value(&self, y_scale: f64) -> f64 {
        Self::edge_value(
            &self.amplitudes,
            self.lowest_freq_value_factor,
            y_scale + 2.0,
        )
    }

    pub fn get_octave_noise(&self, index: usize) -> Option<&ImprovedNoise> {
        self.noise_levels
            .get(self.noise_levels.len() - 1 - index)
            .and_then(|level| level.as_ref())
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
    fn get_value_matches_zero_y_params_path() {
        let mut random = rng(12345);
        let splitter = random.next_positional();
        let noise = PerlinNoise::create(&splitter, -4, &[1.0, 0.0, 1.0, 1.0]);

        for (x, y, z) in [
            (0.0, 0.0, 0.0),
            (100.0, 64.0, -100.0),
            (-4096.25, -32.5, 1024.75),
        ] {
            assert!(
                (noise.get_value(x, y, z)
                    - noise.get_value_with_y_params(x, y, z, 0.0, 0.0, false))
                .abs()
                    < 1e-15
            );
        }
    }

    #[test]
    fn zero_axis_helpers_match_full_noise() {
        let mut random = rng(98_765);
        let splitter = random.next_positional();
        let noise = PerlinNoise::create(&splitter, -6, &[1.0, 0.0, 1.0, 1.0, 0.5]);

        for &(a, b) in &[(0.0, 0.0), (1.25, -30.75), (-1000.0, 4096.5)] {
            assert!((noise.get_value_xz(a, b) - noise.get_value(a, 0.0, b)).abs() < 1e-15);
            assert!((noise.get_value_xy(a, b) - noise.get_value(a, b, 0.0)).abs() < 1e-15);
        }
    }

    #[test]
    fn spatial_variation() {
        let mut random = rng(42);
        let splitter = random.next_positional();
        let noise = PerlinNoise::create(&splitter, -4, &[1.0, 1.0, 1.0, 1.0]);

        let values: Vec<f64> = (0..10)
            .map(|i| noise.get_value(f64::from(i) * 50.0, 64.0, f64::from(i) * 50.0))
            .collect();
        let min = values.iter().copied().fold(f64::INFINITY, f64::min);
        let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        assert!(max - min > 0.01);
    }
}
