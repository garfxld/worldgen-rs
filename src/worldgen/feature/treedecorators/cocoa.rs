use serde::Deserialize;

use crate::registry::blocks::properties::Properties;
use crate::registry::blocks::vanilla_blocks;
use crate::util::direction::Direction;
use crate::worldgen::feature::treedecorators::TreeDecoratorContext;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
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
        let eligible: Vec<_> = logs.iter().filter(|pos| pos.y() - base_y <= 2).copied().collect();

        for log_pos in eligible {
            for direction in Direction::HORIZONTAL {
                if context.random.next_f32() > 0.25 {
                    continue;
                }
                let pos = direction.opposite().relative(log_pos);
                if context.is_air(pos) {
                    let age = context.random.next_i32_bounded(3) as u8;
                    let state = vanilla_blocks::COCOA
                        .default_state()
                        .set_value(&Properties::AGE_2, age)
                        .set_value(&Properties::HORIZONTAL_FACING, direction);
                    context.set_block(pos, state);
                }
            }
        }
    }
}
