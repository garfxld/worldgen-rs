use std::sync::{Arc, OnceLock};

use shulkr::world::block::BlockState;

use crate::util::rng::RandomFactory;
use crate::world::worldgen::densityfunction::function::Df;

use super::condition::{ConditionEvaluator, MaterialCondition};
use super::rule_context::{DensityGetter, MaterialRuleContext};

const ORE_RANDOM: &str = "minecraft:ore";

#[derive(Clone)]
pub struct OreVeinRule {
    pub ore_block: BlockState,
    pub raw_ore_block: BlockState,
    pub filler_block: BlockState,
    pub raw_ore_chance: f32,
    pub density: Df,
    pub richness: Df,
    pub filler_gap: Df,
}

#[derive(Clone)]
pub enum MaterialRule {
    Holder(Arc<OnceLock<MaterialRule>>),
    Block(BlockState),
    Bandlands,
    Sequence(Vec<MaterialRule>),
    Condition {
        if_true: MaterialCondition,
        then_run: Box<MaterialRule>,
    },
    OreVein(Box<OreVeinRule>),
}

pub struct OreVeinEvaluator {
    rule: OreVeinRule,
    density_sampler: DensityGetter,
    richness_sampler: DensityGetter,
    filler_gap_sampler: DensityGetter,
    random_factory: RandomFactory,
}

pub enum RuleEvaluator {
    Block(BlockState),
    Bandlands,
    Sequence(Vec<RuleEvaluator>),
    Condition {
        if_true: ConditionEvaluator,
        then_run: Box<RuleEvaluator>,
    },
    OreVein(Box<OreVeinEvaluator>),
}

impl MaterialRule {
    pub fn compile(&self, context: &mut MaterialRuleContext<'_, '_>) -> RuleEvaluator {
        match self {
            Self::Holder(holder) => holder
                .get()
                .expect("material rule holder bound")
                .compile(context),
            Self::Block(state) => RuleEvaluator::Block(*state),
            Self::Bandlands => RuleEvaluator::Bandlands,
            Self::Sequence(sequence) => {
                if sequence.len() == 1 {
                    return sequence[0].compile(context);
                }
                RuleEvaluator::Sequence(sequence.iter().map(|rule| rule.compile(context)).collect())
            }
            Self::Condition { if_true, then_run } => {
                let if_true = if_true.compile(context);
                let then_run = Box::new(then_run.compile(context));
                RuleEvaluator::Condition { if_true, then_run }
            }
            Self::OreVein(rule) => {
                let density_sampler = context.get_densities_in_chunk(&rule.density, true);
                let richness_sampler = context.get_densities_in_chunk(&rule.richness, true);
                let filler_gap_sampler = context.get_densities_in_chunk(&rule.filler_gap, false);
                let random_factory = context.get_or_create_random_factory(ORE_RANDOM);
                RuleEvaluator::OreVein(Box::new(OreVeinEvaluator {
                    rule: (**rule).clone(),
                    density_sampler,
                    richness_sampler,
                    filler_gap_sampler,
                    random_factory,
                }))
            }
        }
    }
}

impl RuleEvaluator {
    pub fn try_apply(
        &mut self,
        context: &mut MaterialRuleContext<'_, '_>,
        block_x: i32,
        block_y: i32,
        block_z: i32,
    ) -> Option<BlockState> {
        match self {
            Self::Block(state) => Some(*state),
            Self::Bandlands => Some(context.get_band(block_x, block_y, block_z)),
            Self::Sequence(sequence) => sequence
                .iter_mut()
                .find_map(|rule| rule.try_apply(context, block_x, block_y, block_z)),
            Self::Condition { if_true, then_run } => {
                if if_true.test(context) {
                    then_run.try_apply(context, block_x, block_y, block_z)
                } else {
                    None
                }
            }
            Self::OreVein(ore_vein) => ore_vein.try_apply(context, block_x, block_y, block_z),
        }
    }
}

impl OreVeinEvaluator {
    fn try_apply(
        &self,
        context: &mut MaterialRuleContext<'_, '_>,
        block_x: i32,
        block_y: i32,
        block_z: i32,
    ) -> Option<BlockState> {
        let density = context.get_density(&self.density_sampler);
        if density <= 0.0 {
            return None;
        }
        let mut random = self.random_factory.at(block_x, block_y, block_z);
        if random.next_f32() > density {
            return None;
        }
        let richness = context.get_density(&self.richness_sampler);
        if random.next_f32() < richness && context.get_density(&self.filler_gap_sampler) < 0.0 {
            Some(if random.next_f32() < self.rule.raw_ore_chance {
                self.rule.raw_ore_block
            } else {
                self.rule.ore_block
            })
        } else {
            Some(self.rule.filler_block)
        }
    }
}
