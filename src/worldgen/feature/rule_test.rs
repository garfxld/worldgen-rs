use serde_json::Value;

use crate::registry::REGISTRY;
use crate::util::ident::{BlockState, Identifier};
use crate::util::rng::RandomSource;
use crate::worldgen::json::{parse_block_state, parse_id, type_of};

#[derive(Debug, Clone)]
pub enum RuleTest {
    AlwaysTrue,
    BlockMatch(Identifier),
    BlockStateMatch(BlockState),
    TagMatch(Identifier),
    RandomBlockMatch { block: Identifier, probability: f32 },
    RandomBlockStateMatch { state: BlockState, probability: f32 },
}

impl RuleTest {
    pub fn parse(value: &Value) -> Self {
        let predicate_type = value["predicate_type"]
            .as_str()
            .map_or_else(|| type_of(value).to_owned(), str::to_owned);
        match predicate_type.as_str() {
            "minecraft:always_true" => Self::AlwaysTrue,
            "minecraft:block_match" => Self::BlockMatch(parse_id(block_field(value))),
            "minecraft:blockstate_match" => {
                Self::BlockStateMatch(parse_block_state(&value["block_state"]))
            }
            "minecraft:tag_match" => Self::TagMatch(parse_id(
                value["tag"]
                    .as_str()
                    .unwrap_or_else(|| panic!("tag_match without tag: {value}")),
            )),
            "minecraft:random_block_match" => Self::RandomBlockMatch {
                block: parse_id(block_field(value)),
                probability: probability(value),
            },
            "minecraft:random_blockstate_match" => Self::RandomBlockStateMatch {
                state: parse_block_state(&value["block_state"]),
                probability: probability(value),
            },
            other => panic!("unknown rule test type {other}"),
        }
    }

    pub fn test(&self, state: BlockState, random: &mut RandomSource) -> bool {
        match self {
            Self::AlwaysTrue => true,
            Self::BlockMatch(block) => is_block(state, block),
            Self::BlockStateMatch(expected) => state == *expected,
            Self::TagMatch(tag) => state.get_block().has_tag(tag),
            Self::RandomBlockMatch { block, probability } => {
                is_block(state, block) && random.next_f32() < *probability
            }
            Self::RandomBlockStateMatch {
                state: expected,
                probability,
            } => state == *expected && random.next_f32() < *probability,
        }
    }
}

fn is_block(state: BlockState, block: &Identifier) -> bool {
    REGISTRY
        .blocks
        .by_state_id(state)
        .is_some_and(|actual| actual.key == *block)
}

fn block_field(value: &Value) -> &str {
    value["block"]
        .as_str()
        .unwrap_or_else(|| panic!("rule test without block: {value}"))
}

fn probability(value: &Value) -> f32 {
    value["probability"].as_f64().unwrap_or(1.0) as f32
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
    fn tag_and_block_matches_use_vanilla_tag_data() {
        init_global_registry();
        let mut random = RandomSource::Xoroshiro(XoroshiroRandomSource::from_seed(1));

        let stone = REGISTRY.blocks.get_default_state_id(&vanilla_blocks::STONE);
        let dirt = REGISTRY.blocks.get_default_state_id(&vanilla_blocks::DIRT);

        let tag_match = RuleTest::parse(&json!({
            "predicate_type": "minecraft:tag_match",
            "tag": "minecraft:base_stone_overworld"
        }));
        assert!(tag_match.test(stone, &mut random));
        assert!(!tag_match.test(dirt, &mut random));

        let block_match = RuleTest::parse(&json!({
            "predicate_type": "minecraft:block_match",
            "block": "minecraft:stone"
        }));
        assert!(block_match.test(stone, &mut random));
        assert!(!block_match.test(dirt, &mut random));
    }
}
