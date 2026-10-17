use crate::util::value_providers::height::HeightProvider;
use crate::world::worldgen::structure::structure::{GenerationContext, GenerationStub};
use crate::world::worldgen::structure::structures::nether_fossil_pieces;
use shulkr::util::BlockPosition;
use shulkr::util::Direction;
use shulkr::world::block::Block;

pub fn find_generation_point<'a>(
    cx: &'a GenerationContext<'a>,
    height: HeightProvider,
) -> Option<GenerationStub<'a>> {
    let chunk_pos = cx.chunk_pos;
    let (block_x, block_z, mut y) = cx.with_random(|random| {
        let block_x = chunk_pos.0.x * 16 + random.next_i32_bounded(16);
        let block_z = chunk_pos.0.y * 16 + random.next_i32_bounded(16);
        let y = height.sample(random, &cx.worldgen_context());
        (block_x, block_z, y)
    });

    let sea_level = cx.generator.sea_level();
    let column = cx.generator.get_base_column(block_x, block_z);

    while y > sea_level {
        let current = column.get_block(y);
        y -= 1;
        let below = column.get_block(y);
        if current.is_air()
            && (below.is(Block::SOUL_SAND)
                || below.is_side_solid_at(BlockPosition::new(block_x, y, block_z), Direction::Up))
        {
            break;
        }
    }

    if y <= sea_level {
        return None;
    }

    let position = BlockPosition::new(block_x, y, block_z);
    Some(GenerationStub {
        position,
        build: Box::new(move |builder| {
            cx.with_random(|random| {
                nether_fossil_pieces::add_pieces(builder, random, position);
            });
        }),
    })
}
