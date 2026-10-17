use crate::world::worldgen::structure::structure::{GenerationContext, GenerationStub};
use crate::world::worldgen::structure::structures::stronghold_pieces;
use shulkr::util::BlockPosition;

pub fn find_generation_point<'a>(cx: &'a GenerationContext<'a>) -> Option<GenerationStub<'a>> {
    let chunk_pos = cx.chunk_pos;
    Some(GenerationStub {
        position: BlockPosition::new(chunk_pos.0.x * 16, 0, chunk_pos.0.y * 16),
        build: Box::new(move |builder| {
            cx.with_random(|random| {
                stronghold_pieces::generate_pieces(
                    builder,
                    random,
                    cx.seed,
                    chunk_pos.0.x,
                    chunk_pos.0.y,
                    cx.generator.sea_level(),
                    cx.min_y,
                );
            });
        }),
    })
}
