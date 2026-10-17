use std::collections::HashSet;

use serde::Deserialize;

use crate::util::shuffle;
use crate::world::worldgen::feature::state_provider::ProviderHolder;
use crate::world::worldgen::feature::treedecorators::TreeDecoratorContext;
use shulkr::util::BlockPosition;
use shulkr::util::Direction;

#[derive(Debug, Deserialize)]
pub struct AttachedToLeavesDecorator {
    pub probability: f32,
    pub exclusion_radius_xz: i32,
    pub exclusion_radius_y: i32,
    pub block_provider: ProviderHolder,
    pub required_empty_blocks: i32,
    pub directions: Vec<Direction>,
}

impl AttachedToLeavesDecorator {
    pub fn place(&self, context: &mut TreeDecoratorContext<'_, '_>) {
        let mut propagule_blacklist: HashSet<BlockPosition> = HashSet::new();
        let mut leaves = context.leaves.clone();
        shuffle(&mut leaves, context.random);

        for leaf_pos in leaves {
            let direction = self.directions[context
                .random
                .next_i32_bounded(self.directions.len() as i32)
                as usize];
            let placement_pos = leaf_pos.relative(direction);
            if !propagule_blacklist.contains(&placement_pos)
                && context.random.next_f32() < self.probability
                && self.has_required_empty_blocks(context, leaf_pos, direction)
            {
                for x in placement_pos.x() - self.exclusion_radius_xz
                    ..=placement_pos.x() + self.exclusion_radius_xz
                {
                    for y in placement_pos.y() - self.exclusion_radius_y
                        ..=placement_pos.y() + self.exclusion_radius_y
                    {
                        for z in placement_pos.z() - self.exclusion_radius_xz
                            ..=placement_pos.z() + self.exclusion_radius_xz
                        {
                            propagule_blacklist.insert(BlockPosition::new(x, y, z));
                        }
                    }
                }

                let state =
                    self.block_provider
                        .get_state(context.region, context.random, placement_pos);
                context.set_block(placement_pos, state);
            }
        }
    }

    fn has_required_empty_blocks(
        &self,
        context: &TreeDecoratorContext<'_, '_>,
        leaf_pos: BlockPosition,
        direction: Direction,
    ) -> bool {
        let (dx, dy, dz) = direction.offset();
        (1..=self.required_empty_blocks).all(|i| {
            context.is_air(BlockPosition::new(
                leaf_pos.x() + dx * i,
                leaf_pos.y() + dy * i,
                leaf_pos.z() + dz * i,
            ))
        })
    }
}
