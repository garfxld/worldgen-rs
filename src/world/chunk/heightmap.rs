use std::sync::LazyLock;

use shulkr::world::block::BlockState;
use smallvec::SmallVec;

use crate::registry::block_tags;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Deserialize)]
pub enum HeightmapType {
    #[serde(rename = "WORLD_SURFACE")]
    WorldSurface,
    #[serde(rename = "MOTION_BLOCKING")]
    MotionBlocking,
    #[serde(rename = "MOTION_BLOCKING_NO_LEAVES")]
    MotionBlockingNoLeaves,
    #[serde(rename = "OCEAN_FLOOR")]
    OceanFloor,
    #[serde(rename = "WORLD_SURFACE_WG")]
    WorldSurfaceWg,
    #[serde(rename = "OCEAN_FLOOR_WG")]
    OceanFloorWg,
}

impl HeightmapType {
    const WORLD_SURFACE_MASK: u8 = 1 << 0;
    const MOTION_BLOCKING_MASK: u8 = 1 << 1;
    const MOTION_BLOCKING_NO_LEAVES_MASK: u8 = 1 << 2;
    const OCEAN_FLOOR_MASK: u8 = 1 << 3;
    const WORLD_SURFACE_WG_MASK: u8 = 1 << 4;
    const OCEAN_FLOOR_WG_MASK: u8 = 1 << 5;

    pub const fn worldgen_types() -> &'static [HeightmapType] {
        &[HeightmapType::WorldSurfaceWg, HeightmapType::OceanFloorWg]
    }

    pub const fn final_types() -> &'static [HeightmapType] {
        &[
            HeightmapType::WorldSurface,
            HeightmapType::MotionBlocking,
            HeightmapType::MotionBlockingNoLeaves,
            HeightmapType::OceanFloor,
        ]
    }

    pub fn is_opaque(self, state: BlockState) -> bool {
        heightmap_opacity_mask(state, self.mask()) != 0
    }

    const fn mask(self) -> u8 {
        match self {
            Self::WorldSurface => Self::WORLD_SURFACE_MASK,
            Self::MotionBlocking => Self::MOTION_BLOCKING_MASK,
            Self::MotionBlockingNoLeaves => Self::MOTION_BLOCKING_NO_LEAVES_MASK,
            Self::OceanFloor => Self::OCEAN_FLOOR_MASK,
            Self::WorldSurfaceWg => Self::WORLD_SURFACE_WG_MASK,
            Self::OceanFloorWg => Self::OCEAN_FLOOR_WG_MASK,
        }
    }
}

static WORLD_SURFACE_OPACITY_MASK_BY_STATE: LazyLock<Box<[u8]>> =
    LazyLock::new(build_world_surface_opacity_masks);
static HEIGHTMAP_OPACITY_MASK_BY_STATE: LazyLock<Box<[u8]>> =
    LazyLock::new(build_state_opacity_masks);

fn build_world_surface_opacity_masks() -> Box<[u8]> {
    let states = BlockState::all();
    let mut masks = vec![0u8; states.len()];
    for state in states {
        masks[state.state_id() as usize] = if state.is_air() {
            0
        } else {
            HeightmapType::WORLD_SURFACE_MASK | HeightmapType::WORLD_SURFACE_WG_MASK
        };
    }
    masks.into_boxed_slice()
}

fn build_state_opacity_masks() -> Box<[u8]> {
    let states = BlockState::all();
    let mut masks = vec![0u8; states.len()];
    for state in states {
        let block = state.as_block();
        let mut mask = 0;
        if !state.is_air() {
            mask |= HeightmapType::WORLD_SURFACE_MASK | HeightmapType::WORLD_SURFACE_WG_MASK;
        }
        let blocks_motion = block.has_tag(&block_tags::BLOCKS_MOTION_IN_HEIGHTMAP);
        if blocks_motion {
            mask |= HeightmapType::OCEAN_FLOOR_MASK | HeightmapType::OCEAN_FLOOR_WG_MASK;
        }
        if blocks_motion || state.has_fluid() {
            mask |= HeightmapType::MOTION_BLOCKING_MASK;
        }
        if block.has_tag(&block_tags::BLOCKS_MOTION_IN_HEIGHTMAP_NO_LEAVES) || state.has_fluid() {
            mask |= HeightmapType::MOTION_BLOCKING_NO_LEAVES_MASK;
        }
        masks[state.state_id() as usize] = mask;
    }
    masks.into_boxed_slice()
}

