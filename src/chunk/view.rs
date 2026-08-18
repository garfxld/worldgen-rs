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

    pub fn get_relative_biome(
        &self,
        relative_x: usize,
        relative_y: usize,
        relative_z: usize,
    ) -> Option<u16> {
        self.chunk
            .get_relative_biome(relative_x, relative_y, relative_z)
    }

    pub fn blocks(&self) -> impl Iterator<Item = (BlockPos, BlockState)> + '_ {
        let min_y = self.min_y();
        let height = self.height() as usize;
        let world_x0 = self.pos().0.x * 16;
        let world_z0 = self.pos().0.y * 16;

        (0..height).flat_map(move |relative_y| {
            (0..16usize).flat_map(move |relative_x| {
                (0..16usize).filter_map(move |relative_z| {
                    let state = self.get_relative_block(relative_x, relative_y, relative_z)?;
                    if state.is_air() {
                        return None;
                    }
                    let pos = BlockPos::new(
                        world_x0 + relative_x as i32,
                        min_y + relative_y as i32,
                        world_z0 + relative_z as i32,
                    );
                    Some((pos, state))
                })
            })
        })
    }

    pub fn biomes(&self) -> impl Iterator<Item = (BlockPos, u16)> + '_ {
        let min_y = self.min_y();
        let height = self.height() as usize;
        let world_x0 = self.pos().0.x * 16;
        let world_z0 = self.pos().0.y * 16;

        (0..height).step_by(4).flat_map(move |relative_y| {
            (0..16usize).step_by(4).flat_map(move |relative_x| {
                (0..16usize).step_by(4).filter_map(move |relative_z| {
                    let biome = self.get_relative_biome(relative_x, relative_y, relative_z)?;
                    let pos = BlockPos::new(
                        world_x0 + relative_x as i32,
                        min_y + relative_y as i32,
                        world_z0 + relative_z as i32,
                    );
                    Some((pos, biome))
                })
            })
        })
    }
}
