use std::cell::RefCell;

use crate::world::worldgen::biome::biome_manager::BiomeManager;
use crate::world::worldgen::worldgen_context::WorldGenContext;
use shulkr::world::block::{Block, BlockState};

use crate::util::pos::ChunkPos;
use crate::util::rng::RandomSource;
use crate::world::chunk::heightmap::HeightmapType;
use crate::world::chunk::proto::Chunk;
use crate::world::chunk::status::ChunkStatus;
use shulkr::util::BlockPosition;

pub const FEATURE_REGION_RADIUS: i32 = 1;

pub struct WorldGenRegion<'a> {
    center: ChunkPos,
    read_radius: i32,
    chunks: Vec<&'a Chunk>,
    min_y: i32,
    height: i32,
    generation_context: WorldGenContext,
    seed: i64,
    biome_zoom_seed: i64,
    random: RefCell<RandomSource>,
}

impl<'a> WorldGenRegion<'a> {
    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors vanilla WorldGenRegion construction"
    )]
    pub const fn new(
        center: ChunkPos,
        read_radius: i32,
        chunks: Vec<&'a Chunk>,
        min_y: i32,
        height: i32,
        generation_context: WorldGenContext,
        seed: i64,
        biome_zoom_seed: i64,
        random: RandomSource,
    ) -> Self {
        Self {
            center,
            read_radius,
            chunks,
            min_y,
            height,
            generation_context,
            seed,
            biome_zoom_seed,
            random: RefCell::new(random),
        }
    }

    pub fn with_random<R>(&self, f: impl FnOnce(&mut RandomSource) -> R) -> R {
        f(&mut self.random.borrow_mut())
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
        self.generation_context.sea_level()
    }

    pub const fn seed(&self) -> i64 {
        self.seed
    }

    pub const fn is_inside_build_height(&self, y: i32) -> bool {
        y >= self.min_y && y < self.min_y + self.height
    }

    pub fn chunk(&self, pos: ChunkPos) -> Option<&'a Chunk> {
        let dx = pos.0.x - self.center.0.x;
        let dz = pos.0.y - self.center.0.y;
        if dx.abs() > self.read_radius || dz.abs() > self.read_radius {
            return None;
        }
        let size = (self.read_radius * 2 + 1) as usize;
        let index = (dz + self.read_radius) as usize * size + (dx + self.read_radius) as usize;
        Some(self.chunks[index])
    }

    fn chunk_at_block(&self, pos: BlockPosition) -> Option<&'a Chunk> {
        self.chunk(ChunkPos::new(pos.x() >> 4, pos.z() >> 4))
    }

    pub const fn ensure_can_write(&self, pos: BlockPosition) -> bool {
        let dx = (pos.x() >> 4) - self.center.0.x;
        let dz = (pos.z() >> 4) - self.center.0.y;
        dx.abs() <= FEATURE_REGION_RADIUS && dz.abs() <= FEATURE_REGION_RADIUS
    }

    pub fn get_block(&self, pos: BlockPosition) -> BlockState {
        match self.chunk_at_block(pos) {
            Some(chunk) if self.is_inside_build_height(pos.y()) => chunk.get_block_state(pos),
            _ => Block::AIR.default_state(),
        }
    }

    pub fn set_block(&self, pos: BlockPosition, state: BlockState) -> bool {
        self.set_block_with_flags(pos, state, 3)
    }

    pub fn set_block_with_flags(&self, pos: BlockPosition, state: BlockState, flags: i32) -> bool {
        if !self.ensure_can_write(pos) {
            return false;
        }
        let Some(chunk) = self.chunk_at_block(pos) else {
            return false;
        };
        chunk.set_block_state(pos, state, ChunkStatus::Terrain);
        if flags & 16 == 0
            && let Some(post_process_pos) = post_process_pos(state, pos)
        {
            self.mark_pos_for_postprocessing(post_process_pos);
        }
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

    pub fn get_biome(&self, pos: BlockPosition) -> u16 {
        let noise_biome = |x, y, z| self.get_noise_biome(x, y, z);
        BiomeManager::new(&noise_biome, self.biome_zoom_seed).get_biome(pos)
    }

    pub const fn worldgen_context(&self) -> WorldGenContext {
        self.generation_context
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
        sections[local_quart_y / 4].read().get_biome(
            (quart_x & 3) as usize,
            local_quart_y % 4,
            (quart_z & 3) as usize,
        )
    }

    pub fn mark_pos_for_postprocessing(&self, pos: BlockPosition) {
        if let Some(chunk) = self.chunk_at_block(pos) {
            chunk.mark_pos_for_postprocessing(pos);
        }
    }
}

fn post_process_pos(state: BlockState, pos: BlockPosition) -> Option<BlockPosition> {
    let block = state.as_block();
    if block == Block::BROWN_MUSHROOM || block == Block::RED_MUSHROOM {
        Some(pos)
    } else if block == Block::SOUL_SAND || block == Block::MAGMA_BLOCK {
        Some(pos.above())
    } else {
        None
    }
}