#[inline]
fn heightmap_opacity_mask(state: BlockState, requested_mask: u8) -> u8 {
    let world_surface_mask =
        HeightmapType::WORLD_SURFACE_MASK | HeightmapType::WORLD_SURFACE_WG_MASK;
    if requested_mask & !world_surface_mask == 0 {
        let Some(&state_mask) = WORLD_SURFACE_OPACITY_MASK_BY_STATE.get(state.state_id() as usize)
        else {
            panic!("invalid block state id {}", state.state_id());
        };
        return state_mask & requested_mask;
    }

    let Some(&state_mask) = HEIGHTMAP_OPACITY_MASK_BY_STATE.get(state.state_id() as usize) else {
        panic!("invalid block state id {}", state.state_id());
    };
    state_mask & requested_mask
}

#[derive(Debug, Clone)]
pub struct Heightmap {
    data: Box<[u16; 256]>,
    map_type: HeightmapType,
    min_y: i32,
}

impl Heightmap {
    pub fn new(map_type: HeightmapType, min_y: i32) -> Self {
        Self {
            data: Box::new([0; 256]),
            map_type,
            min_y,
        }
    }

    pub const fn heightmap_type(&self) -> HeightmapType {
        self.map_type
    }

    #[inline]
    const fn get_index(local_x: usize, local_z: usize) -> usize {
        local_x + local_z * 16
    }

    pub fn get_first_available(&self, local_x: usize, local_z: usize) -> i32 {
        debug_assert!(local_x < 16 && local_z < 16);
        let index = Self::get_index(local_x, local_z);
        i32::from(self.data[index]) + self.min_y
    }

    pub fn set_height(&mut self, local_x: usize, local_z: usize, height: i32) {
        debug_assert!(local_x < 16 && local_z < 16);
        let index = Self::get_index(local_x, local_z);
        self.data[index] = (height - self.min_y) as u16;
    }

    pub fn update<F>(
        &mut self,
        local_x: usize,
        y: i32,
        local_z: usize,
        state: BlockState,
        get_block: F,
    ) -> bool
    where
        F: Fn(usize, i32, usize) -> BlockState,
    {
        let first_available = self.get_first_available(local_x, local_z);

        if y <= first_available - 2 {
            return false;
        }

        if self.map_type.is_opaque(state) {
            if y >= first_available {
                self.set_height(local_x, local_z, y + 1);
                return true;
            }
        } else if first_available - 1 == y {
            for scan_y in (self.min_y..y).rev() {
                let scan_state = get_block(local_x, scan_y, local_z);
                if self.map_type.is_opaque(scan_state) {
                    self.set_height(local_x, local_z, scan_y + 1);
                    return true;
                }
            }
            self.set_height(local_x, local_z, self.min_y);
            return true;
        }

        false
    }

    pub fn update_for_initial_fill(
        &mut self,
        local_x: usize,
        y: i32,
        local_z: usize,
        state: BlockState,
    ) -> bool {
        let first_available = self.get_first_available(local_x, local_z);
        if self.map_type.is_opaque(state) && y >= first_available {
            self.set_height(local_x, local_z, y + 1);
            return true;
        }

        false
    }
}

