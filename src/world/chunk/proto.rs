use parking_lot::{Mutex, MutexGuard, RwLock, RwLockReadGuard};
use rustc_hash::FxHashMap;
use shulkr::util::Key;
use shulkr::world::block::{Block, BlockState};

use crate::util::pos::ChunkPos;
use crate::world::chunk::heightmap::{ChunkHeightmaps, Heightmap, HeightmapType};
use crate::world::chunk::section;
use crate::world::chunk::section::Sections;
use crate::world::chunk::status::ChunkStatus;
use crate::world::worldgen::structure::structure_start::StructureStart;
use shulkr::util::BlockPosition;

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
    structure_starts: Mutex<FxHashMap<Key, StructureStart>>,
    structure_references: Mutex<FxHashMap<Key, Vec<i64>>>,
}

impl Chunk {
    pub fn new(sections: Sections, pos: ChunkPos, min_y: i32, height: i32) -> Self {
        Self {
            postprocessing: Mutex::new(empty_postprocessing(height)),
            sections,
            pos,
            min_y,
            height,
            heightmaps: RwLock::new(ChunkHeightmaps::empty()),
            structure_starts: Mutex::new(FxHashMap::default()),
            structure_references: Mutex::new(FxHashMap::default()),
        }
    }

    pub fn structure_starts(&self) -> MutexGuard<'_, FxHashMap<Key, StructureStart>> {
        self.structure_starts.lock()
    }

    pub fn structure_references(&self) -> MutexGuard<'_, FxHashMap<Key, Vec<i64>>> {
        self.structure_references.lock()
    }

    pub const fn pos(&self) -> ChunkPos {
        self.pos
    }

    pub const fn min_y(&self) -> i32 {
        self.min_y
    }

    pub const fn height(&self) -> i32 {
        self.height
    }

    pub const fn sections(&self) -> &Sections {
        &self.sections
    }

    pub fn get_relative_block(
        &self,
        relative_x: usize,
        relative_y: usize,
        relative_z: usize,
    ) -> Option<BlockState> {
        self.sections
            .get_relative_block(relative_x, relative_y, relative_z)
    }

    pub fn get_relative_biome(
        &self,
        relative_x: usize,
        relative_y: usize,
        relative_z: usize,
    ) -> Option<u16> {
        self.sections
            .get_relative_biome(relative_x, relative_y, relative_z)
    }

    pub fn get_block_state(&self, pos: BlockPosition) -> BlockState {
        let y = pos.y();
        if y < self.min_y || y >= self.min_y + self.height {
            return Block::AIR.default_state();
        }

        let section_index = self.get_section_index(y);
        let section = &self.sections.sections[section_index];
        let guard = section.read();

        let local_x = (pos.x() & 15) as usize;
        let local_y = (y & 15) as usize;
        let local_z = (pos.z() & 15) as usize;

        section::state_from_id(guard.get_block(local_x, local_y, local_z))
    }

    const fn get_section_index(&self, y: i32) -> usize {
        ((y - self.min_y) / 16) as usize
    }

    pub(crate) fn write_block_batch_for_generation(
        &self,
        blocks: &[(usize, usize, usize, BlockState)],
    ) {
        if blocks.is_empty() {
            return;
        }
        self.sections.write_block_batch(blocks);
    }

    pub(crate) fn prime_heightmaps(&self, heightmap_types: &[HeightmapType]) {
        self.heightmaps.write().prime_from_sections(
            heightmap_types,
            self.min_y,
            &self.sections.sections,
        );
    }

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

    pub fn generation_heightmaps(&self) -> RwLockReadGuard<'_, ChunkHeightmaps> {
        self.heightmaps.read()
    }

    fn update_heightmaps_after_block_change(
        &self,
        heightmap_types: &[HeightmapType],
        local_x: usize,
        y: i32,
        local_z: usize,
        state: BlockState,
    ) {
        let min_y = self.min_y;
        let sections = &self.sections;

        let get_block = |lx: usize, scan_y: i32, lz: usize| {
            let scan_section_index = ((scan_y - min_y) / 16) as usize;
            let scan_local_y = ((scan_y - min_y) % 16) as usize;
            section::state_from_id(sections.sections[scan_section_index].read().get_block(
                lx,
                scan_local_y,
                lz,
            ))
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

    const fn pack_postprocessing_offset(pos: BlockPosition) -> u16 {
        let x = (pos.x() & 15) as u16;
        let y = (pos.y() & 15) as u16;
        let z = (pos.z() & 15) as u16;
        x | (y << 4) | (z << 8)
    }

    pub fn get_highest_filled_section_index(&self) -> i32 {
        for (section_index, section) in self.sections.sections.iter().enumerate().rev() {
            let mut guard = section.write();
            if guard.counts_are_stale() {
                guard.recalculate_counts();
            }
            if !guard.is_empty() {
                return section_index as i32;
            }
        }
        -1
    }

    pub fn get_height(&self, heightmap_type: HeightmapType, x: i32, z: i32) -> i32 {
        self.generation_height_at(heightmap_type, (x & 15) as usize, (z & 15) as usize) - 1
    }

    pub fn set_block_state(
        &self,
        pos: BlockPosition,
        state: BlockState,
        persisted_status: ChunkStatus,
    ) {
        let y = pos.y();
        if y < self.min_y || y >= self.min_y + self.height {
            return;
        }
        let section_index = self.get_section_index(y);
        let local_x = (pos.x() & 15) as usize;
        let local_z = (pos.z() & 15) as usize;
        {
            let mut section = self.sections.sections[section_index].write();
            if section.counts_are_stale() {
                section.recalculate_counts();
            }
            if section.is_empty() && state.is(Block::AIR) {
                return;
            }
            section.set_block(local_x, (y & 15) as usize, local_z, state.state_id());
        }
        self.update_heightmaps_after_block_change(
            persisted_status.heightmaps_after(),
            local_x,
            y,
            local_z,
            state,
        );
    }

    pub(crate) fn replace_noise_heightmaps(
        &self,
        ocean_floor: Heightmap,
        world_surface: Heightmap,
    ) {
        let mut heightmaps = self.heightmaps.write();
        heightmaps.replace(ocean_floor);
        heightmaps.replace(world_surface);
    }

    pub fn get_post_processing(&self) -> MutexGuard<'_, Box<[Vec<u16>]>> {
        self.postprocessing.lock()
    }

    pub(crate) fn mark_pos_for_postprocessing(&self, pos: BlockPosition) {
        let y = pos.y();
        if y < self.min_y || y >= self.min_y + self.height {
            return;
        }

        let section_index = self.get_section_index(y);
        let packed = Self::pack_postprocessing_offset(pos);
        self.postprocessing.lock()[section_index].push(packed);
    }
}
