use crate::util::pos::BlockPos;
use crate::worldgen::structure::structure::{GenerationContext, GenerationStub};
use crate::worldgen::structure::structures::stronghold_pieces;

pub fn find_generation_point<'a>(
    context: &'a GenerationContext<'a>,
) -> Option<GenerationStub<'a>> {
    let chunk_pos = context.chunk_pos;
    Some(GenerationStub {
        position: BlockPos::new(chunk_pos.0.x * 16, 0, chunk_pos.0.y * 16),
        build: Box::new(move |builder| {
            context.with_random(|random| {
                stronghold_pieces::generate_pieces(
                    builder,
                    random,
                    context.seed,
                    chunk_pos.0.x,
                    chunk_pos.0.y,
                    context.generator.sea_level(),
                    context.min_y,
                );
            });
        }),
    })
}
