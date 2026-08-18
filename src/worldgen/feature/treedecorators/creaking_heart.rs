use serde::Deserialize;

use crate::registry::REGISTRY;
use crate::registry::blocks::properties::CreakingHeartState;
use crate::registry::blocks::properties::Properties;
use crate::registry::blocks::vanilla_blocks;
use crate::util::ident::Identifier;
use crate::util::shuffle;
use crate::worldgen::feature::features::DIRECTIONS;
use crate::worldgen::feature::treedecorators::TreeDecoratorContext;

const LOGS: Identifier = Identifier::const_vanilla("logs");

#[derive(Debug, Deserialize)]
pub struct CreakingHeartDecorator {
    pub probability: f32,
}

impl CreakingHeartDecorator {
    pub fn place(&self, context: &mut TreeDecoratorContext<'_, '_>) {
        if context.logs.is_empty() || context.random.next_f32() >= self.probability {
            return;
        }

        let mut heart_placements = context.logs.clone();
        shuffle(&mut heart_placements, context.random);
        let target_pos = heart_placements.into_iter().find(|pos| {
            DIRECTIONS.iter().all(|dir| {
                let neighbour = dir.relative(*pos);
                REGISTRY
                    .blocks
                    .is_in_tag(&context.region.get_block(neighbour).get_block().key, &LOGS)
            })
        });

        if let Some(pos) = target_pos {
            let state = vanilla_blocks::CREAKING_HEART
                .default_state()
                .set_value(
                    &Properties::CREAKING_HEART_STATE,
                    CreakingHeartState::Dormant,
                )
                .set_value(&Properties::NATURAL, true);
            context.set_block(pos, state);
        }
    }
}
