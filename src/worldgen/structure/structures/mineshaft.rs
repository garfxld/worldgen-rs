use crate::chunk::heightmap::HeightmapType;
use crate::util::pos::BlockPos;
use crate::worldgen::structure::pieces::structure_pieces_builder::StructurePiecesBuilder;
use crate::worldgen::structure::structure::{GenerationContext, GenerationStub};
use crate::worldgen::structure::structures::mineshaft_pieces::{
    self, MAGIC_START_Y, MineshaftType,
};

pub fn find_generation_point<'a>(
    context: &'a GenerationContext<'a>,
    mineshaft_type: MineshaftType,
) -> Option<GenerationStub<'a>> {
    context.with_random(|random| {
        random.next_f64();
    });

    let chunk_pos = context.chunk_pos;
    let start_pos = BlockPos::new(
        chunk_pos.0.x * 16 + 8,
        MAGIC_START_Y,
        chunk_pos.0.y * 16,
    );

    let mut builder = StructurePiecesBuilder::new();
    let y_offset = generate_pieces_and_adjust(&mut builder, context, mineshaft_type);
    let pieces = builder.take_pieces();

    Some(GenerationStub {
        position: start_pos.offset(0, y_offset, 0),
        build: Box::new(move |target| {
            for piece in pieces {
                target.add_piece(piece);
            }
        }),
    })
}

fn generate_pieces_and_adjust(
    builder: &mut StructurePiecesBuilder,
    context: &GenerationContext<'_>,
    mineshaft_type: MineshaftType,
) -> i32 {
    let chunk_pos = context.chunk_pos;
    context.with_random(|random| {
        let room = mineshaft_pieces::new_room(
            0,
            random,
            chunk_pos.0.x * 16 + 2,
            chunk_pos.0.y * 16 + 2,
            mineshaft_type,
        );
        let start_bb = room.bounding_box();
        let index = builder.add_piece_indexed(room);
        mineshaft_pieces::add_children(index, start_bb, mineshaft_type, builder, random);

        let sea_level = context.generator.sea_level();
        if mineshaft_type == MineshaftType::Mesa {
            let center = builder.get_bounding_box().center();
            let surface_height = context.generator.get_base_height(
                center.x(),
                center.z(),
                HeightmapType::WorldSurfaceWg,
            );
            let target_y_for_center = if surface_height <= sea_level {
                sea_level
            } else {
                random.next_i32_between(sea_level, surface_height)
            };
            let dy = target_y_for_center - center.y();
            builder.offset_pieces_vertically(dy);
            dy
        } else {
            builder.move_below_sea_level(sea_level, context.min_y, random, 10)
        }
    })
}
