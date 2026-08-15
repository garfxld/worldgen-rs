use serde_json::Value;

use crate::registry::REGISTRY;
use crate::registry::blocks::properties::Axis;
use crate::util::ident::BlockState;
use crate::util::noise::normal_noise::NormalNoise;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::util::rng::legacy_random::LegacyRandomSource;
use crate::util::value_providers::int::IntProvider;
use crate::worldgen::block_predicate::BlockPredicate;
use crate::worldgen::json::{parse_block_state, parse_json, type_of, value_list};
use crate::worldgen::region::WorldGenRegion;

#[derive(Debug)]
pub enum BlockStateProvider {
    Simple(BlockState),
    RotatedBlock(BlockState),
    Weighted(Vec<WeightedState>),
    RuleBased {
        fallback: Option<Box<BlockStateProvider>>,
        rules: Vec<StateRule>,
    },
    RandomizedInt {
        source: Box<BlockStateProvider>,
        property: String,
        values: IntProvider,
    },
    Noise {
        noise: NormalNoise,
        scale: f32,
        states: Vec<BlockState>,
    },
    NoiseThreshold {
        noise: NormalNoise,
        scale: f32,
        threshold: f32,
        high_chance: f32,
        default_state: BlockState,
        low_states: Vec<BlockState>,
        high_states: Vec<BlockState>,
    },
    DualNoise {
        noise: NormalNoise,
        scale: f32,
        slow_noise: NormalNoise,
        slow_scale: f32,
        variety_min: i32,
        variety_max: i32,
        states: Vec<BlockState>,
    },
}

#[derive(Debug)]
pub struct WeightedState {
    pub state: BlockState,
    pub weight: i32,
}

#[derive(Debug)]
pub struct StateRule {
    pub if_true: BlockPredicate,
    pub then: BlockStateProvider,
}

fn parse_noise(seed: i64, parameters: &Value) -> NormalNoise {
    let first_octave = parameters["firstOctave"]
        .as_i64()
        .unwrap_or_else(|| panic!("noise parameters without firstOctave: {parameters}"))
        as i32;
    let amplitudes: Vec<f64> = value_list(&parameters["amplitudes"])
        .iter()
        .map(|amplitude| amplitude.as_f64().unwrap_or_default())
        .collect();
    let mut random = RandomSource::Legacy(LegacyRandomSource::from_seed(seed as u64));
    NormalNoise::create_from_random(&mut random, first_octave, &amplitudes)
}

fn parse_states(value: &Value) -> Vec<BlockState> {
    value_list(value).iter().map(parse_block_state).collect()
}

fn as_f32(value: &Value, key: &str) -> f32 {
    value[key]
        .as_f64()
        .unwrap_or_else(|| panic!("missing float field {key} in {value}")) as f32
}

impl BlockStateProvider {
    pub fn parse(value: &Value) -> Self {
        match type_of(value) {
            "minecraft:simple_state_provider" => Self::Simple(parse_block_state(&value["state"])),
            "minecraft:rotated_block_provider" => {
                Self::RotatedBlock(parse_block_state(&value["state"]))
            }
            "minecraft:weighted_state_provider" => Self::Weighted(
                value_list(&value["entries"])
                    .iter()
                    .map(|entry| WeightedState {
                        state: parse_block_state(&entry["data"]),
                        weight: entry["weight"].as_i64().unwrap_or(1) as i32,
                    })
                    .collect(),
            ),
            "minecraft:rule_based_state_provider" => Self::RuleBased {
                fallback: value
                    .get("fallback")
                    .map(|fallback| Box::new(Self::parse(fallback))),
                rules: value_list(&value["rules"])
                    .iter()
                    .map(|rule| StateRule {
                        if_true: BlockPredicate::parse(&rule["if_true"]),
                        then: Self::parse(&rule["then"]),
                    })
                    .collect(),
            },
            "minecraft:randomized_int_state_provider" => Self::RandomizedInt {
                source: Box::new(Self::parse(&value["source"])),
                property: value["property"]
                    .as_str()
                    .unwrap_or_else(|| panic!("randomized_int_state_provider without property"))
                    .to_owned(),
                values: parse_json(&value["values"]),
            },
            "minecraft:noise_provider" => {
                let seed = value["seed"].as_i64().unwrap_or_default();
                Self::Noise {
                    noise: parse_noise(seed, &value["noise"]),
                    scale: as_f32(value, "scale"),
                    states: parse_states(&value["states"]),
                }
            }
            "minecraft:noise_threshold_provider" => {
                let seed = value["seed"].as_i64().unwrap_or_default();
                Self::NoiseThreshold {
                    noise: parse_noise(seed, &value["noise"]),
                    scale: as_f32(value, "scale"),
                    threshold: as_f32(value, "threshold"),
                    high_chance: as_f32(value, "high_chance"),
                    default_state: parse_block_state(&value["default_state"]),
                    low_states: parse_states(&value["low_states"]),
                    high_states: parse_states(&value["high_states"]),
                }
            }
            "minecraft:dual_noise_provider" => {
                let seed = value["seed"].as_i64().unwrap_or_default();
                let variety = value_list(&value["variety"]);
                let variety_bound = |index: usize, fallback: i64| {
                    variety
                        .get(index)
                        .and_then(Value::as_i64)
                        .or_else(|| {
                            value["variety"][if index == 0 {
                                "min_inclusive"
                            } else {
                                "max_inclusive"
                            }]
                            .as_i64()
                        })
                        .unwrap_or(fallback) as i32
                };
                Self::DualNoise {
                    noise: parse_noise(seed, &value["noise"]),
                    scale: as_f32(value, "scale"),
                    slow_noise: parse_noise(seed, &value["slow_noise"]),
                    slow_scale: as_f32(value, "slow_scale"),
                    variety_min: variety_bound(0, 1),
                    variety_max: variety_bound(1, 1),
                    states: parse_states(&value["states"]),
                }
            }
            other => panic!("unknown state provider type {other}"),
        }
    }