#[derive(Debug, Clone)]
pub struct ChunkHeightmaps {
    world_surface_wg: Option<Heightmap>,
    ocean_floor_wg: Option<Heightmap>,
    world_surface: Option<Heightmap>,
    motion_blocking: Option<Heightmap>,
    motion_blocking_no_leaves: Option<Heightmap>,
    ocean_floor: Option<Heightmap>,
}

impl ChunkHeightmaps {
    pub const fn empty() -> Self {
        Self {
            world_surface_wg: None,
            ocean_floor_wg: None,
            world_surface: None,
            motion_blocking: None,
            motion_blocking_no_leaves: None,
            ocean_floor: None,
        }
    }

    pub fn new(min_y: i32) -> Self {
        Self::with_types(HeightmapType::final_types(), min_y)
    }

    pub fn with_types(types: &[HeightmapType], min_y: i32) -> Self {
        let mut heightmaps = Self::empty();
        for &heightmap_type in types {
            heightmaps.get_or_insert(heightmap_type, min_y);
        }
        heightmaps
    }

    pub const fn get(&self, heightmap_type: HeightmapType) -> Option<&Heightmap> {
        match heightmap_type {
            HeightmapType::WorldSurfaceWg => self.world_surface_wg.as_ref(),
            HeightmapType::OceanFloorWg => self.ocean_floor_wg.as_ref(),
            HeightmapType::WorldSurface => self.world_surface.as_ref(),
            HeightmapType::MotionBlocking => self.motion_blocking.as_ref(),
            HeightmapType::MotionBlockingNoLeaves => self.motion_blocking_no_leaves.as_ref(),
            HeightmapType::OceanFloor => self.ocean_floor.as_ref(),
        }
    }

    pub const fn get_mut(&mut self, heightmap_type: HeightmapType) -> Option<&mut Heightmap> {
        match heightmap_type {
            HeightmapType::WorldSurfaceWg => self.world_surface_wg.as_mut(),
            HeightmapType::OceanFloorWg => self.ocean_floor_wg.as_mut(),
            HeightmapType::WorldSurface => self.world_surface.as_mut(),
            HeightmapType::MotionBlocking => self.motion_blocking.as_mut(),
            HeightmapType::MotionBlockingNoLeaves => self.motion_blocking_no_leaves.as_mut(),
            HeightmapType::OceanFloor => self.ocean_floor.as_mut(),
        }
    }

    pub fn replace(&mut self, heightmap: Heightmap) {
        let heightmap_type = heightmap.heightmap_type();
        match heightmap_type {
            HeightmapType::WorldSurfaceWg => self.world_surface_wg = Some(heightmap),
            HeightmapType::OceanFloorWg => self.ocean_floor_wg = Some(heightmap),
            HeightmapType::WorldSurface => self.world_surface = Some(heightmap),
            HeightmapType::MotionBlocking => self.motion_blocking = Some(heightmap),
            HeightmapType::MotionBlockingNoLeaves => {
                self.motion_blocking_no_leaves = Some(heightmap);
            }
            HeightmapType::OceanFloor => self.ocean_floor = Some(heightmap),
        }
    }

    fn get_or_insert(&mut self, heightmap_type: HeightmapType, min_y: i32) -> &mut Heightmap {
        let slot = match heightmap_type {
            HeightmapType::WorldSurfaceWg => &mut self.world_surface_wg,
            HeightmapType::OceanFloorWg => &mut self.ocean_floor_wg,
            HeightmapType::WorldSurface => &mut self.world_surface,
            HeightmapType::MotionBlocking => &mut self.motion_blocking,
            HeightmapType::MotionBlockingNoLeaves => &mut self.motion_blocking_no_leaves,
            HeightmapType::OceanFloor => &mut self.ocean_floor,
        };
        slot.get_or_insert_with(|| Heightmap::new(heightmap_type, min_y))
    }

