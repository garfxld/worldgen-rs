pub mod alter_ground;
pub mod attached_to_leaves;
pub mod attached_to_logs;
pub mod beehive;
pub mod cocoa;
pub mod creaking_heart;
pub mod leave_vine;
pub mod pale_moss;
pub mod place_on_ground;
pub mod trunk_vine;

use serde::Deserialize;

use crate::registry::blocks::properties::BoolProperty;
use crate::registry::blocks::vanilla_blocks;
use crate::util::hash_set::JavaHashSet;
use crate::util::ident::BlockState;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::worldgen::region::WorldGenRegion;

use alter_ground::AlterGroundDecorator;
use attached_to_leaves::AttachedToLeavesDecorator;
use attached_to_logs::AttachedToLogsDecorator;
use beehive::BeehiveDecorator;
use cocoa::CocoaDecorator;
use creaking_heart::CreakingHeartDecorator;
use leave_vine::LeaveVineDecorator;
use pale_moss::PaleMossDecorator;
use place_on_ground::PlaceOnGroundDecorator;
use trunk_vine::TrunkVineDecorator;

pub struct TreeDecoratorContext<'a, 'b> {
    pub region: &'a WorldGenRegion<'a>,
    pub random: &'a mut RandomSource,
    pub logs: Vec<BlockPos>,
    pub leaves: Vec<BlockPos>,
    pub roots: Vec<BlockPos>,
    decorations: &'b mut JavaHashSet,
}

impl<'a, 'b> TreeDecoratorContext<'a, 'b> {
    pub fn new(
        region: &'a WorldGenRegion<'a>,
        random: &'a mut RandomSource,
        trunks: &JavaHashSet,
        foliage: &JavaHashSet,
        roots: &JavaHashSet,
        decorations: &'b mut JavaHashSet,
    ) -> TreeDecoratorContext<'a, 'b> {
        let mut logs: Vec<BlockPos> = trunks.iter().collect();
        let mut leaves: Vec<BlockPos> = foliage.iter().collect();
        let mut roots: Vec<BlockPos> = roots.iter().collect();
        logs.sort_by_key(BlockPos::y);
        leaves.sort_by_key(BlockPos::y);
        roots.sort_by_key(BlockPos::y);
        TreeDecoratorContext {
            region,
            random,
            logs,
            leaves,
            roots,
            decorations,
        }
    }

    pub fn set_block(&mut self, pos: BlockPos, state: BlockState) {
        self.decorations.insert(pos);
        self.region.set_block(pos, state);
    }

    pub fn place_vine(&mut self, pos: BlockPos, direction: &BoolProperty) {
        let state = vanilla_blocks::VINE.default_state().set_value(direction, true);
        self.set_block(pos, state);
    }

    pub fn is_air(&self, pos: BlockPos) -> bool {
        self.region.get_block(pos).is_air()
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum TreeDecorator {
    #[serde(rename = "minecraft:place_on_ground")]
    PlaceOnGround(PlaceOnGroundDecorator),
    #[serde(rename = "minecraft:attached_to_logs")]
    AttachedToLogs(AttachedToLogsDecorator),
    #[serde(rename = "minecraft:trunk_vine")]
    TrunkVine,
    #[serde(rename = "minecraft:beehive")]
    Beehive(BeehiveDecorator),
    #[serde(rename = "minecraft:leave_vine")]
    LeaveVine(LeaveVineDecorator),
    #[serde(rename = "minecraft:cocoa")]
    Cocoa(CocoaDecorator),
    #[serde(rename = "minecraft:alter_ground")]
    AlterGround(AlterGroundDecorator),
    #[serde(rename = "minecraft:attached_to_leaves")]
    AttachedToLeaves(AttachedToLeavesDecorator),
    #[serde(rename = "minecraft:creaking_heart")]
    CreakingHeart(CreakingHeartDecorator),
    #[serde(rename = "minecraft:pale_moss")]
    PaleMoss(PaleMossDecorator),
}

impl TreeDecorator {
    pub fn place(&self, context: &mut TreeDecoratorContext<'_, '_>) {
        match self {
            Self::PlaceOnGround(p) => p.place(context),
            Self::AttachedToLogs(p) => p.place(context),
            Self::TrunkVine => TrunkVineDecorator.place(context),
            Self::Beehive(p) => p.place(context),
            Self::LeaveVine(p) => p.place(context),
            Self::Cocoa(p) => p.place(context),
            Self::AlterGround(p) => p.place(context),
            Self::AttachedToLeaves(p) => p.place(context),
            Self::CreakingHeart(p) => p.place(context),
            Self::PaleMoss(p) => p.place(context),
        }
    }
}
