use serde::Deserialize;

use crate::world::worldgen::feature::features::tree::lowest_trunk_or_root_of_tree;
use crate::world::worldgen::feature::state_provider::ProviderHolder;
use crate::world::worldgen::feature::treedecorators::TreeDecoratorContext;
use shulkr::util::BlockPosition;

#[derive(Debug, Deserialize)]
pub struct AlterGroundDecorator {
    pub provider: ProviderHolder,
}

impl AlterGroundDecorator {
    pub fn place(&self, context: &mut TreeDecoratorContext<'_, '_>) {
        let positions = lowest_trunk_or_root_of_tree(context);
        let Some(first) = positions.first().copied() else {
            return;
        };
        let min_y = first.y();

        for pos in positions.iter().copied().filter(|pos| pos.y() == min_y) {
            self.place_circle(context, pos.offset(-1, 0, -1));
            self.place_circle(context, pos.offset(2, 0, -1));
            self.place_circle(context, pos.offset(-1, 0, 2));
            self.place_circle(context, pos.offset(2, 0, 2));

            for _ in 0..5 {
                let placement = context.random.next_i32_bounded(64);
                let xx = placement % 8;
                let zz = placement / 8;
                if xx == 0 || xx == 7 || zz == 0 || zz == 7 {
                    self.place_circle(context, pos.offset(-3 + xx, 0, -3 + zz));
                }
            }
        }
    }

    fn place_circle(&self, context: &mut TreeDecoratorContext<'_, '_>, pos: BlockPosition) {
        for dx in -2i32..=2 {
            for dz in -2i32..=2 {
                if dx.abs() == 2 && dz.abs() == 2 {
                    continue;
                }
                self.place_block_at(context, pos.offset(dx, 0, dz));
            }
        }
    }

    fn place_block_at(&self, context: &mut TreeDecoratorContext<'_, '_>, pos: BlockPosition) {
        for dy in (-3..=2).rev() {
            let cursor = pos.offset(0, dy, 0);
            if let Some(state) =
                self.provider
                    .get_optional_state(context.region, context.random, cursor)
            {
                context.set_block(cursor, state);
                break;
            }
            if !context.is_air(cursor) && dy < 0 {
                break;
            }
        }
    }
}
