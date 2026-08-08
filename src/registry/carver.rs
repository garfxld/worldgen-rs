use crate::registry::Registry;
use crate::util::ident::Identifier;
use crate::util::rng::legacy_random::LegacyRandomSource;
use crate::util::value_providers::{FloatProvider, HeightProvider, VerticalAnchor};

#[derive(Debug, Clone)]
pub struct CarverConfig {
    pub probability: f32,
    pub y: HeightProvider,
    pub y_scale: FloatProvider,
    pub lava_level: VerticalAnchor,
    pub replaceable_tag: Identifier,
}

#[derive(Debug, Clone)]
pub struct CaveCarverConfig {
    pub base: CarverConfig,
    pub horizontal_radius_multiplier: FloatProvider,
    pub vertical_radius_multiplier: FloatProvider,
    pub floor_level: FloatProvider,
}

#[derive(Debug, Clone, Copy)]
pub struct CanyonShapeConfig {
    pub distance_factor: FloatProvider,
    pub thickness: FloatProvider,
    pub width_smoothness: i32,
    pub horizontal_radius_factor: FloatProvider,
    pub vertical_radius_default_factor: f32,
    pub vertical_radius_center_factor: f32,
}

impl CanyonShapeConfig {
    #[must_use]
    pub fn init_width_factors(&self, gen_depth: i32, random: &mut LegacyRandomSource) -> Vec<f32> {
        let depth = gen_depth as usize;
        let mut factors = vec![0.0_f32; depth];
        let mut current = 1.0_f32;
        for (y_index, slot) in factors.iter_mut().enumerate() {
            if y_index == 0 || random.next_i32_bounded(self.width_smoothness) == 0 {
                current = 1.0 + random.next_f32() * random.next_f32();
            }
            *slot = current * current;
        }
        factors
    }

    #[must_use]
    pub fn update_vertical_radius(
        &self,
        random: &mut LegacyRandomSource,
        vertical_radius: f64,
        distance: f32,
        current_step: f32,
    ) -> f64 {
        let vertical_multiplier = 1.0_f32 - (0.5 - current_step / distance).abs() * 2.0;
        let factor = self.vertical_radius_default_factor
            + self.vertical_radius_center_factor * vertical_multiplier;
        let jitter = 0.75 + random.next_f32() * 0.25;
        f64::from(factor) * vertical_radius * f64::from(jitter)
    }
}

#[derive(Debug, Clone)]
pub struct CanyonCarverConfig {
    pub base: CarverConfig,
    pub vertical_rotation: FloatProvider,
    pub shape: CanyonShapeConfig,
}

#[derive(Debug, Clone)]
pub enum ConfiguredCarverKind {
    Cave(CaveCarverConfig),
    NetherCave(CaveCarverConfig),
    Canyon(CanyonCarverConfig),
}

#[derive(Debug)]
pub struct ConfiguredCarver {
    pub key: Identifier,
    pub kind: ConfiguredCarverKind,
}

impl ConfiguredCarver {
    #[must_use]
    pub const fn base(&self) -> &CarverConfig {
        match &self.kind {
            ConfiguredCarverKind::Cave(c) | ConfiguredCarverKind::NetherCave(c) => &c.base,
            ConfiguredCarverKind::Canyon(c) => &c.base,
        }
    }
}

pub type ConfiguredCarverRef = &'static ConfiguredCarver;
pub type ConfiguredCarverRegistry = Registry<ConfiguredCarver>;

pub mod vanilla_carvers {
    use super::{
        CanyonCarverConfig, CanyonShapeConfig, CarverConfig,
        CaveCarverConfig, ConfiguredCarver, ConfiguredCarverKind, ConfiguredCarverRegistry,
    };
    use crate::util::ident::Identifier;
    use crate::util::value_providers::{FloatProvider, HeightProvider, VerticalAnchor};

    pub static CAVE: ConfiguredCarver = ConfiguredCarver {
        key: Identifier::const_vanilla("cave"),
        kind: ConfiguredCarverKind::Cave(CaveCarverConfig {
            base: CarverConfig {
                probability: 0.15,
                y: HeightProvider::Uniform {
                    min_inclusive: VerticalAnchor::AboveBottom(8),
                    max_inclusive: VerticalAnchor::Absolute(180),
                },
                y_scale: FloatProvider::Uniform {
                    min_inclusive: 0.1,
                    max_exclusive: 0.9,
                },
                lava_level: VerticalAnchor::AboveBottom(8),
                replaceable_tag: Identifier::const_vanilla("overworld_carver_replaceables"),
            },
            horizontal_radius_multiplier: FloatProvider::Uniform {
                min_inclusive: 0.7,
                max_exclusive: 1.4,
            },
            vertical_radius_multiplier: FloatProvider::Uniform {
                min_inclusive: 0.8,
                max_exclusive: 1.3,
            },
            floor_level: FloatProvider::Uniform {
                min_inclusive: -1.0,
                max_exclusive: -0.4,
            },
        }),
    };

