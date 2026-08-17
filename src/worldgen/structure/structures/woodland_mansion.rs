use crate::registry::blocks::vanilla_blocks;
use crate::util::geometry::BoundingBox;
use crate::util::pos::BlockPos;
use crate::util::rotation::Rotation;
use crate::worldgen::region::WorldGenRegion;
use crate::worldgen::structure::pieces::pieces_container::PiecesContainer;
use crate::worldgen::structure::structure::{
    GenerationContext, GenerationStub, get_lowest_y_in_5by5_box_offset_7_blocks,
};
use crate::worldgen::structure::structures::woodland_mansion_pieces;

pub fn find_generation_point<'a>(
    context: &'a GenerationContext<'a>,
) -> Option<GenerationStub<'a>> {
    let rotation = context.with_random(Rotation::get_random);
    let start_pos = get_lowest_y_in_5by5_box_offset_7_blocks(context, rotation);
    if start_pos.y() < 60 {
        return None;
    }
    Some(GenerationStub {
        position: start_pos,
        build: Box::new(move |builder| {
            let mut pieces = Vec::new();
            context.with_random(|random| {
                woodland_mansion_pieces::generate_mansion(
                    start_pos,
                    rotation,
                    &mut pieces,
                    random,
                );
            });
            for piece in pieces {
                builder.add_piece(piece);
            }
        }),
    })
}

pub fn after_place(
    region: &WorldGenRegion<'_>,
    chunk_bb: BoundingBox,
    pieces: &PiecesContainer,
) {
    let min_y = region.min_y();
    let bounding_box = pieces.calculate_bounding_box();
    let y_start = bounding_box.min_y();

    for x in chunk_bb.min_x()..=chunk_bb.max_x() {
        for z in chunk_bb.min_z()..=chunk_bb.max_z() {
            let pos = BlockPos::new(x, y_start, z);
            if region.get_block(pos).is_air()
                || !bounding_box.is_inside(pos)
                || !pieces.is_inside_piece(pos)
            {
                continue;
            }
            for y in (min_y + 1..y_start).rev() {
                let pos = BlockPos::new(x, y, z);
                let state = region.get_block(pos);
                if !state.is_air() && !state.get_block().config.liquid {
                    break;
                }
                region.set_block(pos, vanilla_blocks::COBBLESTONE.default_state());
            }
        }
    }
}
