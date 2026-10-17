use shulkr::world::block::{Block, BlockState};

use crate::util::rng::RandomSource;
use crate::world::chunk::heightmap::HeightmapType;
use crate::world::worldgen::region::WorldGenRegion;
use crate::world::worldgen::structure::structure_piece::{PieceBase, PieceKind, StructurePiece};
use shulkr::util::BlockPosition;
use shulkr::util::BoundingBox;
use shulkr::util::Direction;

#[derive(Debug)]
pub struct BuriedTreasurePiece;

impl BuriedTreasurePiece {
    #[expect(
        clippy::new_ret_no_self,
        reason = "mirrors the vanilla piece constructor"
    )]
    pub const fn new(offset: BlockPosition) -> StructurePiece {
        StructurePiece::new(
            PieceBase::new(0, BoundingBox::from_point(offset)),
            PieceKind::BuriedTreasure(Self),
        )
    }

    pub fn post_process(
        base: &mut PieceBase,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        chunk_bb: BoundingBox,
    ) {
        let box_ = base.bounding_box;
        let y = region.get_height(HeightmapType::OceanFloorWg, box_.min_x(), box_.min_z());
        let mut pos = BlockPosition::new(box_.min_x(), y, box_.min_z());

        while pos.y() > region.min_y() {
            let current_state = region.get_block(pos);
            let below_state = region.get_block(pos.below());

            if matches!(
                below_state.as_block(),
                Block::SANDSTONE | Block::STONE | Block::ANDESITE | Block::GRANITE | Block::DIORITE
            ) {
                let soft_state = if current_state.is_air() || is_liquid(current_state) {
                    Block::SAND.default_state()
                } else {
                    current_state
                };

                for direction in Direction::ALL {
                    let relative_pos = pos.relative(direction);
                    let relative_state = region.get_block(relative_pos);
                    if relative_state.is_air() || is_liquid(relative_state) {
                        let below_relative_state = region.get_block(relative_pos.below());
                        if (below_relative_state.is_air() || is_liquid(below_relative_state))
                            && direction != Direction::Up
                        {
                            region.set_block(relative_pos, below_state);
                        } else {
                            region.set_block(relative_pos, soft_state);
                        }
                    }
                }

                base.bounding_box = BoundingBox::from_point(pos);
                base.create_chest_at(region, chunk_bb, random, pos, None);
                return;
            }

            pos = pos.below();
        }
    }
}

fn is_liquid(state: BlockState) -> bool {
    state.is(Block::WATER) || state.is(Block::LAVA)
}
