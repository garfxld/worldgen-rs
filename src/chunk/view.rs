use crate::chunk::proto::Chunk;
use crate::util::ident::BlockStateId;
use crate::util::pos::{BlockPos, ChunkPos};

/// Read-only view over a generated chunk, handed to [`crate::Worldgen::generate`]'s
/// callback. Only exposes reads — writes go through the generation pipeline.
pub struct ChunkView<'a> {
    chunk: &'a Chunk,
}

impl<'a> ChunkView<'a> {
    #[must_use]
    pub(crate) const fn new(chunk: &'a Chunk) -> Self {
        Self { chunk }
    }

    #[must_use]
    pub const fn pos(&self) -> ChunkPos {
        self.chunk.pos()
    }

    #[must_use]
    pub const fn min_y(&self) -> i32 {
        self.chunk.min_y()
    }

    #[must_use]
    pub const fn height(&self) -> i32 {
        self.chunk.height()
    }

    #[must_use]
    pub fn get_relative_block(
        &self,
        relative_x: usize,
        relative_y: usize,
        relative_z: usize,
    ) -> Option<BlockStateId> {
        self.chunk
            .get_relative_block(relative_x, relative_y, relative_z)
    }

    #[must_use]
    pub fn get_block_state(&self, pos: BlockPos) -> BlockStateId {
        self.chunk.get_block_state(pos)
    }
}