    pub fn get_state(
        &self,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        pos: BlockPos,
    ) -> BlockState {
        match self {
            Self::RuleBased { .. } => self
                .get_optional_state(region, random, pos)
                .unwrap_or_else(|| region.get_block_state(pos)),
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
            Self::Simple(state) => Some(*state),
            Self::RotatedBlock(state) => {
                let axis = match random.next_i32_bounded(3) {
                    0 => Axis::X,
                    1 => Axis::Y,
                    _ => Axis::Z,
                };
                Some(try_set_axis(*state, axis))
            }
            Self::Weighted(entries) => Some(random_weighted_state(entries, random)),
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
            Self::Noise {
                noise,
                scale,
                states,
            } => Some(random_state_from_noise(
                states,
                noise_value(noise, pos, f64::from(*scale)),
            )),
            Self::NoiseThreshold {
                noise,
                scale,
                threshold,
                high_chance,
                default_state,
                low_states,
                high_states,
            } => {
                let value = noise_value(noise, pos, f64::from(*scale));
                if value < f64::from(*threshold) {
                    Some(random_element(low_states, random))
                } else if random.next_f32() < *high_chance {
                    Some(random_element(high_states, random))
                } else {
                    Some(*default_state)
                }
            }
            Self::DualNoise {
                noise,
                scale,
                slow_noise,
                slow_scale,
                variety_min,
                variety_max,
                states,
            } => {
                let slow_at = |pos: BlockPos| noise_value(slow_noise, pos, f64::from(*slow_scale));
                let local_variety = clamped_map(
                    slow_at(pos),
                    -1.0,
                    1.0,
                    f64::from(*variety_min),
                    f64::from(*variety_max + 1),
                ) as i32;
                let possible_states: Vec<BlockState> = (0..local_variety)
                    .map(|index| {
                        let offset = BlockPos::new(
                            pos.0.x + index * 54_545,
                            pos.0.y,
                            pos.0.z + index * 34_234,
                        );
                        random_state_from_noise(states, slow_at(offset))
                    })
                    .collect();
                Some(random_state_from_noise(
                    &possible_states,
                    noise_value(noise, pos, f64::from(*scale)),
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

    use serde_json::json;

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
        let provider = BlockStateProvider::parse(&json!({
            "type": "minecraft:weighted_state_provider",
            "entries": [
                {"data": {"Name": "minecraft:grass_block"}, "weight": 1},
                {"data": {"Name": "minecraft:podzol"}, "weight": 3}
            ]
        }));

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
                    BlockStateProvider::Weighted(entries) => entries,
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
        let provider = BlockStateProvider::parse(&json!({
            "type": "minecraft:randomized_int_state_provider",
            "source": {
                "type": "minecraft:simple_state_provider",
                "state": {"Name": "minecraft:sugar_cane"}
            },
            "property": "age",
            "values": {"type": "minecraft:constant", "value": 7}
        }));

        let BlockStateProvider::RandomizedInt { source, .. } = &provider else {
            panic!("expected a randomized int provider");
        };
        let BlockStateProvider::Simple(base) = **source else {
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
