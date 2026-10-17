use serde::Deserialize;
use shulkr::util::Key;
use shulkr::world::block::Block;

use shulkr::world::block::Properties;

use crate::util::shuffle;
use crate::world::worldgen::feature::features as feature_registry_fn;
use crate::world::worldgen::feature::treedecorators::TreeDecoratorContext;
use shulkr::util::BlockPosition;

#[derive(Debug, Deserialize)]
pub struct PaleMossDecorator {
    pub leaves_probability: f32,
    pub trunk_probability: f32,
    pub ground_probability: f32,
}

impl PaleMossDecorator {
    pub fn place(&self, context: &mut TreeDecoratorContext<'_, '_>) {
        let mut logs = context.logs.clone();
        shuffle(&mut logs, context.random);
        if logs.is_empty() {
            return;
        }

        let origin = *logs
            .iter()
            .min_by_key(|pos| pos.y())
            .expect("logs is non-empty");
        if context.random.next_f32() < self.ground_probability
            && let Some(moss_patch) =
                feature_registry_fn().configured(&Key::const_vanilla("pale_moss_patch"))
        {
            moss_patch.place(context.region, context.random, origin.above());
        }

        let logs = context.logs.clone();
        for pos in logs {
            if context.random.next_f32() < self.trunk_probability {
                let below = pos.below();
                if context.is_air(below) {
                    add_moss_hanger(below, context);
                }
            }
        }

        let leaves = context.leaves.clone();
        for pos in leaves {
            if context.random.next_f32() < self.leaves_probability {
                let below = pos.below();
                if context.is_air(below) {
                    add_moss_hanger(below, context);
                }
            }
        }
    }
}

fn add_moss_hanger(mut pos: BlockPosition, context: &mut TreeDecoratorContext<'_, '_>) {
    while context.is_air(pos.below()) && context.random.next_f32() >= 0.5 {
        context.set_block(
            pos,
            Block::PALE_HANGING_MOSS
                .default_state()
                .with(Properties::TIP, false),
        );
        pos = pos.below();
    }

    context.set_block(
        pos,
        Block::PALE_HANGING_MOSS
            .default_state()
            .with(Properties::TIP, true),
    );
}
