use std::sync::{Arc, LazyLock, OnceLock};

use rustc_hash::FxHashMap;
use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use shulkr::util::{Axis, BlockPosition, Direction, Key};
use shulkr::world::block::{BlockState, Properties};

use crate::datapack::{self, collect_json_files, parse_id, path_to_id, read_and_parse_normalized};
use crate::util::inclusive_range::InclusiveRange;
use crate::util::math::clamped_lerp;
use crate::util::rng::RandomSource;
use crate::util::rng::legacy_random::LegacyRandomSource;
use crate::util::value_providers::int::IntProvider;
use crate::world::worldgen::block_predicate::{BlockPredicate, HolderSet};
use crate::world::worldgen::region::WorldGenRegion;
use crate::world::worldgen::synth::Noise;
use crate::world::worldgen::synth::normal_noise::{NormalNoise, Parameters};
use std::cell;
use std::cell::RefCell;

static PROVIDERS: LazyLock<FxHashMap<Key, Arc<OnceLock<BlockStateProvider>>>> =
    LazyLock::new(|| {
        let pack = datapack::active();
        let mut holders = FxHashMap::default();
        let mut sources = Vec::new();
        for (namespace, dir) in pack.namespaced_dirs("worldgen/block_state_provider") {
            for file in collect_json_files(&dir) {
                let key = parse_id(&path_to_id(&namespace, &file, &dir));
                holders.insert(key.clone(), Arc::new(OnceLock::new()));
                sources.push((key, file));
            }
        }
        let holders_ref = &holders;
        PENDING.with(|pending| pending.replace(Some(holders_ref.clone())));
        for (key, file) in sources {
            let provider: BlockStateProvider = read_and_parse_normalized(&file)
                .unwrap_or_else(|e| panic!("failed to load {}: {e}", file.display()));
            let _ = holders_ref[&key].set(provider);
        }
        PENDING.with(|pending| pending.replace(None));
        holders
    });

thread_local! {
    static PENDING: RefCell<Option<FxHashMap<Key, Arc<OnceLock<BlockStateProvider>>>>> =
        const { cell::RefCell::new(None) };
}

fn provider_holder(key: &Key) -> Option<Arc<OnceLock<BlockStateProvider>>> {
    if let Some(holder) = PENDING.with(|pending| {
        pending
            .borrow()
            .as_ref()
            .and_then(|map| map.get(key).cloned())
    }) {
        return Some(holder);
    }
    PROVIDERS.get(key).cloned()
}

#[derive(Debug, Clone)]
pub enum ProviderHolder {
    Direct(Box<BlockStateProvider>),
    Reference(Key, Arc<OnceLock<BlockStateProvider>>),
}

impl ProviderHolder {
    pub fn value(&self) -> &BlockStateProvider {
        match self {
            Self::Direct(provider) => provider,
            Self::Reference(key, holder) => holder
                .get()
                .unwrap_or_else(|| panic!("block state provider {key} is unbound")),
        }
    }

    pub fn get_state(
        &self,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        pos: BlockPosition,
    ) -> BlockState {
        self.value().get_state(region, random, pos)
    }

    pub fn get_optional_state(
        &self,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        pos: BlockPosition,
    ) -> Option<BlockState> {
        self.value().get_optional_state(region, random, pos)
    }
}

impl<'de> Deserialize<'de> for ProviderHolder {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        if let Some(id) = value.as_str() {
            let key = parse_id(id);
            let holder = provider_holder(&key)
                .ok_or_else(|| D::Error::custom(format!("unknown block state provider {key}")))?;
            return Ok(Self::Reference(key, holder));
        }
        BlockStateProvider::deserialize(value)
            .map(|provider| Self::Direct(Box::new(provider)))
            .map_err(D::Error::custom)
    }
}

