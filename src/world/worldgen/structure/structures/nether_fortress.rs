use crate::world::worldgen::structure::structure::{GenerationContext, GenerationStub};
use crate::world::worldgen::structure::structures::nether_fortress_pieces::{self, MAGIC_START_Y};
use shulkr::util::BlockPosition;

pub fn find_generation_point<'a>(cx: &'a GenerationContext<'a>) -> Option<GenerationStub<'a>> {
    let chunk_pos = cx.chunk_pos;
    let start_pos = BlockPosition::new(chunk_pos.0.x * 16, MAGIC_START_Y, chunk_pos.0.y * 16);
    Some(GenerationStub {
        position: start_pos,
        build: Box::new(move |builder| {
            cx.with_random(|random| {
                nether_fortress_pieces::generate_pieces(
                    builder,
                    random,
                    chunk_pos.0.x * 16 + 2,
                    chunk_pos.0.y * 16 + 2,
                );
            });
        }),
    })
}
