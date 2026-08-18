use serde::Deserialize;

use crate::registry::blocks::properties::{BoolProperty, Properties};
use crate::util::pos::BlockPos;
use crate::worldgen::feature::treedecorators::TreeDecoratorContext;

#[derive(Debug, Deserialize)]
pub struct LeaveVineDecorator {
    pub probability: f32,
}

impl LeaveVineDecorator {
    pub fn place(&self, context: &mut TreeDecoratorContext<'_, '_>) {
        let leaves = context.leaves.clone();
        for pos in leaves {
            if context.random.next_f32() < self.probability {
                let west = pos.west();
                if context.is_air(west) {
                    add_hanging_vine(west, &Properties::EAST, context);
                }
            }

            if context.random.next_f32() < self.probability {
                let east = pos.east();
                if context.is_air(east) {
                    add_hanging_vine(east, &Properties::WEST, context);
                }
            }

            if context.random.next_f32() < self.probability {
                let north = pos.north();
                if context.is_air(north) {
                    add_hanging_vine(north, &Properties::SOUTH, context);
                }
            }

            if context.random.next_f32() < self.probability {
                let south = pos.south();
                if context.is_air(south) {
                    add_hanging_vine(south, &Properties::NORTH, context);
                }
            }
        }
    }
}

fn add_hanging_vine(
    pos: BlockPos,
    direction: &BoolProperty,
    context: &mut TreeDecoratorContext<'_, '_>,
) {
    context.place_vine(pos, direction);
    let mut remaining = 4;
    let mut below = pos.below();
    while context.is_air(below) && remaining > 0 {
        context.place_vine(below, direction);
        remaining -= 1;
        below = below.below();
    }
}