#[derive(Debug, Clone)]
pub enum BlockStateProvider {
    CopyProperties {
        source: ProviderHolder,
    },
    DualNoise(Box<DualNoiseProvider>),
    Noise(Box<NoiseProvider>),
    NoiseThreshold(Box<NoiseThresholdProvider>),
    RandomBlock {
        blocks: HolderSet,
    },
    RandomizedInt {
        source: ProviderHolder,
        property: String,
        values: IntProvider,
    },
    Rotated {
        state: ProviderHolder,
        direction: Option<Direction>,
    },
    RuleBased {
        fallback: Option<ProviderHolder>,
        rules: Vec<StateRule>,
    },
    Simple {
        state: BlockState,
    },
    Weighted {
        entries: Vec<WeightedState>,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
enum TypedProvider {
    #[serde(rename = "minecraft:copy_properties")]
    CopyProperties { source: ProviderHolder },
    #[serde(rename = "minecraft:dual_noise")]
    DualNoise(RawDualNoiseProvider),
    #[serde(rename = "minecraft:noise")]
    Noise(RawNoiseProvider),
    #[serde(rename = "minecraft:noise_threshold")]
    NoiseThreshold(RawNoiseThresholdProvider),
    #[serde(rename = "minecraft:random_block")]
    RandomBlock { blocks: HolderSet },
    #[serde(rename = "minecraft:randomized_int")]
    RandomizedInt {
        source: ProviderHolder,
        property: String,
        values: IntProvider,
    },
    #[serde(rename = "minecraft:rotated")]
    Rotated {
        state: ProviderHolder,
        #[serde(default)]
        direction: Option<Direction>,
    },
    #[serde(rename = "minecraft:rule_based")]
    RuleBased {
        #[serde(default)]
        fallback: Option<ProviderHolder>,
        rules: Vec<StateRule>,
    },
    #[serde(rename = "minecraft:simple")]
    Simple { state: BlockState },
    #[serde(rename = "minecraft:weighted")]
    Weighted { entries: Vec<WeightedState> },
}

impl<'de> Deserialize<'de> for BlockStateProvider {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        if value.get("type").is_none() {
            let state = BlockState::deserialize(value).map_err(D::Error::custom)?;
            return Ok(Self::Simple { state });
        }
        let typed = TypedProvider::deserialize(value).map_err(D::Error::custom)?;
        Ok(match typed {
            TypedProvider::CopyProperties { source } => Self::CopyProperties { source },
            TypedProvider::DualNoise(raw) => Self::DualNoise(Box::new(raw.into())),
            TypedProvider::Noise(raw) => Self::Noise(Box::new(raw.into())),
            TypedProvider::NoiseThreshold(raw) => Self::NoiseThreshold(Box::new(raw.into())),
            TypedProvider::RandomBlock { blocks } => Self::RandomBlock { blocks },
            TypedProvider::RandomizedInt {
                source,
                property,
                values,
            } => Self::RandomizedInt {
                source,
                property,
                values,
            },
            TypedProvider::Rotated { state, direction } => Self::Rotated { state, direction },
            TypedProvider::RuleBased { fallback, rules } => Self::RuleBased { fallback, rules },
            TypedProvider::Simple { state } => Self::Simple { state },
            TypedProvider::Weighted { entries } => Self::Weighted { entries },
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct WeightedState {
    #[serde(rename = "data")]
    pub state: BlockState,
    pub weight: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StateRule {
    pub if_true: BlockPredicate,
    pub then: ProviderHolder,
}

#[derive(Debug, Clone)]
pub struct NoiseProvider {
    pub noise: Arc<Noise>,
    pub scale: f32,
    pub states: Vec<BlockState>,
}

#[derive(Debug, Clone)]
pub struct NoiseThresholdProvider {
    pub noise: Arc<Noise>,
    pub scale: f32,
    pub threshold: f32,
    pub high_chance: f32,
    pub default_state: BlockState,
    pub low_states: Vec<BlockState>,
    pub high_states: Vec<BlockState>,
}

#[derive(Debug, Clone)]
pub struct DualNoiseProvider {
    pub noise: Arc<Noise>,
    pub scale: f32,
    pub slow_noise: Arc<Noise>,
    pub slow_scale: f32,
    pub variety: InclusiveRange,
    pub states: Vec<BlockState>,
}

fn create_noise(parameters: Parameters, seed: i64) -> Arc<Noise> {
    let mut random = RandomSource::Legacy(LegacyRandomSource::from_seed(seed as u64));
    Arc::new(NormalNoise::new(parameters).create(&mut random))
}

#[derive(Debug, Clone, Deserialize)]
struct RawNoiseProvider {
    seed: i64,
    noise: Parameters,
    scale: f32,
    states: Vec<BlockState>,
}

impl From<RawNoiseProvider> for NoiseProvider {
    fn from(raw: RawNoiseProvider) -> Self {
        Self {
            noise: create_noise(raw.noise, raw.seed),
            scale: raw.scale,
            states: raw.states,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct RawNoiseThresholdProvider {
    seed: i64,
    noise: Parameters,
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
            noise: create_noise(raw.noise, raw.seed),
            scale: raw.scale,
            threshold: raw.threshold,
            high_chance: raw.high_chance,
            default_state: raw.default_state,
            low_states: raw.low_states,
            high_states: raw.high_states,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct RawDualNoiseProvider {
    seed: i64,
    noise: Parameters,
    scale: f32,
    slow_noise: Parameters,
    slow_scale: f32,
    variety: InclusiveRange,
    states: Vec<BlockState>,
}

impl From<RawDualNoiseProvider> for DualNoiseProvider {
    fn from(raw: RawDualNoiseProvider) -> Self {
        Self {
            noise: create_noise(raw.noise, raw.seed),
            scale: raw.scale,
            slow_noise: create_noise(raw.slow_noise, raw.seed),
            slow_scale: raw.slow_scale,
            variety: raw.variety,
            states: raw.states,
        }
    }
}

const DIRECTION_VALUES: [Direction; 6] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];

impl BlockStateProvider {
    pub fn get_state(
        &self,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        pos: BlockPosition,
    ) -> BlockState {
        match self {
            Self::CopyProperties { source } => {
                with_properties_of(source.get_state(region, random, pos), region.get_block(pos))
            }
            Self::DualNoise(provider) => {
                let variety_noise = f64::from(slow_noise_value(provider, pos));
                let local_variety = clamped_lerp(
                    f64::from(provider.variety.min_inclusive),
                    f64::from(provider.variety.max_inclusive + 1),
                    (variety_noise - -1.0) / (1.0 - -1.0),
                ) as i32;
                let possible_states: Vec<BlockState> = (0..local_variety.max(0))
                    .map(|i| {
                        let offset =
                            BlockPosition::new(pos.x() + i * 54_545, pos.y(), pos.z() + i * 34_234);
                        random_state_from_noise(
                            &provider.states,
                            slow_noise_value(provider, offset),
                        )
                    })
                    .collect();
                random_state_from_noise(
                    &possible_states,
                    noise_value(&provider.noise, pos, f64::from(provider.scale)),
                )
            }
            Self::Noise(provider) => random_state_from_noise(
                &provider.states,
                noise_value(&provider.noise, pos, f64::from(provider.scale)),
            ),
            Self::NoiseThreshold(provider) => {
                let local_value =
                    f64::from(noise_value(&provider.noise, pos, f64::from(provider.scale)));
                if local_value < f64::from(provider.threshold) {
                    random_element(&provider.low_states, random)
                } else if random.next_f32() < provider.high_chance {
                    random_element(&provider.high_states, random)
                } else {
                    provider.default_state
                }
            }
            Self::RandomBlock { blocks } => {
                random_block(blocks, random).unwrap_or_else(|| region.get_block(pos))
            }
            Self::RandomizedInt {
                source,
                property,
                values,
            } => {
                let unmodified_state = source.get_state(region, random, pos);
                if unmodified_state.get_by_name(property).is_none() {
                    return unmodified_state;
                }
                let value = values.sample(random);
                unmodified_state
                    .with_by_name(property, &value.to_string())
                    .unwrap_or_else(|| panic!("value {value} out of range for {property}"))
            }
            Self::Rotated { state, direction } => {
                let direction = direction
                    .unwrap_or_else(|| DIRECTION_VALUES[random.next_i32_bounded(6) as usize]);
                let mut new_state = state.get_state(region, random, pos);
                if new_state.has_property(Properties::AXIS) {
                    new_state = new_state.with(Properties::AXIS, direction.axis());
                }
                if new_state.has_property(Properties::FACING) {
                    new_state = new_state.with(Properties::FACING, direction);
                }
                if direction.axis() != Axis::Y
                    && new_state.has_property(Properties::HORIZONTAL_FACING)
                {
                    new_state = new_state.with(Properties::HORIZONTAL_FACING, direction);
                }
                new_state
            }
            Self::RuleBased { .. } => self
                .get_optional_state(region, random, pos)
                .unwrap_or_else(|| region.get_block(pos)),
            Self::Simple { state } => *state,
            Self::Weighted { entries } => random_weighted_state(entries, random),
        }
    }

    pub fn get_optional_state(
        &self,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        pos: BlockPosition,
    ) -> Option<BlockState> {
        match self {
            Self::RandomBlock { blocks } => random_block(blocks, random),
            Self::RuleBased { fallback, rules } => {
                for rule in rules {
                    if rule.if_true.test(region, pos)
                        && let Some(state) = rule.then.get_optional_state(region, random, pos)
                    {
                        return Some(state);
                    }
                }
                fallback
                    .as_ref()
                    .and_then(|fallback| fallback.get_optional_state(region, random, pos))
            }
            _ => Some(self.get_state(region, random, pos)),
        }
    }
}

fn noise_value(noise: &Noise, pos: BlockPosition, scale: f64) -> f32 {
    noise.get(
        f64::from(pos.x()) * scale,
        f64::from(pos.y()) * scale,
        f64::from(pos.z()) * scale,
    )
}

fn slow_noise_value(provider: &DualNoiseProvider, pos: BlockPosition) -> f32 {
    provider.slow_noise.get(
        f64::from(pos.x() as f32 * provider.slow_scale),
        f64::from(pos.y() as f32 * provider.slow_scale),
        f64::from(pos.z() as f32 * provider.slow_scale),
    )
}

fn random_state_from_noise(states: &[BlockState], noise_value: f32) -> BlockState {
    let placement_value = ((1.0 + noise_value) / 2.0).clamp(0.0, 0.9999);
    states[(placement_value * states.len() as f32) as usize]
}

fn random_element(states: &[BlockState], random: &mut RandomSource) -> BlockState {
    states[random.next_i32_bounded(states.len() as i32) as usize]
}

fn random_block(blocks: &HolderSet, random: &mut RandomSource) -> Option<BlockState> {
    let entries = blocks.blocks();
    if entries.is_empty() {
        return None;
    }
    Some(entries[random.next_i32_bounded(entries.len() as i32) as usize].default_state())
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

fn with_properties_of(result: BlockState, source: BlockState) -> BlockState {
    source
        .properties()
        .into_iter()
        .fold(result, |state, (name, value)| try_set(state, name, value))
}

fn try_set(state: BlockState, name: &str, value: &str) -> BlockState {
    if state.get_by_name(name).is_none() {
        return state;
    }
    state.with_by_name(name, value).unwrap_or(state)
}
