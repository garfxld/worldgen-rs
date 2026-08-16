use serde::Deserialize;

use crate::registry::REGISTRY;
use crate::registry::blocks::properties::Axis;
use crate::util::ident::BlockState;
use crate::util::inclusive_range::InclusiveRange;
use crate::util::noise::normal_noise::NormalNoise;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::util::rng::legacy_random::LegacyRandomSource;
use crate::util::value_providers::int::IntProvider;
use crate::worldgen::block_predicate::BlockPredicate;
use crate::worldgen::region::WorldGenRegion;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum BlockStateProvider {
    #[serde(rename = "minecraft:simple_state_provider")]
    Simple { state: BlockState },
    #[serde(rename = "minecraft:rotated_block_provider")]
    RotatedBlock { state: BlockState },
    #[serde(rename = "minecraft:weighted_state_provider")]
    Weighted { entries: Vec<WeightedState> },
    #[serde(rename = "minecraft:rule_based_state_provider")]
    RuleBased {
        #[serde(default)]
        fallback: Option<Box<BlockStateProvider>>,
        rules: Vec<StateRule>,
    },
    #[serde(rename = "minecraft:randomized_int_state_provider")]
    RandomizedInt {
        source: Box<BlockStateProvider>,
        property: String,
        values: IntProvider,
    },
    #[serde(rename = "minecraft:noise_provider")]
    Noise(NoiseProvider),
    #[serde(rename = "minecraft:noise_threshold_provider")]
    NoiseThreshold(NoiseThresholdProvider),
    #[serde(rename = "minecraft:dual_noise_provider")]
    DualNoise(DualNoiseProvider),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeightedState {
    #[serde(rename = "data")]
    pub state: BlockState,
    pub weight: i32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateRule {
    pub if_true: BlockPredicate,
    pub then: BlockStateProvider,
}

#[derive(Debug, Deserialize)]
#[serde(from = "RawNoiseProvider")]
pub struct NoiseProvider {
    pub noise: NormalNoise,
    pub scale: f32,
    pub states: Vec<BlockState>,
}

#[derive(Debug, Deserialize)]
#[serde(from = "RawNoiseThresholdProvider")]
pub struct NoiseThresholdProvider {
    pub noise: NormalNoise,
    pub scale: f32,
    pub threshold: f32,
    pub high_chance: f32,
    pub default_state: BlockState,
    pub low_states: Vec<BlockState>,
    pub high_states: Vec<BlockState>,
}

#[derive(Debug, Deserialize)]
#[serde(from = "RawDualNoiseProvider")]
pub struct DualNoiseProvider {
    pub noise: NormalNoise,
    pub scale: f32,
    pub slow_noise: NormalNoise,
    pub slow_scale: f32,
    pub variety: InclusiveRange,
    pub states: Vec<BlockState>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NoiseParameters {
    #[serde(rename = "firstOctave")]
    first_octave: i32,
    amplitudes: Vec<f64>,
}

impl NoiseParameters {
    fn build(&self, seed: i64) -> NormalNoise {
        let mut random = RandomSource::Legacy(LegacyRandomSource::from_seed(seed as u64));
        NormalNoise::create_from_random(&mut random, self.first_octave, &self.amplitudes)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNoiseProvider {
    seed: i64,
    noise: NoiseParameters,
    scale: f32,
    states: Vec<BlockState>,
}

impl From<RawNoiseProvider> for NoiseProvider {
    fn from(raw: RawNoiseProvider) -> Self {
        Self {
            noise: raw.noise.build(raw.seed),
            scale: raw.scale,
            states: raw.states,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNoiseThresholdProvider {
    seed: i64,
    noise: NoiseParameters,
    scale: f32,
    threshold: f32,
    high_chance: f32,
    default_state: BlockState,
    low_states: Vec<BlockState>,
    high_states: Vec<BlockState>,
}

impl From<RawNoiseThresholdProvider> for NoiseThresholdProvider {
    fn from(raw: RawNoiseThresholdProvider) -> Self {
        Self {
            noise: raw.noise.build(raw.seed),
            scale: raw.scale,
            threshold: raw.threshold,
            high_chance: raw.high_chance,
            default_state: raw.default_state,
            low_states: raw.low_states,
            high_states: raw.high_states,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDualNoiseProvider {
    seed: i64,
    noise: NoiseParameters,
    scale: f32,
    slow_noise: NoiseParameters,
    slow_scale: f32,
    variety: InclusiveRange,
    states: Vec<BlockState>,
}

impl From<RawDualNoiseProvider> for DualNoiseProvider {
    fn from(raw: RawDualNoiseProvider) -> Self {
        Self {
            noise: raw.noise.build(raw.seed),
            scale: raw.scale,
            slow_noise: raw.slow_noise.build(raw.seed),
            slow_scale: raw.slow_scale,
            variety: raw.variety,
            states: raw.states,
        }
    }
}

impl BlockStateProvider {
    pub fn get_state(
        &self,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        pos: BlockPos,
    ) -> BlockState {
        match self {
            Self::RuleBased { .. } => self
                .get_optional_state(region, random, pos)
                .unwrap_or_else(|| region.get_block(pos)),
            _ => self
                .get_optional_state(region, random, pos)
                .expect("only rule-based providers may yield no state"),
        }
    }

    pub fn get_optional_state(
        &self,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        pos: BlockPos,
    ) -> Option<BlockState> {
        match self {
            Self::Simple { state } => Some(*state),
            Self::RotatedBlock { state } => {
                let axis = match random.next_i32_bounded(3) {
                    0 => Axis::X,
                    1 => Axis::Y,
                    _ => Axis::Z,
                };
                Some(try_set_axis(*state, axis))
            }
            Self::Weighted { entries } => Some(random_weighted_state(entries, random)),
            Self::RuleBased { fallback, rules } => {
                for rule in rules {
                    if rule.if_true.test(region, pos) {
                        return Some(rule.then.get_state(region, random, pos));
                    }
                }
                fallback
                    .as_ref()
                    .map(|fallback| fallback.get_state(region, random, pos))
            }
            Self::RandomizedInt {
                source,
                property,
                values,
            } => {
                let state = source.get_state(region, random, pos);
                let value = values.sample(random);
                Some(set_int_property(state, property, value))
            }
            Self::Noise(provider) => Some(random_state_from_noise(
                &provider.states,
                noise_value(&provider.noise, pos, f64::from(provider.scale)),
            )),
            Self::NoiseThreshold(provider) => {
                let value = noise_value(&provider.noise, pos, f64::from(provider.scale));
                if value < f64::from(provider.threshold) {
                    Some(random_element(&provider.low_states, random))
                } else if random.next_f32() < provider.high_chance {
                    Some(random_element(&provider.high_states, random))
                } else {
                    Some(provider.default_state)
                }
            }
            Self::DualNoise(provider) => {
                let slow_at = |pos: BlockPos| {
                    noise_value(&provider.slow_noise, pos, f64::from(provider.slow_scale))
                };
                let local_variety = clamped_map(
                    slow_at(pos),
                    -1.0,
                    1.0,
                    f64::from(provider.variety.min_inclusive),
                    f64::from(provider.variety.max_inclusive + 1),
                ) as i32;
                let possible_states: Vec<BlockState> = (0..local_variety)
                    .map(|index| {
                        let offset = BlockPos::new(
                            pos.0.x + index * 54_545,
                            pos.0.y,
                            pos.0.z + index * 34_234,
                        );
                        random_state_from_noise(&provider.states, slow_at(offset))
                    })
                    .collect();
                Some(random_state_from_noise(
                    &possible_states,
                    noise_value(&provider.noise, pos, f64::from(provider.scale)),
                ))
            }
        }
    }
}

fn noise_value(noise: &NormalNoise, pos: BlockPos, scale: f64) -> f64 {
    noise.get_value(
        f64::from(pos.0.x) * scale,
        f64::from(pos.0.y) * scale,
        f64::from(pos.0.z) * scale,
    )
}

fn clamped_map(value: f64, from_min: f64, from_max: f64, to_min: f64, to_max: f64) -> f64 {
    let ratio = ((value - from_min) / (from_max - from_min)).clamp(0.0, 1.0);
    to_min + ratio * (to_max - to_min)
}

fn random_state_from_noise(states: &[BlockState], noise_value: f64) -> BlockState {
    let placement_value = ((1.0 + noise_value) / 2.0).clamp(0.0, 0.999_9);
    states[(placement_value * states.len() as f64) as usize]
}

fn random_element(states: &[BlockState], random: &mut RandomSource) -> BlockState {
    states[random.next_i32_bounded(states.len() as i32) as usize]
}

fn random_weighted_state(entries: &[WeightedState], random: &mut RandomSource) -> BlockState {
    let total_weight: i32 = entries.iter().map(|entry| entry.weight).sum();
    let mut roll = random.next_i32_bounded(total_weight);
    for entry in entries {
        roll -= entry.weight;
        if roll < 0 {
            return entry.state;
        }
    }
    panic!("weighted state provider exhausted its entries")
}

fn try_set_axis(state: BlockState, axis: Axis) -> BlockState {
    let block = state.get_block();
    REGISTRY
        .blocks
        .state_id_from_block_properties(block, &[("axis", axis_name(axis))])
        .unwrap_or(state)
}

const fn axis_name(axis: Axis) -> &'static str {
    match axis {
        Axis::X => "x",
        Axis::Y => "y",
        Axis::Z => "z",
    }
}

fn set_int_property(state: BlockState, property: &str, value: i32) -> BlockState {
    let block = state.get_block();
    let existing = REGISTRY.blocks.get_properties(state);
    let text = value.to_string();
    let overrides: Vec<(&str, &str)> = existing
        .iter()
        .map(|&(name, current)| {
            if name == property {
                (name, text.as_str())
            } else {
                (name, current)
            }
        })
        .collect();
    REGISTRY
        .blocks
        .state_id_from_block_properties(block, &overrides)
        .unwrap_or(state)
}

#[cfg(test)]
mod tests {
    use std::sync::Once;

    use serde_json::{from_value, json};

    use super::*;
    use crate::registry::Registries;
    use crate::registry::blocks::vanilla_blocks;
    use crate::util::rng::xoroshiro::XoroshiroRandomSource;

    fn init_global_registry() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let mut registries = Registries::new_vanilla().expect("vanilla datapack should load");
            registries.freeze();
            let _ = REGISTRY.init(registries);
        });
    }

    #[test]
    fn weighted_provider_consumes_one_draw_and_respects_weights() {
        init_global_registry();
        let provider = from_value::<BlockStateProvider>(json!({
            "type": "minecraft:weighted_state_provider",
            "entries": [
                {"data": {"Name": "minecraft:grass_block"}, "weight": 1},
                {"data": {"Name": "minecraft:podzol"}, "weight": 3}
            ]
        }))
        .expect("test value");

        let grass = REGISTRY
            .blocks
            .get_default_state_id(&vanilla_blocks::GRASS_BLOCK);
        let podzol = REGISTRY
            .blocks
            .get_default_state_id(&vanilla_blocks::PODZOL);

        let mut counts = (0, 0);
        for seed in 0..64u64 {
            let mut random = RandomSource::Xoroshiro(XoroshiroRandomSource::from_seed(seed));
            let state = random_weighted_state(
                match &provider {
                    BlockStateProvider::Weighted { entries } => entries,
                    _ => panic!("expected a weighted provider"),
                },
                &mut random,
            );
            if state == grass {
                counts.0 += 1;
            } else if state == podzol {
                counts.1 += 1;
            } else {
                panic!("unexpected state {state:?}");
            }
        }
        assert!(counts.0 > 0 && counts.1 > counts.0);
    }

    #[test]
    fn randomized_int_provider_sets_the_named_property() {
        init_global_registry();
        let provider = from_value::<BlockStateProvider>(json!({
            "type": "minecraft:randomized_int_state_provider",
            "source": {
                "type": "minecraft:simple_state_provider",
                "state": {"Name": "minecraft:sugar_cane"}
            },
            "property": "age",
            "values": {"type": "minecraft:constant", "value": 7}
        }))
        .expect("test value");

        let BlockStateProvider::RandomizedInt { source, .. } = &provider else {
            panic!("expected a randomized int provider");
        };
        let BlockStateProvider::Simple { state: base } = **source else {
            panic!("expected a simple source provider");
        };

        let with_age = set_int_property(base, "age", 7);
        assert_ne!(with_age, base);
        assert_eq!(
            REGISTRY
                .blocks
                .get_properties(with_age)
                .iter()
                .find(|(name, _)| *name == "age")
                .map(|&(_, value)| value),
            Some("7")
        );
    }
}
