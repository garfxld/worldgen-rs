use crate::registry::blocks::vanilla_blocks;
use crate::util::direction::Direction;
use crate::util::pos::BlockPos;
use crate::util::value_providers::height::HeightProvider;
use crate::worldgen::structure::structure::{GenerationContext, GenerationStub};
use crate::worldgen::structure::structures::nether_fossil_pieces;

pub fn find_generation_point<'a>(
    context: &'a GenerationContext<'a>,
    height: HeightProvider,
) -> Option<GenerationStub<'a>> {
    let chunk_pos = context.chunk_pos;
    let (block_x, block_z, mut y) = context.with_random(|random| {
        let block_x = chunk_pos.0.x * 16 + random.next_i32_bounded(16);
        let block_z = chunk_pos.0.y * 16 + random.next_i32_bounded(16);
        let y = height.sample(random, context.min_y, context.height);
        (block_x, block_z, y)
    });

    let sea_level = context.generator.sea_level();
    let column = context.generator.get_base_column(block_x, block_z);

    while y > sea_level {
        let current = column.get_block(y);
        y -= 1;
        let below = column.get_block(y);
        if current.is_air()
            && (below.get_block() == &vanilla_blocks::SOUL_SAND
                || below.is_face_sturdy_at(BlockPos::new(block_x, y, block_z), Direction::Up))
        {
            break;
        }
    }

    if y <= sea_level {
        return None;
    }

    let position = BlockPos::new(block_x, y, block_z);
    Some(GenerationStub {
        position,
        build: Box::new(move |builder| {
            context.with_random(|random| {
                nether_fossil_pieces::add_pieces(builder, random, position);
            });
        }),
    })
}
