use std::sync::{Arc, OnceLock};

use rustc_hash::FxHashSet;

use crate::util::math::map;
use crate::util::rng::RandomFactory;
use crate::util::value_providers::VerticalAnchor;

use super::rule_context::MaterialRuleContext;

#[derive(Clone)]
pub enum MaterialCondition {
    Holder(Arc<OnceLock<MaterialCondition>>),
    Biome(Arc<FxHashSet<u16>>),
    NoiseThreshold {
        noise: String,
        min_threshold: f64,
        max_threshold: f64,
        is_3d: bool,
    },
    VerticalGradient {
        random_name: String,
        true_at_and_below: VerticalAnchor,
        false_at_and_above: VerticalAnchor,
    },
    YAbove {
        anchor: VerticalAnchor,
        surface_depth_multiplier: i32,
        add_stone_depth: bool,
    },
    Water {
        offset: i32,
        surface_depth_multiplier: i32,
        add_stone_depth: bool,
    },
    Temperature,
    Steep,
    Not(Box<MaterialCondition>),
    Hole,
    AbovePreliminarySurface,
    StoneDepth {
        offset: i32,
        add_surface_depth: bool,
        secondary_depth_range: i32,
        ceiling: bool,
    },
}

pub enum LazyXz {
    Hole,
    Steep,
}

pub enum LazyY {
    Biome(Arc<FxHashSet<u16>>),
    StoneDepth {
        offset: i32,
        add_surface_depth: bool,
        secondary_depth_range: i32,
        ceiling: bool,
    },
    VerticalGradient {
        true_at_and_below: i32,
        false_at_and_above: i32,
        random_factory: RandomFactory,
    },
    Water {
        offset: i32,
        surface_depth_multiplier: i32,
        add_stone_depth: bool,
    },
    YAbove {
        anchor: VerticalAnchor,
        surface_depth_multiplier: i32,
        add_stone_depth: bool,
    },
}

pub enum ConditionEvaluator {
    Constant(bool),
    Not(Box<ConditionEvaluator>),
    LazyXz {
        kind: LazyXz,
        last_update: i64,
        result: Option<bool>,
    },
    LazyY {
        kind: LazyY,
        last_update: i64,
        result: Option<bool>,
    },
    NoiseThreshold {
        sampler: usize,
        min_threshold: f64,
        max_threshold: f64,
    },
    AbovePreliminarySurface,
    Temperature,
}

impl MaterialCondition {
    pub fn compile(&self, context: &mut MaterialRuleContext<'_, '_>) -> ConditionEvaluator {
        let lazy_xz = |kind, context: &MaterialRuleContext<'_, '_>| ConditionEvaluator::LazyXz {
            kind,
            last_update: context.last_update_xz().wrapping_sub(1),
            result: None,
        };
        let lazy_y = |kind, context: &MaterialRuleContext<'_, '_>| ConditionEvaluator::LazyY {
            kind,
            last_update: context.last_update_y().wrapping_sub(1),
            result: None,
        };
        match self {
            Self::Holder(holder) => holder
                .get()
                .expect("material condition holder bound")
                .compile(context),
            Self::Biome(biomes) => {
                if let Some(possible_biomes) = context.possible_biomes() {
                    if !biomes.iter().any(|biome| possible_biomes.contains(biome)) {
                        return ConditionEvaluator::Constant(false);
                    }
                    if possible_biomes.iter().all(|biome| biomes.contains(biome)) {
                        return ConditionEvaluator::Constant(true);
                    }
                }
                lazy_y(LazyY::Biome(biomes.clone()), context)
            }
            Self::NoiseThreshold {
                noise,
                min_threshold,
                max_threshold,
                is_3d,
            } => ConditionEvaluator::NoiseThreshold {
                sampler: context.get_noise_sampler(noise, *is_3d),
                min_threshold: *min_threshold,
                max_threshold: *max_threshold,
            },
            Self::VerticalGradient {
                random_name,
                true_at_and_below,
                false_at_and_above,
            } => {
                let true_at_and_below = context.resolve_anchor_y(*true_at_and_below);
                let false_at_and_above = context.resolve_anchor_y(*false_at_and_above);
                let random_factory = context.get_or_create_random_factory(random_name);
                lazy_y(
                    LazyY::VerticalGradient {
                        true_at_and_below,
                        false_at_and_above,
                        random_factory,
                    },
                    context,
                )
            }
            Self::YAbove {
                anchor,
                surface_depth_multiplier,
                add_stone_depth,
            } => lazy_y(
                LazyY::YAbove {
                    anchor: *anchor,
                    surface_depth_multiplier: *surface_depth_multiplier,
                    add_stone_depth: *add_stone_depth,
                },
                context,
            ),
            Self::Water {
                offset,
                surface_depth_multiplier,
                add_stone_depth,
            } => lazy_y(
                LazyY::Water {
                    offset: *offset,
                    surface_depth_multiplier: *surface_depth_multiplier,
                    add_stone_depth: *add_stone_depth,
                },
                context,
            ),
            Self::Temperature => ConditionEvaluator::Temperature,
            Self::Steep => lazy_xz(LazyXz::Steep, context),
            Self::Not(target) => ConditionEvaluator::Not(Box::new(target.compile(context))),
            Self::Hole => lazy_xz(LazyXz::Hole, context),
            Self::AbovePreliminarySurface => ConditionEvaluator::AbovePreliminarySurface,
            Self::StoneDepth {
                offset,
                add_surface_depth,
                secondary_depth_range,
                ceiling,
            } => lazy_y(
                LazyY::StoneDepth {
                    offset: *offset,
                    add_surface_depth: *add_surface_depth,
                    secondary_depth_range: *secondary_depth_range,
                    ceiling: *ceiling,
                },
                context,
            ),
        }
    }
}

