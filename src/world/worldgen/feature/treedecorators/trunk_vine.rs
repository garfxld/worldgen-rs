use crate::world::worldgen::feature::treedecorators::TreeDecoratorContext;
use shulkr::util::Direction;
use shulkr::world::block::Properties;

pub struct TrunkVineDecorator;

impl TrunkVineDecorator {
    pub fn place(&self, context: &mut TreeDecoratorContext<'_, '_>) {
        for pos in context.logs.clone() {
            for (direction, attached_face) in [
                (Direction::West, Properties::EAST),
                (Direction::East, Properties::WEST),
                (Direction::North, Properties::SOUTH),
                (Direction::South, Properties::NORTH),
            ] {
                if context.random.next_i32_bounded(3) <= 0 {
                    continue;
                }
                let side = pos.relative(direction);
                if context.is_air(side) {
                    context.place_vine(side, attached_face);
                }
            }
        }
    }
}
