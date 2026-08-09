use std::marker::PhantomData;

use parking_lot::{Mutex, RwLock, RwLockReadGuard};

use crate::chunk::heightmap::{ChunkHeightmaps, Heightmap, HeightmapType};
use crate::chunk::section::Sections;
use crate::chunk::status::ChunkStatus;
use crate::registry::REGISTRY;
use crate::registry::blocks::vanilla_blocks;
use crate::util::ident::BlockStateId;
use crate::util::pos::{BlockPos, ChunkPos};

fn empty_postprocessing(height: i32) -> Box<[Vec<u16>]> {
    let section_count = (height / 16) as usize;
    (0..section_count).map(|_| Vec::new()).collect()
}

pub struct Chunk {
    sections: Sections,
    pos: ChunkPos,
    min_y: i32,
    height: i32,
    heightmaps: RwLock<ChunkHeightmaps>,
    postprocessing: Mutex<Box<[Vec<u16>]>>,
}

impl Chunk {
    #[must_use]
    pub fn new(sections: Sections, pos: ChunkPos, min_y: i32, height: i32) -> Self {
        Self {
            postprocessing: Mutex::new(empty_postprocessing(height)),
            sections,
            pos,
            min_y,
            height,
            heightmaps: RwLock::new(ChunkHeightmaps::empty()),
        }
    }

    #[must_use]
    pub const fn pos(&self) -> ChunkPos {
        self.pos
    }

    #[must_use]
    pub const fn min_y(&self) -> i32 {
        self.min_y
    }

    #[must_use]
    pub const fn height(&self) -> i32 {
        self.height
    }

    #[must_use]
    pub const fn sections(&self) -> &Sections {
        &self.sections
    }

    #[must_use]
    pub fn get_relative_block(
        &self,
        relative_x: usize,
        relative_y: usize,
        relative_z: usize,
    ) -> Option<BlockStateId> {
        self.sections
            .get_relative_block(relative_x, relative_y, relative_z)
    }

    #[must_use]
    pub fn get_block_state(&self, pos: BlockPos) -> BlockStateId {
        let y = pos.0.y;
        if y < self.min_y || y >= self.min_y + self.height {
            return REGISTRY.blocks.get_default_state_id(&vanilla_blocks::AIR);
        }

        let section_index = self.get_section_index(y);
        let section = &self.sections.sections[section_index];
        let guard = section.read();

        let local_x = (pos.0.x & 15) as usize;
        let local_y = (y & 15) as usize;
        let local_z = (pos.0.z & 15) as usize;

        guard.states.get(local_x, local_y, local_z)
    }

    const fn get_section_index(&self, y: i32) -> usize {
        ((y - self.min_y) / 16) as usize
    }

    pub(crate) fn write_block_batch_for_generation(
        &self,
        blocks: &[(usize, usize, usize, BlockStateId)],
    ) {
        if blocks.is_empty() {
            return;
        }
        self.sections.write_block_batch(blocks);
    }

    pub(crate) fn write_column_blocks_for_generation(
        &self,
        x: usize,
        z: usize,
        blocks: &[(usize, BlockStateId)],
    ) {
        if blocks.is_empty() {
            return;
        }
        self.sections.write_column_blocks(x, z, blocks);
    }

    pub(crate) fn set_relative_block_for_generation(
        &self,
        status: ChunkStatus,
        relative_x: usize,
        relative_y: usize,
        relative_z: usize,
        value: BlockStateId,
    ) {
        self.sections
            .set_relative_block_for_generation(relative_x, relative_y, relative_z, value);
        self.update_heightmaps_after_block_change(
            status.heightmaps_after(),
            relative_x,
            self.min_y + relative_y as i32,
            relative_z,
            value,
        );
    }

    pub(crate) fn prime_heightmaps(&self, heightmap_types: &[HeightmapType]) {
        self.heightmaps.write().prime_from_sections(
            heightmap_types,
            self.min_y,
            &self.sections.sections,
        );
    }

