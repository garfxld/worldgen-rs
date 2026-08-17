use crate::util::rotation::Rotation;
use crate::worldgen::structure::structure::{
    GenerationContext, GenerationStub, get_lowest_y_in_5by5_box_offset_7_blocks,
};
use crate::worldgen::structure::structures::end_city_pieces;

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
                end_city_pieces::start_house_tower(start_pos, rotation, &mut pieces, random);
            });
            for piece in pieces {
                builder.add_piece(piece);
            }
        }),
    })
}
