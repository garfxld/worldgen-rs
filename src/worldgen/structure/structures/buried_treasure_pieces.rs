use crate::chunk::heightmap::HeightmapType;
use crate::registry::blocks::vanilla_blocks;
use crate::util::direction::Direction;
use crate::util::geometry::BoundingBox;
use crate::util::ident::BlockState;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::worldgen::region::WorldGenRegion;
use crate::worldgen::structure::structure_piece::{PieceBase, PieceKind, StructurePiece};

#[derive(Debug)]
pub struct BuriedTreasurePiece;

impl BuriedTreasurePiece {
    #[expect(
        clippy::new_ret_no_self,
        reason = "mirrors the vanilla piece constructor"
    )]
    pub const fn new(offset: BlockPos) -> StructurePiece {
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
        let mut pos = BlockPos::new(box_.min_x(), y, box_.min_z());

        while pos.y() > region.min_y() {
            let current_state = region.get_block(pos);
            let below_state = region.get_block(pos.below());
            let below_block = below_state.get_block();
            if below_block == &vanilla_blocks::SANDSTONE
                || below_block == &vanilla_blocks::STONE
                || below_block == &vanilla_blocks::ANDESITE
                || below_block == &vanilla_blocks::GRANITE
                || below_block == &vanilla_blocks::DIORITE
            {
                let soft_state = if current_state.is_air() || is_liquid(current_state) {
                    vanilla_blocks::SAND.default_state()
                } else {
                    current_state
                };

                for direction in Direction::ALL {
                    let relative_pos = direction.relative(pos);
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
    let block = state.get_block();
    block == &vanilla_blocks::WATER || block == &vanilla_blocks::LAVA
}
