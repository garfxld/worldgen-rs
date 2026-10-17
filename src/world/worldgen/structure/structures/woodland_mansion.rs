use crate::util::rotation::RotationExt;
use crate::world::worldgen::region::WorldGenRegion;
use crate::world::worldgen::structure::pieces::pieces_container::PiecesContainer;
use crate::world::worldgen::structure::structure::{
    GenerationContext, GenerationStub, get_lowest_y_in_5by5_box,
};
use crate::world::worldgen::structure::structures::woodland_mansion_pieces;
use shulkr::util::BlockPosition;
use shulkr::util::BoundingBox;
use shulkr::world::block::Block;
use shulkr::world::block::Rotation;

pub fn find_generation_point<'a>(cx: &'a GenerationContext<'a>) -> Option<GenerationStub<'a>> {
    let block_x = cx.chunk_pos.0.x * 16 + 7;
    let block_z = cx.chunk_pos.0.y * 16 + 7;
    if !cx.could_valid_biome_exist_in_terrain_column(block_x, block_z) {
        return None;
    }
    let rotation = cx.with_random(Rotation::get_random);
    let start_pos = get_lowest_y_in_5by5_box(cx, block_x, block_z, rotation);
    if start_pos.y() < 60 {
        return None;
    }
    Some(GenerationStub {
        position: start_pos,
        build: Box::new(move |builder| {
            let mut pieces = Vec::new();
            cx.with_random(|random| {
                woodland_mansion_pieces::generate_mansion(start_pos, rotation, &mut pieces, random);
            });
            for piece in pieces {
                builder.add_piece(piece);
            }
        }),
    })
}

pub fn after_place(region: &WorldGenRegion<'_>, chunk_bb: BoundingBox, pieces: &PiecesContainer) {
    let min_y = region.min_y();
    let bounding_box = pieces.calculate_bounding_box();
    let y_start = bounding_box.min_y();

    for x in chunk_bb.min_x()..=chunk_bb.max_x() {
        for z in chunk_bb.min_z()..=chunk_bb.max_z() {
            let pos = BlockPosition::new(x, y_start, z);
            if region.get_block(pos).is_air()
                || !bounding_box.is_inside(pos)
                || !pieces.is_inside_piece(pos)
            {
                continue;
            }
            for y in (min_y + 1..y_start).rev() {
                let pos = BlockPosition::new(x, y, z);
                let state = region.get_block(pos);
                if !state.is_air() && !state.liquid() {
                    break;
                }
                region.set_block(pos, Block::COBBLESTONE.default_state());
            }
        }
    }
}
