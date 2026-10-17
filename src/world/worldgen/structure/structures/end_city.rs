use crate::util::rotation::RotationExt;
use crate::world::worldgen::structure::structure::{
    GenerationContext, GenerationStub, get_lowest_y_in_5by5_box,
};
use crate::world::worldgen::structure::structures::end_city_pieces;
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
                end_city_pieces::start_house_tower(start_pos, rotation, &mut pieces, random);
            });
            for piece in pieces {
                builder.add_piece(piece);
            }
        }),
    })
}
