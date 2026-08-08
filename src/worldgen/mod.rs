pub mod backend;
pub mod biome_source;
pub mod climate;
pub mod density;
pub mod generator;
pub mod scheduler;
pub mod surface;

use std::result::Result as StdResult;

use crate::chunk::status::ChunkStatus;
use crate::chunk::view::ChunkView;
use crate::error::Error;
use crate::util::ident::Identifier;
use crate::util::pos::ChunkPos;

pub type Result<T> = StdResult<T, Error>;

#[derive(Debug, Clone)]
pub struct WorldgenSettings {
    pub seed: i64,
    pub dimension: Identifier,
}

pub struct Worldgen {}

impl Worldgen {
    pub fn new(_settings: WorldgenSettings) -> Result<Self> {
        todo!()
    }

    pub fn generate<T>(
        &self,
        _pos: ChunkPos,
        _status: ChunkStatus,
        _read: impl FnOnce(&ChunkView) -> T,
    ) -> Result<T> {
        todo!()
    }
}
