use crate::chunk::proto::Chunk;
use crate::util::ident::BlockState;
use crate::util::pos::{BlockPos, ChunkPos};

pub struct ChunkView<'a> {
    chunk: &'a Chunk,
}

impl<'a> ChunkView<'a> {
    pub(crate) const fn new(chunk: &'a Chunk) -> Self {
        Self { chunk }
    }

    pub const fn pos(&self) -> ChunkPos {
        self.chunk.pos()
    }

    pub const fn min_y(&self) -> i32 {
        self.chunk.min_y()
    }

    pub const fn height(&self) -> i32 {
        self.chunk.height()
    }

    pub fn get_relative_block(
        &self,
        relative_x: usize,
        relative_y: usize,
        relative_z: usize,
    ) -> Option<BlockState> {
        self.chunk
            .get_relative_block(relative_x, relative_y, relative_z)
    }

    pub fn get_block_state(&self, pos: BlockPos) -> BlockState {
        self.chunk.get_block_state(pos)
    }
}
