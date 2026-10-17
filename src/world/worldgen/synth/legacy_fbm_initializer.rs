use crate::util::rng::RandomSource;

use super::noise_stack::{Noise, NoiseStack};
use super::perlin_noise::PerlinNoise;

pub fn create_for_legacy_nether_biome(
    random: &mut RandomSource,
    first_octave: i32,
    amplitudes: &[f64],
) -> NoiseStack {
    let octaves = amplitudes.len() as i32;
    let zero_octave_index = -first_octave;
    let mut noise_levels: Vec<Option<PerlinNoise>> = vec![None; octaves as usize];
    let zero_octave = PerlinNoise::new(random);
    if zero_octave_index >= 0 && zero_octave_index < octaves {
        let amplitude = amplitudes[zero_octave_index as usize];
        if amplitude != 0.0 {
            noise_levels[zero_octave_index as usize] = Some(zero_octave);
        }
    }

    let mut i = zero_octave_index - 1;
    while i >= 0 {
        if i < octaves {
            if amplitudes[i as usize] != 0.0 {
                noise_levels[i as usize] = Some(PerlinNoise::new(random));
            } else {
                random.consume_count(262);
            }
        } else {
            random.consume_count(262);
        }
        i -= 1;
    }

    assert!(
        noise_levels.iter().filter(|n| n.is_some()).count()
            == amplitudes.iter().filter(|a| **a != 0.0).count(),
        "Failed to create correct number of noise levels for given non-zero amplitudes"
    );
    assert!(
        zero_octave_index >= octaves - 1,
        "Positive octaves are temporarily disabled"
    );

    let mut factor = 2.0f64.powf(-f64::from(zero_octave_index));
    let mut value_factor =
        2.0f64.powf(f64::from(octaves - 1)) / (2.0f64.powf(f64::from(octaves)) - 1.0);
    let mut stack = NoiseStack::builder();
    for (i, noise) in noise_levels.into_iter().enumerate() {
        if let Some(noise) = noise {
            stack = stack.add(
                Noise::Perlin(noise),
                factor,
                (value_factor * amplitudes[i]) as f32,
            );
        }
        factor *= 2.0;
        value_factor /= 2.0;
    }
    stack.build()
}