    pub static CAVE_EXTRA_UNDERGROUND: ConfiguredCarver = ConfiguredCarver {
        key: Identifier::const_vanilla("cave_extra_underground"),
        kind: ConfiguredCarverKind::Cave(CaveCarverConfig {
            base: CarverConfig {
                probability: 0.07,
                y: HeightProvider::Uniform {
                    min_inclusive: VerticalAnchor::AboveBottom(8),
                    max_inclusive: VerticalAnchor::Absolute(47),
                },
                y_scale: FloatProvider::Uniform {
                    min_inclusive: 0.1,
                    max_exclusive: 0.9,
                },
                lava_level: VerticalAnchor::AboveBottom(8),
                replaceable_tag: Identifier::const_vanilla("overworld_carver_replaceables"),
            },
            horizontal_radius_multiplier: FloatProvider::Uniform {
                min_inclusive: 0.7,
                max_exclusive: 1.4,
            },
            vertical_radius_multiplier: FloatProvider::Uniform {
                min_inclusive: 0.8,
                max_exclusive: 1.3,
            },
            floor_level: FloatProvider::Uniform {
                min_inclusive: -1.0,
                max_exclusive: -0.4,
            },
        }),
    };

    pub static NETHER_CAVE: ConfiguredCarver = ConfiguredCarver {
        key: Identifier::const_vanilla("nether_cave"),
        kind: ConfiguredCarverKind::NetherCave(CaveCarverConfig {
            base: CarverConfig {
                probability: 0.2,
                y: HeightProvider::Uniform {
                    min_inclusive: VerticalAnchor::Absolute(0),
                    max_inclusive: VerticalAnchor::BelowTop(1),
                },
                y_scale: FloatProvider::Constant(0.5),
                lava_level: VerticalAnchor::AboveBottom(10),
                replaceable_tag: Identifier::const_vanilla("nether_carver_replaceables"),
            },
            horizontal_radius_multiplier: FloatProvider::Constant(1.0),
            vertical_radius_multiplier: FloatProvider::Constant(1.0),
            floor_level: FloatProvider::Constant(-0.7),
        }),
    };

    pub static CANYON: ConfiguredCarver = ConfiguredCarver {
        key: Identifier::const_vanilla("canyon"),
        kind: ConfiguredCarverKind::Canyon(CanyonCarverConfig {
            base: CarverConfig {
                probability: 0.01,
                y: HeightProvider::Uniform {
                    min_inclusive: VerticalAnchor::Absolute(10),
                    max_inclusive: VerticalAnchor::Absolute(67),
                },
                y_scale: FloatProvider::Constant(3.0),
                lava_level: VerticalAnchor::AboveBottom(8),
                replaceable_tag: Identifier::const_vanilla("overworld_carver_replaceables"),
            },
            vertical_rotation: FloatProvider::Uniform {
                min_inclusive: -0.125,
                max_exclusive: 0.125,
            },
            shape: CanyonShapeConfig {
                distance_factor: FloatProvider::Uniform {
                    min_inclusive: 0.75,
                    max_exclusive: 1.0,
                },
                thickness: FloatProvider::Trapezoid {
                    min: 0.0,
                    max: 6.0,
                    plateau: 2.0,
                },
                width_smoothness: 3,
                horizontal_radius_factor: FloatProvider::Uniform {
                    min_inclusive: 0.75,
                    max_exclusive: 1.0,
                },
                vertical_radius_default_factor: 1.0,
                vertical_radius_center_factor: 0.0,
            },
        }),
    };

    pub fn register_carvers(registry: &mut ConfiguredCarverRegistry) {
        registry.register(CAVE.key.clone(), &CAVE);
        registry.register(CAVE_EXTRA_UNDERGROUND.key.clone(), &CAVE_EXTRA_UNDERGROUND);
        registry.register(NETHER_CAVE.key.clone(), &NETHER_CAVE);
        registry.register(CANYON.key.clone(), &CANYON);
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "carver registry tests assert required vanilla properties are present"
)]
#[expect(
    clippy::float_cmp,
    reason = "probability is an exact f32 literal transcribed from vanilla JSON, not a computed value"
)]
mod tests {
    use super::vanilla_carvers;
    use super::{ConfiguredCarverKind, ConfiguredCarverRegistry};
    use crate::util::ident::Identifier;
    use crate::util::value_providers::FloatProvider;

    fn create_test_registry() -> ConfiguredCarverRegistry {
        let mut registry = ConfiguredCarverRegistry::new();
        vanilla_carvers::register_carvers(&mut registry);
        registry.freeze();
        registry
    }

    #[test]
    fn cave_probability_matches_vanilla() {
        let registry = create_test_registry();
        let cave = registry.by_key(&Identifier::const_vanilla("cave")).unwrap();

        assert_eq!(cave.base().probability, 0.15);
        assert!(matches!(cave.kind, ConfiguredCarverKind::Cave(_)));
    }

    #[test]
    fn canyon_shape_thickness_is_trapezoid() {
        let registry = create_test_registry();
        let canyon = registry
            .by_key(&Identifier::const_vanilla("canyon"))
            .unwrap();

        let ConfiguredCarverKind::Canyon(config) = &canyon.kind else {
            panic!("expected canyon configuration");
        };
        assert!(matches!(
            config.shape.thickness,
            FloatProvider::Trapezoid { .. }
        ));
    }
}