    #[must_use]
    pub(crate) fn generation_height_at(
        &self,
        heightmap_type: HeightmapType,
        local_x: usize,
        local_z: usize,
    ) -> i32 {
        {
            let heightmaps = self.heightmaps.read();
            if let Some(heightmap) = heightmaps.get(heightmap_type) {
                return heightmap.get_first_available(local_x, local_z);
            }
        }
        self.prime_heightmaps(&[heightmap_type]);
        let heightmaps = self.heightmaps.read();
        let Some(heightmap) = heightmaps.get(heightmap_type) else {
            panic!("heightmap {heightmap_type:?} missing after priming");
        };
        heightmap.get_first_available(local_x, local_z)
    }

    pub(crate) fn generation_heightmaps(&self) -> RwLockReadGuard<'_, ChunkHeightmaps> {
        self.heightmaps.read()
    }

    pub(crate) fn update_heightmaps_after_direct_column_writes(
        &self,
        status: ChunkStatus,
        local_x: usize,
        local_z: usize,
        relative_writes: &[(usize, BlockStateId)],
    ) {
        if relative_writes.is_empty() {
            return;
        }
        self.update_heightmaps_after_column_block_changes(
            status.heightmaps_after(),
            local_x,
            local_z,
            relative_writes,
        );
    }

    fn update_heightmaps_after_block_change(
        &self,
        heightmap_types: &[HeightmapType],
        local_x: usize,
        y: i32,
        local_z: usize,
        state: BlockStateId,
    ) {
        let min_y = self.min_y;
        let sections = &self.sections;

        let get_block = |lx: usize, scan_y: i32, lz: usize| {
            let scan_section_index = ((scan_y - min_y) / 16) as usize;
            let scan_local_y = ((scan_y - min_y) % 16) as usize;
            sections.sections[scan_section_index]
                .read()
                .states
                .get(lx, scan_local_y, lz)
        };

        let mut heightmaps = self.heightmaps.write();
        heightmaps.prime_from_sections(heightmap_types, min_y, &sections.sections);

        for &hm_type in heightmap_types {
            let Some(heightmap) = heightmaps.get_mut(hm_type) else {
                panic!("heightmap {hm_type:?} missing after priming");
            };
            heightmap.update(local_x, y, local_z, state, get_block);
        }
    }

    fn update_heightmaps_after_column_block_changes(
        &self,
        heightmap_types: &[HeightmapType],
        local_x: usize,
        local_z: usize,
        relative_writes: &[(usize, BlockStateId)],
    ) {
        let min_y = self.min_y;
        let sections = &self.sections;

        let get_block = |lx: usize, scan_y: i32, lz: usize| {
            let scan_section_index = ((scan_y - min_y) / 16) as usize;
            let scan_local_y = ((scan_y - min_y) % 16) as usize;
            sections.sections[scan_section_index]
                .read()
                .states
                .get(lx, scan_local_y, lz)
        };

        let mut heightmaps = self.heightmaps.write();
        heightmaps.prime_from_sections(heightmap_types, min_y, &sections.sections);

        for &(relative_y, state) in relative_writes {
            let y = min_y + relative_y as i32;
            for &hm_type in heightmap_types {
                let Some(heightmap) = heightmaps.get_mut(hm_type) else {
                    panic!("heightmap {hm_type:?} missing after priming");
                };
                heightmap.update(local_x, y, local_z, state, get_block);
            }
        }
    }

    #[must_use]
    const fn pack_postprocessing_offset(pos: BlockPos) -> u16 {
        let x = (pos.0.x & 15) as u16;
        let y = (pos.0.y & 15) as u16;
        let z = (pos.0.z & 15) as u16;
        x | (y << 4) | (z << 8)
    }

    #[must_use]
    pub fn unpack_postprocessing_offset(
        packed: u16,
        section_y: i32,
        chunk_pos: ChunkPos,
    ) -> BlockPos {
        let x = chunk_pos.0.x * 16 + i32::from(packed & 15);
        let y = section_y * 16 + i32::from((packed >> 4) & 15);
        let z = chunk_pos.0.y * 16 + i32::from((packed >> 8) & 15);
        BlockPos::new(x, y, z)
    }

    pub(crate) fn mark_pos_for_postprocessing(&self, pos: BlockPos) {
        let y = pos.0.y;
        if y < self.min_y || y >= self.min_y + self.height {
            return;
        }

        let section_index = self.get_section_index(y);
        let packed = Self::pack_postprocessing_offset(pos);
        self.postprocessing.lock()[section_index].push(packed);
    }
}