    pub fn get_final_mut(&mut self, heightmap_type: HeightmapType) -> &mut Heightmap {
        if matches!(
            heightmap_type,
            HeightmapType::WorldSurfaceWg | HeightmapType::OceanFloorWg
        ) {
            panic!("worldgen heightmap {heightmap_type:?} is not a final chunk heightmap");
        }
        let Some(heightmap) = self.get_mut(heightmap_type) else {
            panic!("full chunk is missing required heightmap {heightmap_type:?}");
        };
        heightmap
    }

    fn set_primed_height(
        &mut self,
        heightmap_type: HeightmapType,
        local_x: usize,
        local_z: usize,
        height: i32,
    ) {
        let Some(heightmap) = self.get_mut(heightmap_type) else {
            panic!("heightmap {heightmap_type:?} missing after priming");
        };
        heightmap.set_height(local_x, local_z, height);
    }

    pub fn prime_from_sections(
        &mut self,
        types: &[HeightmapType],
        min_y: i32,
        sections: &[parking_lot::RwLock<super::section::ChunkSection>],
    ) {
        let mut types_to_prime = SmallVec::<[(HeightmapType, u8); 4]>::new();
        let mut pending_mask_base = 0;
        for &heightmap_type in types {
            if self.get(heightmap_type).is_none() {
                let mask = heightmap_type.mask();
                types_to_prime.push((heightmap_type, mask));
                pending_mask_base |= mask;
            }
        }

        if types_to_prime.is_empty() {
            return;
        }

        for &(heightmap_type, _) in &types_to_prime {
            self.get_or_insert(heightmap_type, min_y);
        }

        let mut pending_masks = [pending_mask_base; 16 * 16];
        let mut pending_columns = pending_masks.len();

        'sections: for section_idx in (0..sections.len()).rev() {
            let section = sections[section_idx].read();
            if section.has_only_air() {
                continue;
            }

            for local_y in (0..16).rev() {
                let y = min_y + (section_idx * 16 + local_y) as i32;
                let layer_start = local_y * 16 * 16;

                for (column_index, pending_mask) in pending_masks.iter_mut().enumerate() {
                    if *pending_mask == 0 {
                        continue;
                    }

                    let state = super::section::state_from_id(
                        section.get_block_at_index(layer_start + column_index),
                    );
                    let matched_mask = heightmap_opacity_mask(state, *pending_mask);
                    if matched_mask == 0 {
                        continue;
                    }

                    let x = column_index % 16;
                    let z = column_index / 16;
                    for &(heightmap_type, mask) in &types_to_prime {
                        if matched_mask & mask != 0 {
                            self.set_primed_height(heightmap_type, x, z, y + 1);
                        }
                    }
                    *pending_mask &= !matched_mask;
                    if *pending_mask == 0 {
                        pending_columns -= 1;
                    }
                }

                if pending_columns == 0 {
                    break 'sections;
                }
            }
        }
    }
}

impl Default for ChunkHeightmaps {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use crate::datapack;
    use crate::world::worldgen;
    use shulkr::world::block::{Block, Properties};

    use super::*;
    use crate::world::chunk::section::{Sections, new_section};

