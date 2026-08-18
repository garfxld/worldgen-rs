use serde::Deserialize;

use crate::registry::REGISTRY;
use crate::util::ident::{BlockState, Identifier};
use crate::util::rng::RandomSource;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "predicate_type")]
pub enum RuleTest {
    #[serde(rename = "minecraft:always_true")]
    AlwaysTrue,
    #[serde(rename = "minecraft:block_match")]
    BlockMatch { block: Identifier },
    #[serde(rename = "minecraft:blockstate_match")]
    BlockStateMatch { block_state: BlockState },
    #[serde(rename = "minecraft:tag_match")]
    TagMatch { tag: Identifier },
    #[serde(rename = "minecraft:random_block_match")]
    RandomBlockMatch { block: Identifier, probability: f32 },
    #[serde(rename = "minecraft:random_blockstate_match")]
    RandomBlockStateMatch {
        block_state: BlockState,
        probability: f32,
    },
}

impl RuleTest {
    pub fn test(&self, state: BlockState, random: &mut RandomSource) -> bool {
        match self {
            Self::AlwaysTrue => true,
            Self::BlockMatch { block } => is_block(state, block),
            Self::BlockStateMatch { block_state } => state == *block_state,
            Self::TagMatch { tag } => state.get_block().has_tag(tag),
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

fn is_block(state: BlockState, block: &Identifier) -> bool {
    REGISTRY
        .blocks
        .by_state_id(state)
        .is_some_and(|actual| actual.key == *block)
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
    fn tag_and_block_matches_use_vanilla_tag_data() {
        init_global_registry();
        let mut random = RandomSource::Xoroshiro(XoroshiroRandomSource::from_seed(1));

        let stone = REGISTRY.blocks.get_default_state_id(&vanilla_blocks::STONE);
        let dirt = REGISTRY.blocks.get_default_state_id(&vanilla_blocks::DIRT);

        let tag_match = from_value::<RuleTest>(json!({
            "predicate_type": "minecraft:tag_match",
            "tag": "minecraft:base_stone_overworld"
        }))
        .expect("test value");
        assert!(tag_match.test(stone, &mut random));
        assert!(!tag_match.test(dirt, &mut random));

        let block_match = from_value::<RuleTest>(json!({
            "predicate_type": "minecraft:block_match",
            "block": "minecraft:stone"
        }))
        .expect("test value");
        assert!(block_match.test(stone, &mut random));
        assert!(!block_match.test(dirt, &mut random));
    }
}