pub enum NoisePhase {}
pub enum SurfacePhase {}
pub enum CarversPhase {}

#[repr(transparent)]
pub struct GenerationChunk<'a, Phase> {
    chunk: &'a Chunk,
    phase: PhantomData<fn() -> Phase>,
}

impl<Phase> Copy for GenerationChunk<'_, Phase> {}

impl<Phase> Clone for GenerationChunk<'_, Phase> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, Phase> GenerationChunk<'a, Phase> {
    #[must_use]
    pub const fn from_chunk(chunk: &'a Chunk) -> Self {
        Self {
            chunk,
            phase: PhantomData,
        }
    }

    #[must_use]
    pub const fn pos(self) -> ChunkPos {
        self.chunk.pos()
    }

    #[must_use]
    pub const fn min_y(self) -> i32 {
        self.chunk.min_y()
    }

    #[must_use]
    pub const fn height(self) -> i32 {
        self.chunk.height()
    }

    #[must_use]
    #[inline]
    pub fn section_count(self) -> usize {
        self.chunk.sections().sections.len()
    }

    #[must_use]
    #[inline]
    pub fn get_relative_block(
        self,
        relative_x: usize,
        relative_y: usize,
        relative_z: usize,
    ) -> Option<BlockStateId> {
        self.chunk
            .get_relative_block(relative_x, relative_y, relative_z)
    }

    #[must_use]
    #[inline]
    pub fn get_block_state(self, pos: BlockPos) -> BlockStateId {
        self.chunk.get_block_state(pos)
    }
}

impl GenerationChunk<'_, NoisePhase> {
    #[inline]
    pub fn write_block_batch(self, blocks: &[(usize, usize, usize, BlockStateId)]) {
        self.chunk.write_block_batch_for_generation(blocks);
    }

    pub fn replace_noise_heightmaps(self, ocean_floor: Heightmap, world_surface: Heightmap) {
        assert_eq!(ocean_floor.heightmap_type(), HeightmapType::OceanFloorWg);
        assert_eq!(
            world_surface.heightmap_type(),
            HeightmapType::WorldSurfaceWg
        );
        let mut heightmaps = self.chunk.heightmaps.write();
        heightmaps.replace(ocean_floor);
        heightmaps.replace(world_surface);
    }

    #[inline]
    pub fn mark_pos_for_postprocessing(self, pos: BlockPos) {
        self.chunk.mark_pos_for_postprocessing(pos);
    }

    #[inline]
    pub fn set_relative_block(
        self,
        relative_x: usize,
        relative_y: usize,
        relative_z: usize,
        state: BlockStateId,
    ) {
        self.chunk.set_relative_block_for_generation(
            ChunkStatus::Biomes,
            relative_x,
            relative_y,
            relative_z,
            state,
        );
    }
}

impl GenerationChunk<'_, SurfacePhase> {
    pub fn prime_world_surface_heightmap(self) {
        self.chunk
            .prime_heightmaps(&[HeightmapType::WorldSurfaceWg]);
    }

    #[must_use]
    pub fn read_all_biomes(self) -> Box<[u16]> {
        self.chunk.sections().read_all_biomes()
    }

    #[inline]
    pub fn read_column_into(self, local_x: usize, local_z: usize, output: &mut Vec<BlockStateId>) {
        self.chunk
            .sections()
            .read_column_into(local_x, local_z, output);
    }

    #[must_use]
    #[inline]
    pub fn world_surface_height_at(self, local_x: usize, local_z: usize) -> i32 {
        self.chunk
            .generation_height_at(HeightmapType::WorldSurfaceWg, local_x, local_z)
    }

    #[inline]
    pub fn write_column(self, local_x: usize, local_z: usize, blocks: &[(usize, BlockStateId)]) {
        self.chunk
            .write_column_blocks_for_generation(local_x, local_z, blocks);
        self.chunk.update_heightmaps_after_direct_column_writes(
            ChunkStatus::Noise,
            local_x,
            local_z,
            blocks,
        );
    }

    #[inline]
    pub fn set_relative_block(
        self,
        relative_x: usize,
        relative_y: usize,
        relative_z: usize,
        state: BlockStateId,
    ) {
        self.chunk.set_relative_block_for_generation(
            ChunkStatus::Noise,
            relative_x,
            relative_y,
            relative_z,
            state,
        );
    }
}

