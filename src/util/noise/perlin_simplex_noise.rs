use std::collections::BTreeSet;

use crate::util::noise::SimplexNoise;
use crate::util::rng::RandomSource;
use crate::util::rng::legacy_random::LegacyRandomSource;

pub struct PerlinSimplexNoise {
    noise_levels: Vec<Option<SimplexNoise>>,
    highest_freq_input_factor: f64,
    highest_freq_value_factor: f64,
}

impl PerlinSimplexNoise {
    /// # Panics
    ///
    /// Panics if `octaves` is empty.
    #[must_use]
    pub fn new(random: &mut RandomSource, octaves: &[i32]) -> Self {
        let octave_set: BTreeSet<i32> = octaves.iter().copied().collect();
        assert!(!octave_set.is_empty(), "Need some octaves");

        let first_octave = *octave_set.first().expect("non-empty octave set");
        let last_octave = *octave_set.last().expect("non-empty octave set");
        let high_freq_octaves = last_octave;
        let total = (last_octave - first_octave + 1) as usize;

        let zero_octave = SimplexNoise::new(random);
        let zero_index = high_freq_octaves;

        let mut noise_levels: Vec<Option<SimplexNoise>> = vec![None; total];

        let hf_seed = if high_freq_octaves > 0 {
            Some(
                (zero_octave.get_value_3d(zero_octave.xo, zero_octave.yo, zero_octave.zo)
                    * 9.223_372_036_854_776e18) as i64,
            )
        } else {
            None
        };

        if zero_index >= 0 && (zero_index as usize) < total && octave_set.contains(&0) {
            noise_levels[zero_index as usize] = Some(zero_octave);
        }

        let start = (zero_index + 1).max(0) as usize;
        for (i, level) in noise_levels.iter_mut().enumerate().skip(start) {
            let octave_level = zero_index - i as i32;
            if octave_set.contains(&octave_level) {
                *level = Some(SimplexNoise::new(random));
            } else {
                random.consume_count(262);
            }
        }

        if let Some(seed) = hf_seed {
            let mut hf_random = RandomSource::Legacy(LegacyRandomSource::from_seed(seed as u64));

            for ix in (0..zero_index as usize).rev() {
                let octave_level = zero_index - ix as i32;
                if octave_set.contains(&octave_level) {
                    noise_levels[ix] = Some(SimplexNoise::new(&mut hf_random));
                } else {
                    hf_random.consume_count(262);
                }
            }
        }

        Self {
            noise_levels,
            highest_freq_input_factor: 2.0f64.powi(last_octave),
            highest_freq_value_factor: 1.0 / (2.0f64.powi(total as i32) - 1.0),
        }
    }

    #[must_use]
    pub fn get_value(&self, x: f64, z: f64) -> f64 {
        let mut sum = 0.0;
        let mut factor = self.highest_freq_input_factor;
        let mut amplitude = self.highest_freq_value_factor;

        for noise in &self.noise_levels {
            if let Some(n) = noise {
                sum += n.get_value_2d(x * factor, z * factor) * amplitude;
            }
            factor /= 2.0;
            amplitude *= 2.0;
        }

        sum
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
    fn spatial_variation() {
        let noise = PerlinSimplexNoise::new(&mut rng(42), &[-2, -1, 0]);
        let values: Vec<f64> = (0..10)
            .map(|i| noise.get_value(f64::from(i) * 10.0, f64::from(i) * 5.0))
            .collect();
        let min = values.iter().copied().fold(f64::INFINITY, f64::min);
        let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        assert!(max - min > 0.001);
    }

    #[test]
    fn deterministic_for_same_seed() {
        let n1 = PerlinSimplexNoise::new(&mut rng(7), &[-3, -2, -1, 0, 1]);
        let n2 = PerlinSimplexNoise::new(&mut rng(7), &[-3, -2, -1, 0, 1]);
        assert!((n1.get_value(12.5, -4.25) - n2.get_value(12.5, -4.25)).abs() < 1e-15);
    }
}
