use std::collections::BTreeSet;

use crate::util::noise::SimplexNoise;
use crate::util::rng::RandomSource;
use crate::util::rng::legacy_random::LegacyRandomSource;

const POSITIVE_OCTAVE_SEED_SCALE: f64 = 9.223_372_036_854_776e18;

pub struct PerlinSimplexNoise {
    noise_levels: Vec<Option<SimplexNoise>>,
    highest_freq_input_factor: f64,
    highest_freq_value_factor: f64,
}

impl PerlinSimplexNoise {
    pub fn new(random: &mut RandomSource, octaves: &[i32]) -> Self {
        let octave_set: BTreeSet<i32> = octaves.iter().copied().collect();
        assert!(!octave_set.is_empty(), "Need some octaves");

        let low_freq_octaves = -*octave_set.first().expect("non-empty octave set");
        let high_freq_octaves = *octave_set.last().expect("non-empty octave set");
        let octave_count = (low_freq_octaves + high_freq_octaves + 1) as usize;

        let zero_octave = SimplexNoise::new(random);
        let zero_octave_self_sample =
            zero_octave.get_value_3d(zero_octave.xo, zero_octave.yo, zero_octave.zo);
        let zero_octave_index = high_freq_octaves;

        let mut noise_levels: Vec<Option<SimplexNoise>> = vec![None; octave_count];
        if zero_octave_index >= 0
            && (zero_octave_index as usize) < octave_count
            && octave_set.contains(&0)
        {
            noise_levels[zero_octave_index as usize] = Some(zero_octave);
        }

        for i in (zero_octave_index + 1)..octave_count as i32 {
            if i >= 0 && octave_set.contains(&(zero_octave_index - i)) {
                noise_levels[i as usize] = Some(SimplexNoise::new(random));
            } else {
                random.consume_count(262);
            }
        }

        if high_freq_octaves > 0 {
            let positive_octave_seed = (zero_octave_self_sample * POSITIVE_OCTAVE_SEED_SCALE) as i64;
            let mut high_freq_random =
                RandomSource::Legacy(LegacyRandomSource::from_seed(positive_octave_seed as u64));

            for i in (0..zero_octave_index).rev() {
                if (i as usize) < octave_count && octave_set.contains(&(zero_octave_index - i)) {
                    noise_levels[i as usize] = Some(SimplexNoise::new(&mut high_freq_random));
                } else {
                    high_freq_random.consume_count(262);
                }
            }
        }

        Self {
            noise_levels,
            highest_freq_input_factor: 2.0f64.powi(high_freq_octaves),
            highest_freq_value_factor: 1.0 / (2.0f64.powi(octave_count as i32) - 1.0),
        }
    }

    pub fn get_value(&self, x: f64, z: f64) -> f64 {
        let mut value = 0.0;
        let mut factor = self.highest_freq_input_factor;
        let mut value_factor = self.highest_freq_value_factor;

        for noise_level in &self.noise_levels {
            if let Some(noise) = noise_level {
                value += noise.get_value_2d(x * factor, z * factor) * value_factor;
            }
            factor /= 2.0;
            value_factor *= 2.0;
        }

        value
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
