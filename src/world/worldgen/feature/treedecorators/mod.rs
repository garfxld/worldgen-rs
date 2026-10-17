pub mod alter_ground;
pub mod attached_to_leaves;
pub mod attached_to_logs;
pub mod beehive;
pub mod cocoa;
pub mod creaking_heart;
pub mod leave_vine;
pub mod pale_moss;
pub mod place_on_ground;
pub mod shelf_mushroom;
pub mod trunk_vine;

use serde::Deserialize;
use shulkr::world::block::Block;

use crate::util::hash_set::JavaHashSet;
use crate::util::rng::RandomSource;
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util::BlockPosition;
use shulkr::world::block::BlockState;
use shulkr::world::block::BoolProperty;

use alter_ground::AlterGroundDecorator;
use attached_to_leaves::AttachedToLeavesDecorator;
use attached_to_logs::AttachedToLogsDecorator;
use beehive::BeehiveDecorator;
use cocoa::CocoaDecorator;
use creaking_heart::CreakingHeartDecorator;
use leave_vine::LeaveVineDecorator;
use pale_moss::PaleMossDecorator;
use place_on_ground::PlaceOnGroundDecorator;
use shelf_mushroom::ShelfMushroomDecorator;
use trunk_vine::TrunkVineDecorator;

pub struct TreeDecoratorContext<'a, 'b> {
    pub region: &'a WorldGenRegion<'a>,
    pub random: &'a mut RandomSource,
    pub logs: Vec<BlockPosition>,
    pub leaves: Vec<BlockPosition>,
    pub roots: Vec<BlockPosition>,
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
        let mut logs: Vec<BlockPosition> = trunks.iter().collect();
        let mut leaves: Vec<BlockPosition> = foliage.iter().collect();
        let mut roots: Vec<BlockPosition> = roots.iter().collect();
        logs.sort_by_key(BlockPosition::y);
        leaves.sort_by_key(BlockPosition::y);
        roots.sort_by_key(BlockPosition::y);
        TreeDecoratorContext {
            region,
            random,
            logs,
            leaves,
            roots,
            decorations,
        }
    }

    pub fn set_block(&mut self, pos: BlockPosition, state: BlockState) {
        self.decorations.insert(pos);
        self.region.set_block(pos, state);
    }

    pub fn place_vine(&mut self, pos: BlockPosition, direction: BoolProperty) {
        let state = Block::VINE.default_state().with(direction, true);
        self.set_block(pos, state);
    }

    pub fn is_air(&self, pos: BlockPosition) -> bool {
        self.region.get_block(pos).is_air()
    }

    pub fn is_replaceable(&self, pos: BlockPosition) -> bool {
        self.region.get_block(pos).is_replaceable()
    }

    pub fn is_water_or_water_nearby(&self, pos: BlockPosition) -> bool {
        [pos, pos.east(), pos.west(), pos.north(), pos.south()]
            .into_iter()
            .any(|pos| self.region.get_block(pos).is(Block::WATER))
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
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
    #[serde(rename = "minecraft:shelf_mushroom")]
    ShelfMushroom(ShelfMushroomDecorator),
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
            Self::ShelfMushroom(p) => p.place(context),
        }
    }
}
