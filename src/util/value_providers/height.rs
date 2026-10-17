use serde::Deserialize;

use crate::util::rng::RandomSource;
use crate::world::worldgen::worldgen_context::WorldGenContext;

use super::VerticalAnchor;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(from = "RawHeightProvider")]
pub enum HeightProvider {
    Constant(VerticalAnchor),
    Uniform {
        min_inclusive: VerticalAnchor,
        max_inclusive: VerticalAnchor,
    },
    Trapezoid {
        min_inclusive: VerticalAnchor,
        max_inclusive: VerticalAnchor,
        plateau: i32,
    },
    BiasedToBottom {
        min_inclusive: VerticalAnchor,
        max_inclusive: VerticalAnchor,
        inner: i32,
    },
    VeryBiasedToBottom {
        min_inclusive: VerticalAnchor,
        max_inclusive: VerticalAnchor,
        inner: i32,
    },
}

impl HeightProvider {
    pub fn sample(self, random: &mut RandomSource, context: &WorldGenContext) -> i32 {
        match self {
            Self::Constant(anchor) => anchor.resolve_y(context),
            Self::Uniform {
                min_inclusive,
                max_inclusive,
            } => {
                let min = min_inclusive.resolve_y(context);
                let max = max_inclusive.resolve_y(context);
                if min > max {
                    min
                } else {
                    random.next_i32_between(min, max)
                }
            }
            Self::Trapezoid {
                min_inclusive,
                max_inclusive,
                plateau,
            } => {
                let min = min_inclusive.resolve_y(context);
                let max = max_inclusive.resolve_y(context);
                if min > max {
                    min
                } else {
                    let range = max - min;
                    if plateau >= range {
                        random.next_i32_between(min, max)
                    } else {
                        let plateau_start = (range - plateau) / 2;
                        let plateau_end = range - plateau_start;
                        min + random.next_i32_between(0, plateau_end)
                            + random.next_i32_between(0, plateau_start)
                    }
                }
            }
            Self::BiasedToBottom {
                min_inclusive,
                max_inclusive,
                inner,
            } => {
                let min = min_inclusive.resolve_y(context);
                let max = max_inclusive.resolve_y(context);
                if max - min - inner < 0 {
                    min
                } else {
                    let limit = random.next_i32_bounded(max - min - inner + 1);
                    random.next_i32_bounded(limit + inner) + min
                }
            }
            Self::VeryBiasedToBottom {
                min_inclusive,
                max_inclusive,
                inner,
            } => {
                let min = min_inclusive.resolve_y(context);
                let max = max_inclusive.resolve_y(context);
                if max - min - inner < 0 {
                    min
                } else {
                    let upper_inclusive = random.next_i32_between(min + inner, max);
                    let biased_upper_inclusive = random.next_i32_between(min, upper_inclusive - 1);
                    random.next_i32_between(min, biased_upper_inclusive - 1 + inner)
                }
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum RawHeightProvider {
    Tagged(TaggedHeightProvider),
    Constant(VerticalAnchor),
}

#[derive(Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum TaggedHeightProvider {
    #[serde(rename = "minecraft:constant")]
    Constant { value: VerticalAnchor },
    #[serde(rename = "minecraft:uniform")]
    Uniform {
        min_inclusive: VerticalAnchor,
        max_inclusive: VerticalAnchor,
    },
    #[serde(rename = "minecraft:trapezoid")]
    Trapezoid {
        min_inclusive: VerticalAnchor,
        max_inclusive: VerticalAnchor,
        #[serde(default)]
        plateau: i32,
    },
    #[serde(rename = "minecraft:biased_to_bottom")]
    BiasedToBottom {
        min_inclusive: VerticalAnchor,
        max_inclusive: VerticalAnchor,
        #[serde(default = "default_inner")]
        inner: i32,
    },
    #[serde(rename = "minecraft:very_biased_to_bottom")]
    VeryBiasedToBottom {
        min_inclusive: VerticalAnchor,
        max_inclusive: VerticalAnchor,
        #[serde(default = "default_inner")]
        inner: i32,
    },
}

const fn default_inner() -> i32 {
    1
}

impl From<RawHeightProvider> for HeightProvider {
    fn from(raw: RawHeightProvider) -> Self {
        let tagged = match raw {
            RawHeightProvider::Constant(anchor) => return Self::Constant(anchor),
            RawHeightProvider::Tagged(tagged) => tagged,
        };
        match tagged {
            TaggedHeightProvider::Constant { value } => Self::Constant(value),
            TaggedHeightProvider::Uniform {
                min_inclusive,
                max_inclusive,
            } => Self::Uniform {
                min_inclusive,
                max_inclusive,
            },
            TaggedHeightProvider::Trapezoid {
                min_inclusive,
                max_inclusive,
                plateau,
            } => Self::Trapezoid {
                min_inclusive,
                max_inclusive,
                plateau,
            },
            TaggedHeightProvider::BiasedToBottom {
                min_inclusive,
                max_inclusive,
                inner,
            } => Self::BiasedToBottom {
                min_inclusive,
                max_inclusive,
                inner,
            },
            TaggedHeightProvider::VeryBiasedToBottom {
                min_inclusive,
                max_inclusive,
                inner,
            } => Self::VeryBiasedToBottom {
                min_inclusive,
                max_inclusive,
                inner,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONTEXT: WorldGenContext = WorldGenContext::new(-64, 384, 63, -64, 384);
    use crate::util::rng::xoroshiro::XoroshiroRandomSource;

    fn rng() -> RandomSource {
        RandomSource::Xoroshiro(XoroshiroRandomSource::from_seed(0))
    }

    #[test]
    fn constant_provider_resolves_the_anchor() {
        let provider = HeightProvider::Constant(VerticalAnchor::Absolute(100));
        let mut random = rng();
        assert_eq!(provider.sample(&mut random, &CONTEXT), 100);
    }

    #[test]
    fn uniform_provider_samples_within_resolved_bounds() {
        let provider = HeightProvider::Uniform {
            min_inclusive: VerticalAnchor::AboveBottom(0),
            max_inclusive: VerticalAnchor::AboveBottom(10),
        };
        let mut random = rng();
        for _ in 0..50 {
            let value = provider.sample(&mut random, &CONTEXT);
            assert!((-64..=-54).contains(&value));
        }
    }

    #[test]
    fn inverted_range_falls_back_to_min() {
        let provider = HeightProvider::Uniform {
            min_inclusive: VerticalAnchor::AboveBottom(10),
            max_inclusive: VerticalAnchor::AboveBottom(0),
        };
        let mut random = rng();
        assert_eq!(provider.sample(&mut random, &CONTEXT), -54);
    }

    #[test]
    fn deserializes_bare_anchor_as_constant() {
        let provider: HeightProvider =
            serde_json::from_str(r#"{"absolute": 5}"#).expect("test value");
        assert!(matches!(
            provider,
            HeightProvider::Constant(VerticalAnchor::Absolute(5))
        ));
    }
}
