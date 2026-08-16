use crate::registry::blocks::properties::Properties;
use crate::util::direction::Direction;
use crate::worldgen::feature::treedecorators::TreeDecoratorContext;

pub struct TrunkVineDecorator;

impl TrunkVineDecorator {
    pub fn place(&self, context: &mut TreeDecoratorContext<'_, '_>) {
        for pos in context.logs.clone() {
            for (direction, attached_face) in [
                (Direction::West, &Properties::EAST),
                (Direction::East, &Properties::WEST),
                (Direction::North, &Properties::SOUTH),
                (Direction::South, &Properties::NORTH),
            ] {
                if context.random.next_i32_bounded(3) <= 0 {
                    continue;
                }
                let side = direction.relative(pos);
                if context.is_air(side) {
                    context.place_vine(side, attached_face);
                }
            }
        }
    }
}
