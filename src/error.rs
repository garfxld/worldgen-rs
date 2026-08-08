use std::io;

use crate::chunk::status::ChunkStatus;
use crate::util::pos::ChunkPos;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to build the chunk generator: {0}")]
    Generator(String),
    #[error("chunk {0:?} was not available at the requested status")]
    NotReady(ChunkPos),
    #[error("generation to {0:?} is not implemented yet")]
    Unsupported(ChunkStatus),
    #[error("failed to load datapack: {0}")]
    Datapack(#[source] io::Error),
}
