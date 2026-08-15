use crate::chunk::heightmap::HeightmapType;
use crate::chunk::proto::Chunk;
use crate::chunk::status::ChunkStatus;
use crate::registry::REGISTRY;
use crate::registry::blocks::vanilla_blocks;
use crate::util::ident::BlockState;
use crate::util::pos::{BlockPos, ChunkPos};

pub const FEATURE_REGION_RADIUS: i32 = 1;
const FEATURE_REGION_SIZE: usize = 3;

pub struct WorldGenRegion<'a> {
    center: ChunkPos,
    chunks: [&'a Chunk; FEATURE_REGION_SIZE * FEATURE_REGION_SIZE],
    min_y: i32,
    height: i32,
    sea_level: i32,
    seed: i64,
    biome_zoom_seed: i64,
}

impl<'a> WorldGenRegion<'a> {
    pub const fn new(
        center: ChunkPos,
        chunks: [&'a Chunk; FEATURE_REGION_SIZE * FEATURE_REGION_SIZE],
        min_y: i32,
        height: i32,
        sea_level: i32,
        seed: i64,
        biome_zoom_seed: i64,
    ) -> Self {
        Self {
            center,
            chunks,
            min_y,
            height,
            sea_level,
            seed,
            biome_zoom_seed,
        }
    }

    pub const fn biome_zoom_seed(&self) -> i64 {
        self.biome_zoom_seed
    }

    pub const fn center(&self) -> ChunkPos {
        self.center
    }

    pub const fn min_y(&self) -> i32 {
        self.min_y
    }

    pub const fn max_y(&self) -> i32 {
        self.min_y + self.height
    }

    pub const fn sea_level(&self) -> i32 {
        self.sea_level
    }

    pub const fn seed(&self) -> i64 {
        self.seed
    }

    pub const fn is_inside_build_height(&self, y: i32) -> bool {
        y >= self.min_y && y < self.min_y + self.height
    }

    pub const fn chunk(&self, pos: ChunkPos) -> Option<&'a Chunk> {
        let dx = pos.0.x - self.center.0.x;
        let dz = pos.0.y - self.center.0.y;
        if dx.abs() > FEATURE_REGION_RADIUS || dz.abs() > FEATURE_REGION_RADIUS {
            return None;
        }
        let index = (dz + FEATURE_REGION_RADIUS) as usize * FEATURE_REGION_SIZE
            + (dx + FEATURE_REGION_RADIUS) as usize;
        Some(self.chunks[index])
    }

    const fn chunk_at_block(&self, pos: BlockPos) -> Option<&'a Chunk> {
        self.chunk(ChunkPos::new(pos.0.x >> 4, pos.0.z >> 4))
    }

    pub fn get_block_state(&self, pos: BlockPos) -> BlockState {
        match self.chunk_at_block(pos) {
            Some(chunk) if self.is_inside_build_height(pos.0.y) => chunk.get_block_state(pos),
            _ => REGISTRY.blocks.get_default_state_id(&vanilla_blocks::AIR),
        }
    }

    pub fn set_block(&self, pos: BlockPos, state: BlockState) -> bool {
        if !self.is_inside_build_height(pos.0.y) {
            return false;
        }
        let Some(chunk) = self.chunk_at_block(pos) else {
            return false;
        };
        chunk.set_relative_block_for_generation(
            ChunkStatus::Features,
            (pos.0.x & 15) as usize,
            (pos.0.y - self.min_y) as usize,
            (pos.0.z & 15) as usize,
            state,
        );
        true
    }

    pub fn get_height(&self, heightmap_type: HeightmapType, x: i32, z: i32) -> i32 {
        match self.chunk(ChunkPos::new(x >> 4, z >> 4)) {
            Some(chunk) => {
                chunk.generation_height_at(heightmap_type, (x & 15) as usize, (z & 15) as usize)
            }
            None => self.min_y,
        }
    }

    pub fn get_biome(&self, pos: BlockPos) -> u16 {
        let quart_x = pos.0.x >> 2;
        let quart_z = pos.0.z >> 2;
        let quart_y = pos.0.y >> 2;
        self.get_noise_biome(quart_x, quart_y, quart_z)
    }

    pub fn get_noise_biome(&self, quart_x: i32, quart_y: i32, quart_z: i32) -> u16 {
        let chunk_pos = ChunkPos::new(quart_x >> 2, quart_z >> 2);
        let Some(chunk) = self.chunk(chunk_pos) else {
            return 0;
        };
        let sections = &chunk.sections().sections;
        let min_quart_y = chunk.min_y() >> 2;
        let total_quarts_y = (sections.len() * 4) as i32;
        let local_quart_y = (quart_y - min_quart_y).clamp(0, total_quarts_y - 1) as usize;
        sections[local_quart_y / 4].read().biomes.get(
            (quart_x & 3) as usize,
            local_quart_y % 4,
            (quart_z & 3) as usize,
        )
    }

    pub fn mark_pos_for_postprocessing(&self, pos: BlockPos) {
        if let Some(chunk) = self.chunk_at_block(pos) {
            chunk.mark_pos_for_postprocessing(pos);
        }
    }
}
