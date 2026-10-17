use serde::Deserialize;
use shulkr::world::block::Block;

use crate::world::worldgen::feature::treedecorators::TreeDecoratorContext;
use shulkr::util::Direction;
use shulkr::world::block::Properties;

#[derive(Debug, Deserialize)]
pub struct CocoaDecorator {
    pub probability: f32,
}

impl CocoaDecorator {
    pub fn place(&self, context: &mut TreeDecoratorContext<'_, '_>) {
        let logs = &context.logs;
        if logs.is_empty() {
            return;
        }

        if context.random.next_f32() >= self.probability {
            return;
        }

        let base_y = logs[0].y();
        let eligible: Vec<_> = logs
            .iter()
            .filter(|pos| pos.y() - base_y <= 2)
            .copied()
            .collect();

        for log_pos in eligible {
            for direction in Direction::HORIZONTAL {
                if context.random.next_f32() > 0.25 {
                    continue;
                }
                let pos = log_pos.relative(direction.opposite());
                if context.is_air(pos) {
                    let age = context.random.next_i32_bounded(3) as u8;
                    let state = Block::COCOA
                        .default_state()
                        .with(Properties::AGE_2, age)
                        .with(Properties::HORIZONTAL_FACING, direction);
                    context.set_block(pos, state);
                }
            }
        }
    }
}
