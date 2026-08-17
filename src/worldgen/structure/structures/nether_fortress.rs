use crate::util::pos::BlockPos;
use crate::worldgen::structure::structure::{GenerationContext, GenerationStub};
use crate::worldgen::structure::structures::nether_fortress_pieces::{
    self, MAGIC_START_Y,
};

pub fn find_generation_point<'a>(
    context: &'a GenerationContext<'a>,
) -> Option<GenerationStub<'a>> {
    let chunk_pos = context.chunk_pos;
    let start_pos = BlockPos::new(chunk_pos.0.x * 16, MAGIC_START_Y, chunk_pos.0.y * 16);
    Some(GenerationStub {
        position: start_pos,
        build: Box::new(move |builder| {
            context.with_random(|random| {
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
