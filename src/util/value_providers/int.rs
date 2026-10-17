use serde::Deserialize;

use crate::util::rng::RandomSource;

#[derive(Debug, Clone, Deserialize)]
#[serde(from = "RawIntProvider")]
pub enum IntProvider {
    Constant(i32),
    Uniform {
        min_inclusive: i32,
        max_inclusive: i32,
    },
    BiasedToBottom {
        min_inclusive: i32,
        max_inclusive: i32,
    },
    VeryBiasedToBottom {
        min_inclusive: i32,
        max_inclusive: i32,
    },
    Trapezoid {
        min: i32,
        max: i32,
        plateau: i32,
    },
    ClampedNormal {
        mean: f32,
        deviation: f32,
        min_inclusive: i32,
        max_inclusive: i32,
    },
    Clamped {
        source: Box<IntProvider>,
        min_inclusive: i32,
        max_inclusive: i32,
    },
    WeightedList {
        distribution: Vec<WeightedIntProvider>,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct WeightedIntProvider {
    pub data: IntProvider,
    pub weight: i32,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(try_from = "RawUniformIntProvider")]
pub struct UniformIntProvider {
    pub min_inclusive: i32,
    pub max_inclusive: i32,
}

impl UniformIntProvider {
    pub fn sample(self, random: &mut RandomSource) -> i32 {
        random.next_i32_between(self.min_inclusive, self.max_inclusive)
    }

    pub const fn with_max_inclusive(self, max_inclusive: i32) -> Self {
        Self {
            min_inclusive: self.min_inclusive,
            max_inclusive,
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum RawUniformIntProvider {
    Tagged(TaggedUniformIntProvider),
    Range {
        min_inclusive: i32,
        max_inclusive: i32,
    },
}

#[derive(Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum TaggedUniformIntProvider {
    #[serde(rename = "minecraft:uniform")]
    Uniform {
        min_inclusive: i32,
        max_inclusive: i32,
    },
}

impl TryFrom<RawUniformIntProvider> for UniformIntProvider {
    type Error = &'static str;

    fn try_from(raw: RawUniformIntProvider) -> Result<Self, Self::Error> {
        let (min_inclusive, max_inclusive) = match raw {
            RawUniformIntProvider::Range {
                min_inclusive,
                max_inclusive,
            }
            | RawUniformIntProvider::Tagged(TaggedUniformIntProvider::Uniform {
                min_inclusive,
                max_inclusive,
            }) => (min_inclusive, max_inclusive),
        };

        if min_inclusive > max_inclusive {
            return Err("UniformIntProvider min_inclusive exceeds max_inclusive");
        }

        Ok(Self {
            min_inclusive,
            max_inclusive,
        })
    }
}

impl IntProvider {
    pub fn min(&self) -> i32 {
        match self {
            Self::Constant(value) => *value,
            Self::Uniform { min_inclusive, .. }
            | Self::BiasedToBottom { min_inclusive, .. }
            | Self::VeryBiasedToBottom { min_inclusive, .. }
            | Self::Clamped { min_inclusive, .. }
            | Self::ClampedNormal { min_inclusive, .. } => *min_inclusive,
            Self::Trapezoid { min, .. } => *min,
            Self::WeightedList { distribution } => {
                let mut min = 0;
                let mut found = false;
                for entry in distribution {
                    let value = entry.data.min();
                    if !found || value < min {
                        min = value;
                        found = true;
                    }
                }
                min
            }
        }
    }

    pub fn max(&self) -> i32 {
        match self {
            Self::Constant(value) => *value,
            Self::Uniform { max_inclusive, .. }
            | Self::BiasedToBottom { max_inclusive, .. }
            | Self::VeryBiasedToBottom { max_inclusive, .. }
            | Self::Clamped { max_inclusive, .. }
            | Self::ClampedNormal { max_inclusive, .. } => *max_inclusive,
            Self::Trapezoid { max, .. } => *max,
            Self::WeightedList { distribution } => {
                let mut max = 0;
                let mut found = false;
                for entry in distribution {
                    let value = entry.data.max();
                    if !found || value > max {
                        max = value;
                        found = true;
                    }
                }
                max
            }
        }
    }

    pub fn sample(&self, random: &mut RandomSource) -> i32 {
        match self {
            Self::Constant(v) => *v,
            Self::Uniform {
                min_inclusive,
                max_inclusive,
            } => random.next_i32_between(*min_inclusive, *max_inclusive),
            Self::BiasedToBottom {
                min_inclusive,
                max_inclusive,
            } => {
                let span = *max_inclusive - *min_inclusive + 1;
                let bound = random.next_i32_bounded(span) + 1;
                *min_inclusive + random.next_i32_bounded(bound)
            }
            Self::VeryBiasedToBottom {
                min_inclusive,
                max_inclusive,
            } => {
                let inner = random.next_i32_bounded(*max_inclusive - *min_inclusive + 1) + 1;
                let middle = random.next_i32_bounded(inner) + 1;
                *min_inclusive + random.next_i32_bounded(middle)
            }
            Self::Trapezoid { min, max, plateau } => {
                if *plateau == 0 && *max == -*min {
                    random.next_i32_bounded(*max + 1) - random.next_i32_bounded(*max + 1)
                } else {
                    let range = *max - *min;
                    if *plateau >= range {
                        random.next_i32_between(*min, *max)
                    } else {
                        let plateau_start = (range - *plateau) / 2;
                        let plateau_end = range - plateau_start;
                        *min + random.next_i32_between(0, plateau_end)
                            + random.next_i32_between(0, plateau_start)
                    }
                }
            }
            Self::ClampedNormal {
                mean,
                deviation,
                min_inclusive,
                max_inclusive,
            } => {
                let sample = *mean + *deviation * random.next_gaussian() as f32;
                sample.clamp(*min_inclusive as f32, *max_inclusive as f32) as i32
            }
            Self::Clamped {
                source,
                min_inclusive,
                max_inclusive,
            } => source.sample(random).clamp(*min_inclusive, *max_inclusive),
            Self::WeightedList { distribution } => {
                let total_weight: i32 = distribution.iter().map(|entry| entry.weight).sum();
                if total_weight <= 0 {
                    return 0;
                }
                let mut target = random.next_i32_bounded(total_weight);
                for entry in distribution {
                    target -= entry.weight;
                    if target < 0 {
                        return entry.data.sample(random);
                    }
                }
                0
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum RawIntProvider {
    Constant(i32),
    Tagged(TaggedIntProvider),
}

#[derive(Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum TaggedIntProvider {
    #[serde(rename = "minecraft:constant")]
    Constant { value: i32 },
    #[serde(rename = "minecraft:uniform")]
    Uniform {
        min_inclusive: i32,
        max_inclusive: i32,
    },
    #[serde(rename = "minecraft:biased_to_bottom")]
    BiasedToBottom {
        min_inclusive: i32,
        max_inclusive: i32,
    },
    #[serde(rename = "minecraft:very_biased_to_bottom")]
    VeryBiasedToBottom {
        min_inclusive: i32,
        max_inclusive: i32,
    },
    #[serde(rename = "minecraft:trapezoid")]
    Trapezoid { min: i32, max: i32, plateau: i32 },
    #[serde(rename = "minecraft:clamped_normal")]
    ClampedNormal {
        mean: f32,
        deviation: f32,
        min_inclusive: i32,
        max_inclusive: i32,
    },
    #[serde(rename = "minecraft:clamped")]
    Clamped {
        source: Box<IntProvider>,
        min_inclusive: i32,
        max_inclusive: i32,
    },
    #[serde(rename = "minecraft:weighted_list")]
    WeightedList {
        distribution: Vec<WeightedIntProvider>,
    },
}

impl From<RawIntProvider> for IntProvider {
    fn from(raw: RawIntProvider) -> Self {
        let tagged = match raw {
            RawIntProvider::Constant(value) => return Self::Constant(value),
            RawIntProvider::Tagged(tagged) => tagged,
        };
        match tagged {
            TaggedIntProvider::Constant { value } => Self::Constant(value),
            TaggedIntProvider::Uniform {
                min_inclusive,
                max_inclusive,
            } => Self::Uniform {
                min_inclusive,
                max_inclusive,
            },
            TaggedIntProvider::BiasedToBottom {
                min_inclusive,
                max_inclusive,
            } => Self::BiasedToBottom {
                min_inclusive,
                max_inclusive,
            },
            TaggedIntProvider::VeryBiasedToBottom {
                min_inclusive,
                max_inclusive,
            } => Self::VeryBiasedToBottom {
                min_inclusive,
                max_inclusive,
            },
            TaggedIntProvider::Trapezoid { min, max, plateau } => {
                Self::Trapezoid { min, max, plateau }
            }
            TaggedIntProvider::ClampedNormal {
                mean,
                deviation,
                min_inclusive,
                max_inclusive,
            } => Self::ClampedNormal {
                mean,
                deviation,
                min_inclusive,
                max_inclusive,
            },
            TaggedIntProvider::Clamped {
                source,
                min_inclusive,
                max_inclusive,
            } => Self::Clamped {
                source,
                min_inclusive,
                max_inclusive,
            },
            TaggedIntProvider::WeightedList { distribution } => Self::WeightedList { distribution },
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
        let provider = IntProvider::Constant(42);
        let mut random = rng();
        assert_eq!(provider.sample(&mut random), 42);
        assert_eq!(provider.min(), 42);
        assert_eq!(provider.max(), 42);
    }

    #[test]
    fn uniform_provider_samples_within_bounds() {
        let provider = IntProvider::Uniform {
            min_inclusive: 5,
            max_inclusive: 10,
        };
        let mut random = rng();
        for _ in 0..50 {
            let value = provider.sample(&mut random);
            assert!((5..=10).contains(&value));
        }
    }

    #[test]
    fn clamped_provider_never_exceeds_its_bounds() {
        let provider = IntProvider::Clamped {
            source: Box::new(IntProvider::Uniform {
                min_inclusive: -100,
                max_inclusive: 100,
            }),
            min_inclusive: -3,
            max_inclusive: 3,
        };
        let mut random = rng();
        for _ in 0..50 {
            let value = provider.sample(&mut random);
            assert!((-3..=3).contains(&value));
        }
    }

    #[test]
    fn weighted_list_picks_only_present_alternatives() {
        let provider = IntProvider::WeightedList {
            distribution: vec![WeightedIntProvider {
                data: IntProvider::Constant(7),
                weight: 1,
            }],
        };
        let mut random = rng();
        assert_eq!(provider.sample(&mut random), 7);
    }

    #[test]
    fn deserializes_bare_number_as_constant() {
        let provider: IntProvider = serde_json::from_str("5").expect("test value");
        assert!(matches!(provider, IntProvider::Constant(5)));
    }

    #[test]
    fn deserializes_tagged_uniform_variant() {
        let provider: IntProvider = serde_json::from_str(
            r#"{"type": "minecraft:uniform", "min_inclusive": 1, "max_inclusive": 4}"#,
        )
        .expect("test value");
        assert_eq!(provider.min(), 1);
        assert_eq!(provider.max(), 4);
    }
}
