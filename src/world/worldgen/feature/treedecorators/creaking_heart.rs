use serde::Deserialize;
use shulkr::util::Key;
use shulkr::world::block::Block;

use shulkr::world::block::CreakingHeartState;
use shulkr::world::block::Properties;

use crate::util::shuffle;
use crate::world::worldgen::feature::features::DIRECTIONS;
use crate::world::worldgen::feature::treedecorators::TreeDecoratorContext;

const LOGS: Key = Key::const_vanilla("logs");

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
                let neighbour = pos.relative(*dir);
                context
                    .region
                    .get_block(neighbour)
                    .as_block()
                    .has_tag(&LOGS)
            })
        });

        if let Some(pos) = target_pos {
            let state = Block::CREAKING_HEART
                .default_state()
                .with(
                    Properties::CREAKING_HEART_STATE,
                    CreakingHeartState::Dormant,
                )
                .with(Properties::NATURAL, true);
            context.set_block(pos, state);
        }
    }
}
