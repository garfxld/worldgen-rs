use serde::{Deserialize, Deserializer};

use crate::util::interval::Interval;
use crate::util::rng::RandomSource;
use crate::util::rng::name_hash::NameHash;

use super::legacy_fbm_initializer::create_for_legacy_nether_biome;
use super::noise_stack::{Noise, NoiseStack};
use super::perlin_noise::{PerlinNoise, STANDARD_DEVIATION};
use serde::de;

const INPUT_FACTOR: f64 = 1.018_126_888_217_522_7;
const TARGET_DEVIATION: f64 = 0.333_333_333_333_333_3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Normalization {
    Disabled,
    Enabled,
    Legacy,
}

impl<'de> Deserialize<'de> for Normalization {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Bool(bool),
            Str(String),
        }
        match Raw::deserialize(d)? {
            Raw::Bool(true) => Ok(Self::Enabled),
            Raw::Bool(false) => Ok(Self::Disabled),
            Raw::Str(s) if s == "legacy" => Ok(Self::Legacy),
            Raw::Str(s) => Err(de::Error::custom(format!(
                "Invalid normalization type: {s}"
            ))),
        }
    }
}

const fn default_base_amplitude() -> f64 {
    1.0
}

const fn default_octave_count() -> i32 {
    1
}

const fn default_normalize() -> Normalization {
    Normalization::Enabled
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Parameters {
    #[serde(default = "default_base_amplitude")]
    pub base_amplitude: f64,
    pub base_octave: i32,
    #[serde(default = "default_octave_count")]
    pub octave_count: i32,
    #[serde(default = "default_normalize")]
    pub normalize: Normalization,
    #[serde(default)]
    pub amplitude_modifiers: Vec<f64>,
}

#[derive(Debug, Clone, Copy)]
struct OctaveInfo {
    octave_index: i32,
    frequency: f64,
    amplitude: f64,
}

impl OctaveInfo {
    fn seed(self) -> String {
        format!("octave_{}", self.octave_index)
    }

    const fn abs_amplitude(self) -> f64 {
        self.amplitude.abs()
    }
}

#[derive(Debug, Clone)]
pub struct NormalNoise {
    pub parameters: Parameters,
    octaves: Vec<OctaveInfo>,
    normalization_factor: f64,
    range: Interval,
}

const fn amplitude_modifier(modifiers: &[f64], index: i32) -> f64 {
    if modifiers.is_empty() {
        1.0
    } else {
        modifiers[index as usize]
    }
}

fn build_octaves(
    base_octave: i32,
    base_amplitude: f64,
    octave_count: i32,
    normalize: bool,
    modifiers: &[f64],
) -> Vec<OctaveInfo> {
    let mut frequency = 2.0f64.powi(base_octave);
    let mut amplitude = base_amplitude;
    if normalize {
        amplitude *= 0.5f64.powf(-f64::from(octave_count - 1))
            / (0.5f64.powf(-f64::from(octave_count)) - 1.0);
    }
    let mut octaves = Vec::with_capacity(octave_count as usize);
    for i in 0..octave_count {
        let modifier = amplitude_modifier(modifiers, i);
        if modifier != 0.0 {
            octaves.push(OctaveInfo {
                octave_index: base_octave + i,
                frequency,
                amplitude: amplitude * modifier,
            });
        }
        frequency *= 2.0;
        amplitude *= 0.5;
    }
    octaves
}

fn estimate_deviation(octaves: &[OctaveInfo]) -> f64 {
    let mut variance = 0.0;
    for octave in octaves {
        let layer_deviation = STANDARD_DEVIATION * octave.abs_amplitude();
        variance += layer_deviation * layer_deviation;
    }
    variance.sqrt()
}

fn compute_normalization_factor(target_amplitude: f64, octaves: &[OctaveInfo]) -> f64 {
    let input_deviation = estimate_deviation(octaves);
    if input_deviation == 0.0 {
        return 0.0;
    }
    let input_sum_deviation = input_deviation * 2.0f64.sqrt();
    let target_deviation = target_amplitude * TARGET_DEVIATION;
    target_deviation / input_sum_deviation
}

fn parity_expected_deviation(octave_span: i32) -> f64 {
    0.1 * (1.0 + 1.0 / f64::from(octave_span + 1))
}

fn compute_parity_normalization_factor(
    base_amplitude: f64,
    octave_count: i32,
    modifiers: &[f64],
) -> f64 {
    let mut min_octave = i32::MAX;
    let mut max_octave = i32::MIN;
    for i in 0..octave_count {
        if amplitude_modifier(modifiers, i) != 0.0 {
            min_octave = min_octave.min(i);
            max_octave = max_octave.max(i);
        }
    }
    base_amplitude * 0.5 * TARGET_DEVIATION
        / parity_expected_deviation(max_octave.wrapping_sub(min_octave))
}

fn compute_parity_base_amplitude(base_octave: i32, amplitudes: &[f64]) -> f64 {
    let octaves = build_octaves(base_octave, 1.0, amplitudes.len() as i32, true, amplitudes);
    let target_amplitude = sum_abs(&octaves);
    let new_normalization_factor = compute_normalization_factor(target_amplitude, &octaves);
    if new_normalization_factor == 0.0 {
        return 1.0;
    }
    let old_normalization_factor =
        compute_parity_normalization_factor(1.0, amplitudes.len() as i32, amplitudes);
    old_normalization_factor / new_normalization_factor
}

fn sum_abs(octaves: &[OctaveInfo]) -> f64 {
    let mut sum = 0.0;
    for octave in octaves {
        sum += octave.abs_amplitude();
    }
    sum
}

impl NormalNoise {
    pub fn new(parameters: Parameters) -> Self {
        let octaves = build_octaves(
            parameters.base_octave,
            parameters.base_amplitude,
            parameters.octave_count,
            parameters.normalize != Normalization::Disabled,
            &parameters.amplitude_modifiers,
        );
        let mut target_amplitude = sum_abs(&octaves);
        let mut normalization_factor = compute_normalization_factor(target_amplitude, &octaves);
        if parameters.normalize == Normalization::Legacy && normalization_factor != 0.0 {
            let parity = compute_parity_normalization_factor(
                parameters.base_amplitude,
                parameters.octave_count,
                &parameters.amplitude_modifiers,
            );
            target_amplitude *= parity / normalization_factor;
            normalization_factor = parity;
        }
        let range = Interval::of_symmetric((target_amplitude * TARGET_DEVIATION * 6.0) as f32);
        Self {
            parameters,
            octaves,
            normalization_factor,
            range,
        }
    }

    pub fn create_parity(first_octave: i32, amplitudes: &[f64]) -> Self {
        assert!(!amplitudes.is_empty(), "Need at least 1 amplitude");
        let octave_count = amplitudes.len() as i32;
        let mut amplitude_modifiers = Vec::new();
        for (i, &modifier) in amplitudes.iter().enumerate() {
            if modifier != 1.0 {
                if amplitude_modifiers.is_empty() {
                    amplitude_modifiers = vec![1.0; amplitudes.len()];
                }
                amplitude_modifiers[i] = modifier;
            }
        }
        Self::new(Parameters {
            base_amplitude: compute_parity_base_amplitude(first_octave, amplitudes),
            base_octave: first_octave,
            octave_count,
            normalize: Normalization::Enabled,
            amplitude_modifiers,
        })
    }

    pub const fn range(&self) -> Interval {
        self.range
    }

    pub fn create(&self, random: &mut RandomSource) -> Noise {
        let first_random = random.next_positional();
        let second_random = random.next_positional();
        let mut stack = NoiseStack::builder();
        for octave in &self.octaves {
            let seed = NameHash::new(&octave.seed());
            let first = PerlinNoise::new(&mut first_random.with_hash_of(&seed));
            let second = PerlinNoise::new(&mut second_random.with_hash_of(&seed));
            let value_factor = (self.normalization_factor * octave.amplitude) as f32;
            stack = stack
                .add(Noise::Perlin(first), octave.frequency, value_factor)
                .add(
                    Noise::Perlin(second),
                    octave.frequency * INPUT_FACTOR,
                    value_factor,
                );
        }
        Noise::Stack(stack.build())
    }

    pub fn create_for_legacy_nether_biome(&self, random: &mut RandomSource) -> Noise {
        let amplitudes = if self.parameters.amplitude_modifiers.is_empty() {
            vec![1.0; self.parameters.octave_count as usize]
        } else {
            self.parameters.amplitude_modifiers.clone()
        };
        let first =
            create_for_legacy_nether_biome(random, self.parameters.base_octave, &amplitudes);
        let second =
            create_for_legacy_nether_biome(random, self.parameters.base_octave, &amplitudes);
        let value_factor = (self.normalization_factor * self.parameters.base_amplitude) as f32;
        Noise::Stack(
            NoiseStack::builder()
                .add_stack(first, 1.0, value_factor)
                .add_stack(second, INPUT_FACTOR, value_factor)
                .build(),
        )
    }
}