    fn init_test_registry() {
        use std::sync::Once;
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            worldgen::ensure_registry_initialized(&datapack::Datapack::builtin())
                .expect("vanilla datapack should load");
        });
    }

    #[test]
    fn test_get_index() {
        assert_eq!(Heightmap::get_index(0, 0), 0);
        assert_eq!(Heightmap::get_index(15, 0), 15);
        assert_eq!(Heightmap::get_index(0, 1), 16);
        assert_eq!(Heightmap::get_index(15, 15), 255);
    }

    #[test]
    fn heightmap_predicates_use_blocks_motion_in_heightmap_tag_and_fluid_state() {
        init_test_registry();

        let water = Block::WATER.default_state();
        assert!(!HeightmapType::OceanFloorWg.is_opaque(water));
        assert!(HeightmapType::MotionBlocking.is_opaque(water));

        let slab = Block::OAK_SLAB.default_state();
        let waterlogged_slab = slab.with(Properties::WATERLOGGED, true);
        assert!(waterlogged_slab.has_fluid());
        assert!(HeightmapType::MotionBlocking.is_opaque(waterlogged_slab));

        let cobweb = Block::COBWEB.default_state();
        assert!(!HeightmapType::OceanFloorWg.is_opaque(cobweb));
    }

    #[test]
    fn initial_fill_update_tracks_only_matching_blocks() {
        init_test_registry();

        let water = Block::WATER.default_state();
        let stone = Block::STONE.default_state();

        let mut ocean_floor = Heightmap::new(HeightmapType::OceanFloorWg, 0);
        assert!(!ocean_floor.update_for_initial_fill(0, 12, 0, water));
        assert_eq!(ocean_floor.get_first_available(0, 0), 0);

        assert!(ocean_floor.update_for_initial_fill(0, 5, 0, stone));
        assert_eq!(ocean_floor.get_first_available(0, 0), 6);

        assert!(!ocean_floor.update_for_initial_fill(0, 4, 0, stone));
        assert_eq!(ocean_floor.get_first_available(0, 0), 6);
    }

    #[test]
    fn section_priming_preserves_heightmap_predicates_across_sections() {
        init_test_registry();

        let stone = Block::STONE.default_state();
        let water = Block::WATER.default_state();
        let leaves = Block::OAK_LEAVES.default_state();
        let cobweb = Block::COBWEB.default_state();
        let mut lower = new_section();
        let mut upper = new_section();

        lower.set_block(0, 15, 0, stone.state_id());
        upper.set_block(0, 10, 0, water.state_id());
        upper.set_block(1, 4, 2, stone.state_id());
        upper.set_block(1, 12, 2, leaves.state_id());
        upper.set_block(3, 3, 4, stone.state_id());
        upper.set_block(3, 14, 4, cobweb.state_id());

        let sections = Sections::from_owned(vec![lower, upper].into_boxed_slice());
        let mut heightmaps = ChunkHeightmaps::empty();
        heightmaps.prime_from_sections(
            &[
                HeightmapType::WorldSurface,
                HeightmapType::MotionBlocking,
                HeightmapType::MotionBlockingNoLeaves,
                HeightmapType::OceanFloor,
                HeightmapType::WorldSurfaceWg,
                HeightmapType::OceanFloorWg,
            ],
            -16,
            &sections.sections,
        );

        let first_available = |heightmap_type, x, z| {
            let Some(heightmap) = heightmaps.get(heightmap_type) else {
                panic!("heightmap {heightmap_type:?} was not primed");
            };
            heightmap.get_first_available(x, z)
        };

        assert_eq!(first_available(HeightmapType::WorldSurface, 0, 0), 11);
        assert_eq!(first_available(HeightmapType::MotionBlocking, 0, 0), 11);
        assert_eq!(
            first_available(HeightmapType::MotionBlockingNoLeaves, 0, 0),
            11
        );
        assert_eq!(first_available(HeightmapType::OceanFloor, 0, 0), 0);
        assert_eq!(first_available(HeightmapType::WorldSurfaceWg, 0, 0), 11);
        assert_eq!(first_available(HeightmapType::OceanFloorWg, 0, 0), 0);

        assert_eq!(first_available(HeightmapType::WorldSurface, 1, 2), 13);
        assert_eq!(first_available(HeightmapType::MotionBlocking, 1, 2), 13);
        assert_eq!(
            first_available(HeightmapType::MotionBlockingNoLeaves, 1, 2),
            5
        );
        assert_eq!(first_available(HeightmapType::OceanFloor, 1, 2), 13);

        assert_eq!(first_available(HeightmapType::WorldSurface, 3, 4), 15);
        assert_eq!(first_available(HeightmapType::MotionBlocking, 3, 4), 4);
        assert_eq!(first_available(HeightmapType::OceanFloor, 3, 4), 4);
        assert_eq!(first_available(HeightmapType::WorldSurface, 15, 15), -16);
    }
}
