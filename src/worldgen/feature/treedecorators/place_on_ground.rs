use serde::Deserialize;

use crate::chunk::heightmap::HeightmapType;
use crate::registry::blocks::shapes::is_shape_full_block;
use crate::registry::blocks::vanilla_blocks;
use crate::util::geometry::BoundingBox;
use crate::util::pos::BlockPos;
use crate::worldgen::feature::features::tree::lowest_trunk_or_root_of_tree;
use crate::worldgen::feature::state_provider::BlockStateProvider;
use crate::worldgen::feature::treedecorators::TreeDecoratorContext;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlaceOnGroundDecorator {
    #[serde(default = "default_tries")]
    pub tries: i32,
    #[serde(default = "default_radius")]
    pub radius: i32,
    #[serde(default = "default_height")]
    pub height: i32,
    pub block_state_provider: BlockStateProvider,
}

const fn default_tries() -> i32 {
    128
}

const fn default_radius() -> i32 {
    2
}

const fn default_height() -> i32 {
    1
}

impl PlaceOnGroundDecorator {
    pub fn place(&self, context: &mut TreeDecoratorContext<'_, '_>) {
        let positions = lowest_trunk_or_root_of_tree(context);
        let Some(&origin) = positions.first() else {
            return;
        };
        let min_y = origin.y();
        let (mut min_x, mut max_x) = (origin.x(), origin.x());
        let (mut min_z, mut max_z) = (origin.z(), origin.z());
        for position in &positions {
            if position.y() == min_y {
                min_x = min_x.min(position.x());
                max_x = max_x.max(position.x());
                min_z = min_z.min(position.z());
                max_z = max_z.max(position.z());
            }
        }

        let bb = BoundingBox::new(
            BlockPos::new(min_x, min_y, min_z).0,
            BlockPos::new(max_x, min_y, max_z).0,
        )
        .inflated_by_xyz(self.radius, self.height, self.radius);

        for _ in 0..self.tries {
            let pos = BlockPos::new(
                context.random.next_i32_between(bb.min_x(), bb.max_x()),
                context.random.next_i32_between(bb.min_y(), bb.max_y()),
                context.random.next_i32_between(bb.min_z(), bb.max_z()),
            );
            attempt_to_place_block_above(context, pos, &self.block_state_provider);
        }
    }
}

fn attempt_to_place_block_above(
    context: &mut TreeDecoratorContext<'_, '_>,
    pos: BlockPos,
    provider: &BlockStateProvider,
) {
    let above = pos.above();
    let above_state = context.region.get_block(above);
    let height = context
        .region
        .get_height(HeightmapType::MotionBlockingNoLeaves, pos.x(), pos.z());
    if (above_state.is_air() || above_state.get_block() == &vanilla_blocks::VINE)
        && is_shape_full_block(context.region.get_block(pos).get_occlusion_shape())
        && height <= above.y()
    {
        let state = provider.get_state(context.region, context.random, above);
        context.set_block(above, state);
    }
}

