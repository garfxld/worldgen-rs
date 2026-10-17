use serde::Deserialize;

use crate::util::shuffle;
use crate::world::worldgen::feature::state_provider::ProviderHolder;
use crate::world::worldgen::feature::treedecorators::TreeDecoratorContext;
use shulkr::util::Direction;

#[derive(Debug, Deserialize)]
pub struct AttachedToLogsDecorator {
    pub probability: f32,
    pub block_provider: ProviderHolder,
    pub directions: Vec<Direction>,
}

impl AttachedToLogsDecorator {
    pub fn place(&self, context: &mut TreeDecoratorContext<'_, '_>) {
        let mut logs = context.logs.clone();
        shuffle(&mut logs, context.random);
        for log_pos in logs {
            let direction = self.directions[context
                .random
                .next_i32_bounded(self.directions.len() as i32)
                as usize];
            let placement_pos = log_pos.relative(direction);
            if context.random.next_f32() <= self.probability && context.is_air(placement_pos) {
                let state =
                    self.block_provider
                        .get_state(context.region, context.random, placement_pos);
                context.set_block(placement_pos, state);
            }
        }
    }
}
