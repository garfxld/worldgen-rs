use serde::Deserialize;
use shulkr::util::{BlockPosition, Key};
use shulkr::world::block::BlockState;

use crate::util::rng::RandomSource;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "predicate_type")]
pub enum RuleTest {
    #[serde(rename = "minecraft:all_of")]
    AllOf { rules: Vec<RuleTest> },
    #[serde(rename = "minecraft:always_true")]
    AlwaysTrue,
    #[serde(rename = "minecraft:any_of")]
    AnyOf { rules: Vec<RuleTest> },
    #[serde(rename = "minecraft:not")]
    Not { rule: Box<RuleTest> },
    #[serde(rename = "minecraft:height_match")]
    HeightMatch {
        min_inclusive: i32,
        max_inclusive: i32,
    },
    #[serde(rename = "minecraft:block_match")]
    BlockMatch { block: Key },
    #[serde(rename = "minecraft:blockstate_match")]
    BlockStateMatch { block_state: BlockState },
    #[serde(rename = "minecraft:tag_match")]
    TagMatch { tag: Key },
    #[serde(rename = "minecraft:random_block_match")]
    RandomBlockMatch { block: Key, probability: f32 },
    #[serde(rename = "minecraft:random_blockstate_match")]
    RandomBlockStateMatch {
        block_state: BlockState,
        probability: f32,
    },
}

impl RuleTest {
    pub fn test(&self, state: BlockState, pos: BlockPosition, random: &mut RandomSource) -> bool {
        match self {
            Self::AllOf { rules } => rules.iter().all(|rule| rule.test(state, pos, random)),
            Self::AlwaysTrue => true,
            Self::AnyOf { rules } => rules.iter().any(|rule| rule.test(state, pos, random)),
            Self::Not { rule } => !rule.test(state, pos, random),
            Self::HeightMatch {
                min_inclusive,
                max_inclusive,
            } => *min_inclusive <= pos.y() && pos.y() <= *max_inclusive,
            Self::BlockMatch { block } => is_block(state, block),
            Self::BlockStateMatch { block_state } => state == *block_state,
            Self::TagMatch { tag } => state.as_block().has_tag(tag),
            Self::RandomBlockMatch { block, probability } => {
                is_block(state, block) && random.next_f32() < *probability
            }
            Self::RandomBlockStateMatch {
                block_state,
                probability,
            } => state == *block_state && random.next_f32() < *probability,
        }
    }
}

fn is_block(state: BlockState, block: &Key) -> bool {
    Some(state.as_block()).is_some_and(|actual| actual.key() == block)
}

#[cfg(test)]
mod tests {
    use crate::datapack;
    use crate::world::worldgen;
    use std::sync::Once;

    use serde_json::{from_value, json};
    use shulkr::world::block::Block;

    use super::*;
    use crate::util::rng::xoroshiro::XoroshiroRandomSource;

    fn init_global_registry() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            worldgen::ensure_registry_initialized(&datapack::Datapack::builtin())
                .expect("vanilla datapack should load");
        });
    }

    #[test]
    fn tag_and_block_matches_use_vanilla_tag_data() {
        init_global_registry();
        let mut random = RandomSource::Xoroshiro(XoroshiroRandomSource::from_seed(1));

        let stone = Block::STONE.default_state();
        let dirt = Block::DIRT.default_state();

        let tag_match = from_value::<RuleTest>(json!({
            "predicate_type": "minecraft:tag_match",
            "tag": "minecraft:base_stone_overworld"
        }))
        .expect("test value");
        assert!(tag_match.test(stone, BlockPosition::new(0, 0, 0), &mut random));
        assert!(!tag_match.test(dirt, BlockPosition::new(0, 0, 0), &mut random));

        let block_match = from_value::<RuleTest>(json!({
            "predicate_type": "minecraft:block_match",
            "block": "minecraft:stone"
        }))
        .expect("test value");
        assert!(block_match.test(stone, BlockPosition::new(0, 0, 0), &mut random));
        assert!(!block_match.test(dirt, BlockPosition::new(0, 0, 0), &mut random));
    }
}
