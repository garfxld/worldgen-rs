use serde::Deserialize;

use crate::util::rng::RandomSource;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(from = "RawFloatProvider")]
pub enum FloatProvider {
    Constant(f32),
    Uniform {
        min_inclusive: f32,
        max_exclusive: f32,
    },
    Trapezoid {
        min: f32,
        max: f32,
        plateau: f32,
    },
    ClampedNormal {
        mean: f32,
        deviation: f32,
        min: f32,
        max: f32,
    },
}

impl FloatProvider {
    pub fn sample(self, random: &mut RandomSource) -> f32 {
        match self {
            Self::Constant(v) => v,
            Self::Uniform {
                min_inclusive,
                max_exclusive,
            } => random.next_f32() * (max_exclusive - min_inclusive) + min_inclusive,
            Self::Trapezoid { min, max, plateau } => {
                let range = max - min;
                let plateau_start = (range - plateau) / 2.0;
                let plateau_end = range - plateau_start;
                min + random.next_f32() * plateau_end + random.next_f32() * plateau_start
            }
            Self::ClampedNormal {
                mean,
                deviation,
                min,
                max,
            } => {
                let sample = mean + deviation * random.next_gaussian() as f32;
                sample.clamp(min, max)
            }
        }
    }

    pub const fn min(self) -> f32 {
        match self {
            Self::Constant(v) => v,
            Self::Uniform { min_inclusive, .. } => min_inclusive,
            Self::Trapezoid { min, .. } | Self::ClampedNormal { min, .. } => min,
        }
    }

    pub const fn max(self) -> f32 {
        match self {
            Self::Constant(v) => v,
            Self::Uniform { max_exclusive, .. } => max_exclusive,
            Self::Trapezoid { max, .. } | Self::ClampedNormal { max, .. } => max,
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum RawFloatProvider {
    Constant(f32),
    Tagged(TaggedFloatProvider),
}

#[derive(Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum TaggedFloatProvider {
    #[serde(rename = "minecraft:constant")]
    Constant { value: f32 },
    #[serde(rename = "minecraft:uniform")]
    Uniform {
        min_inclusive: f32,
        max_exclusive: f32,
    },
    #[serde(rename = "minecraft:trapezoid")]
    Trapezoid { min: f32, max: f32, plateau: f32 },
    #[serde(rename = "minecraft:clamped_normal")]
    ClampedNormal {
        mean: f32,
        deviation: f32,
        min: f32,
        max: f32,
    },
}

impl From<RawFloatProvider> for FloatProvider {
    fn from(raw: RawFloatProvider) -> Self {
        let tagged = match raw {
            RawFloatProvider::Constant(value) => return Self::Constant(value),
            RawFloatProvider::Tagged(tagged) => tagged,
        };
        match tagged {
            TaggedFloatProvider::Constant { value } => Self::Constant(value),
            TaggedFloatProvider::Uniform {
                min_inclusive,
                max_exclusive,
            } => Self::Uniform {
                min_inclusive,
                max_exclusive,
            },
            TaggedFloatProvider::Trapezoid { min, max, plateau } => {
                Self::Trapezoid { min, max, plateau }
            }
            TaggedFloatProvider::ClampedNormal {
                mean,
                deviation,
                min,
                max,
            } => Self::ClampedNormal {
                mean,
                deviation,
                min,
                max,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::rng::xoroshiro::XoroshiroRandomSource;

    fn rng() -> RandomSource {
        RandomSource::Xoroshiro(XoroshiroRandomSource::from_seed(0))
    }

    #[test]
    fn constant_provider_ignores_the_rng() {
        let provider = FloatProvider::Constant(3.5);
        let mut random = rng();
        assert!((provider.sample(&mut random) - 3.5).abs() < f32::EPSILON);
        assert!((provider.min() - 3.5).abs() < f32::EPSILON);
        assert!((provider.max() - 3.5).abs() < f32::EPSILON);
    }

    #[test]
    fn uniform_provider_samples_within_bounds() {
        let provider = FloatProvider::Uniform {
            min_inclusive: -1.0,
            max_exclusive: 1.0,
        };
        let mut random = rng();
        for _ in 0..50 {
            let value = provider.sample(&mut random);
            assert!((-1.0..1.0).contains(&value));
        }
    }

    #[test]
    fn deserializes_bare_number_as_constant() {
        let provider: FloatProvider = serde_json::from_str("2.5").expect("test value");
        assert!(matches!(provider, FloatProvider::Constant(v) if (v - 2.5).abs() < f32::EPSILON));
    }
}