impl LazyXz {
    const fn compute(&self, context: &MaterialRuleContext<'_, '_>) -> bool {
        match self {
            Self::Hole => context.surface_depth() <= 0,
            Self::Steep => context.surface_gradient_x() <= -4 || context.surface_gradient_z() >= 4,
        }
    }
}

impl LazyY {
    fn compute(&self, context: &mut MaterialRuleContext<'_, '_>) -> bool {
        match self {
            Self::Biome(biomes) => biomes.contains(&context.get_biome().id()),
            Self::StoneDepth {
                offset,
                add_surface_depth,
                secondary_depth_range,
                ceiling,
            } => {
                let stone_depth = if *ceiling {
                    context.stone_depth_below()
                } else {
                    context.stone_depth_above()
                };
                let surface_depth = if *add_surface_depth {
                    context.surface_depth()
                } else {
                    0
                };
                let secondary_surface_depth = if *secondary_depth_range == 0 {
                    0
                } else {
                    map(
                        context.get_surface_secondary(),
                        -1.0,
                        1.0,
                        0.0,
                        f64::from(*secondary_depth_range),
                    ) as i32
                };
                stone_depth <= 1 + offset + surface_depth + secondary_surface_depth
            }
            Self::VerticalGradient {
                true_at_and_below,
                false_at_and_above,
                random_factory,
            } => {
                let block_y = context.block_y();
                if block_y <= *true_at_and_below {
                    return true;
                }
                if block_y >= *false_at_and_above {
                    return false;
                }
                let probability = map(
                    f64::from(block_y),
                    f64::from(*true_at_and_below),
                    f64::from(*false_at_and_above),
                    1.0,
                    0.0,
                );
                let mut random = random_factory.at(context.block_x(), block_y, context.block_z());
                f64::from(random.next_f32()) < probability
            }
            Self::Water {
                offset,
                surface_depth_multiplier,
                add_stone_depth,
            } => {
                context.water_height() == i32::MIN
                    || context.block_y()
                        + if *add_stone_depth {
                            context.stone_depth_above()
                        } else {
                            0
                        }
                        >= context.water_height()
                            + offset
                            + context.surface_depth() * surface_depth_multiplier
            }
            Self::YAbove {
                anchor,
                surface_depth_multiplier,
                add_stone_depth,
            } => {
                context.block_y()
                    + if *add_stone_depth {
                        context.stone_depth_above()
                    } else {
                        0
                    }
                    >= context.resolve_anchor_y(*anchor)
                        + context.surface_depth() * surface_depth_multiplier
            }
        }
    }
}

impl ConditionEvaluator {
    pub fn test(&mut self, context: &mut MaterialRuleContext<'_, '_>) -> bool {
        match self {
            Self::Constant(value) => *value,
            Self::Not(target) => !target.test(context),
            Self::LazyXz {
                kind,
                last_update,
                result,
            } => {
                let last_context_update = context.last_update_xz();
                if last_context_update == *last_update {
                    result.expect("Update triggered but the result is null")
                } else {
                    *last_update = last_context_update;
                    let value = kind.compute(context);
                    *result = Some(value);
                    value
                }
            }
            Self::LazyY {
                kind,
                last_update,
                result,
            } => {
                let last_context_update = context.last_update_y();
                if last_context_update == *last_update {
                    result.expect("Update triggered but the result is null")
                } else {
                    *last_update = last_context_update;
                    let value = kind.compute(context);
                    *result = Some(value);
                    value
                }
            }
            Self::NoiseThreshold {
                sampler,
                min_threshold,
                max_threshold,
            } => {
                let value = context.sample_noise(*sampler);
                value >= *min_threshold && value <= *max_threshold
            }
            Self::AbovePreliminarySurface => context.block_y() >= context.get_min_surface_level(),
            Self::Temperature => context.cold_enough_to_snow(),
        }
    }
}
