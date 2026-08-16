use serde::Deserialize;

use crate::registry::blocks::properties::Properties;
use crate::registry::blocks::vanilla_blocks;
use crate::util::direction::Direction;
use crate::util::shuffle;
use crate::worldgen::feature::treedecorators::TreeDecoratorContext;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BeehiveDecorator {
    pub probability: f32,
}

const WORLDGEN_FACING: Direction = Direction::South;
const SPAWN_DIRECTIONS: [Direction; 3] = [Direction::East, Direction::South, Direction::West];

impl BeehiveDecorator {
    pub fn place(&self, context: &mut TreeDecoratorContext<'_, '_>) {
        let logs = &context.logs;
        if logs.is_empty() {
            return;
        }

        if context.random.next_f32() >= self.probability {
            return;
        }

        let leaves = &context.leaves;
        let hive_y = if !leaves.is_empty() {
            (leaves[0].y() - 1).max(logs[0].y() + 1)
        } else {
            (logs[0].y() + 1 + context.random.next_i32_bounded(3)).min(logs[logs.len() - 1].y())
        };

        let hive_placements: Vec<_> = logs
            .iter()
            .filter(|pos| pos.y() == hive_y)
            .flat_map(|pos| SPAWN_DIRECTIONS.iter().map(move |dir| dir.relative(*pos)))
            .collect();

        if hive_placements.is_empty() {
            return;
        }

        let mut placements = hive_placements;
        shuffle(&mut placements, context.random);

        let facing_step = WORLDGEN_FACING.offset();

        let hive_pos = placements.iter().find(|pos| {
            context.is_air(**pos)
                && context.is_air(pos.offset(facing_step.0, facing_step.1, facing_step.2))
        });

        if let Some(&pos) = hive_pos {
            let state = vanilla_blocks::BEE_NEST
                .default_state()
                .set_value(&Properties::HORIZONTAL_FACING, WORLDGEN_FACING);
            context.set_block(pos, state);

            let bees = 2 + context.random.next_i32_bounded(2);
            for _ in 0..bees {
                context.random.next_i32_bounded(599);
            }
        }
    }
}