impl GenerationChunk<'_, CarversPhase> {
    pub fn prime_world_surface_heightmap(self) {
        self.chunk
            .prime_heightmaps(&[HeightmapType::WorldSurfaceWg]);
    }

    #[inline]
    pub fn set_block_state(self, pos: BlockPos, state: BlockStateId) {
        let relative_x = (pos.0.x & 15) as usize;
        let relative_y = (pos.0.y - self.chunk.min_y) as usize;
        let relative_z = (pos.0.z & 15) as usize;
        self.chunk.set_relative_block_for_generation(
            ChunkStatus::Surface,
            relative_x,
            relative_y,
            relative_z,
            state,
        );
    }

    pub fn with_world_surface_heightmap<R>(self, f: impl FnOnce(&Heightmap) -> R) -> Option<R> {
        let heightmaps = self.chunk.generation_heightmaps();
        if let Some(heightmap) = heightmaps.get(HeightmapType::WorldSurfaceWg) {
            return Some(f(heightmap));
        }
        drop(heightmaps);

        self.prime_world_surface_heightmap();
        let heightmaps = self.chunk.generation_heightmaps();
        heightmaps.get(HeightmapType::WorldSurfaceWg).map(f)
    }

    #[inline]
    pub fn mark_pos_for_postprocessing(self, pos: BlockPos) {
        self.chunk.mark_pos_for_postprocessing(pos);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::section::ChunkSection;
    use crate::registry::{REGISTRY, Registries};

    fn init_test_registry() {
        use std::sync::Once;
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let mut registries = Registries::new_vanilla().expect("vanilla datapack should load");
            registries.freeze();
            let _ = REGISTRY.init(registries);
        });
    }

    fn test_chunk() -> Chunk {
        let sections = (0..2)
            .map(|_| ChunkSection::new_empty())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Chunk::new(Sections::from_owned(sections), ChunkPos::new(0, 0), -16, 32)
    }

    #[test]
    fn set_relative_block_crosses_section_boundary() {
        init_test_registry();
        let stone = vanilla_blocks::STONE.default_state();

        let chunk = test_chunk();
        chunk.set_relative_block_for_generation(
            ChunkStatus::Noise,
            0,
            15,
            0,
            stone,
        );
        chunk.set_relative_block_for_generation(
            ChunkStatus::Noise,
            0,
            16,
            0,
            stone,
        );

        assert_eq!(chunk.get_relative_block(0, 15, 0), Some(stone));
        assert_eq!(chunk.get_relative_block(0, 16, 0), Some(stone));
        assert_eq!(chunk.get_block_state(BlockPos::new(0, -1, 0)), stone);
        assert_eq!(chunk.get_block_state(BlockPos::new(0, 0, 0)), stone);
    }

    #[test]
    fn set_relative_block_updates_worldgen_heightmap() {
        init_test_registry();
        let stone = vanilla_blocks::STONE.default_state();

        let chunk = test_chunk();
        chunk.set_relative_block_for_generation(
            ChunkStatus::Noise,
            0,
            20,
            0,
            stone,
        );

        assert_eq!(chunk.generation_height_at(HeightmapType::WorldSurfaceWg, 0, 0), 5);
    }

    #[test]
    fn generation_chunk_typestate_gates_phase_specific_writes() {
        init_test_registry();
        let stone = vanilla_blocks::STONE.default_state();

        let chunk = test_chunk();
        let noise = GenerationChunk::<NoisePhase>::from_chunk(&chunk);
        noise.set_relative_block(0, 0, 0, stone);

        assert_eq!(noise.get_relative_block(0, 0, 0), Some(stone));
    }
}
