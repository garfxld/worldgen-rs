use std::sync::OnceLock;

use parking_lot::RwLock;

use crate::chunk::paletted_container::{BiomePalette, BlockPalette};
use crate::registry::REGISTRY;
use crate::util::ident::{BlockStateId, Identifier};
use crate::util::pos::{BlockPos, ChunkPos};

fn plains_biome_id() -> u16 {
    static PLAINS_ID: OnceLock<u16> = OnceLock::new();
    *PLAINS_ID.get_or_init(|| {
        REGISTRY
            .biomes
            .by_key(&Identifier::const_vanilla("plains"))
            .expect("vanilla biome registry should contain plains")
            .id() as u16
    })
}

#[derive(Debug)]
pub struct Sections {
    pub sections: Box<[RwLock<ChunkSection>]>,
}

const BLOCKS_PER_SECTION: u16 = 16 * 16 * 16;

impl Sections {
    #[must_use]
    pub fn from_owned(sections: Box<[ChunkSection]>) -> Self {
        let sections = sections.into_vec().into_iter().map(RwLock::new).collect();
        Self { sections }
    }

    #[must_use]
    pub fn get_relative_block(
        &self,
        relative_x: usize,
        relative_y: usize,
        relative_z: usize,
    ) -> Option<BlockStateId> {
        debug_assert!(relative_x < BlockPalette::SIZE);
        debug_assert!(relative_z < BlockPalette::SIZE);

        let section_index = relative_y / BlockPalette::SIZE;
        let relative_y = relative_y % BlockPalette::SIZE;
        self.sections.get(section_index).map(|section| {
            section
                .read()
                .states
                .get(relative_x, relative_y, relative_z)
        })
    }

    pub fn read_column_into(&self, x: usize, z: usize, buf: &mut Vec<BlockStateId>) {
        debug_assert!(x < BlockPalette::SIZE);
        debug_assert!(z < BlockPalette::SIZE);

        let total = self.sections.len() * 16;
        if buf.len() != total {
            buf.resize(total, BlockStateId::default());
        }
        for (i, section) in self.sections.iter().enumerate() {
            let guard = section.read();
            let base = i * 16;
            guard
                .states
                .copy_column_into(x, z, &mut buf[base..base + 16]);
        }
    }

    #[must_use]
    pub fn read_all_biomes(&self) -> Box<[u16]> {
        let total = self.sections.len() * 64;
        let mut biomes = vec![0u16; total];
        for (i, section) in self.sections.iter().enumerate() {
            let guard = section.read();
            let base = i * 64;
            for qy in 0..4 {
                for qz in 0..4 {
                    for qx in 0..4 {
                        biomes[base + qy * 16 + qz * 4 + qx] = guard.biomes.get(qx, qy, qz);
                    }
                }
            }
        }
        biomes.into_boxed_slice()
    }

    pub fn for_each_biome_id(&self, mut visitor: impl FnMut(u16)) {
        for section in &self.sections {
            let guard = section.read();
            for qy in 0..4 {
                for qz in 0..4 {
                    for qx in 0..4 {
                        visitor(guard.biomes.get(qx, qy, qz));
                    }
                }
            }
        }
    }

    #[must_use]
    pub fn section_emptiness_map(&self) -> Box<[bool]> {
        self.sections
            .iter()
            .map(|section| section.read().is_empty())
            .collect()
    }

    #[must_use]
    pub fn block_light_sources(&self, chunk_pos: ChunkPos, min_y: i32) -> Vec<BlockPos> {
        let mut sources = Vec::new();
        let chunk_min_x = chunk_pos.0.x * BlockPalette::SIZE as i32;
        let chunk_min_z = chunk_pos.0.y * BlockPalette::SIZE as i32;

        for (section_index, section) in self.sections.iter().enumerate() {
            let section_min_y = min_y + (section_index * BlockPalette::SIZE) as i32;
            section.read().append_block_light_sources(
                chunk_min_x,
                section_min_y,
                chunk_min_z,
                &mut sources,
            );
        }

        sources
    }

    pub fn write_column_blocks(&self, x: usize, z: usize, blocks: &[(usize, BlockStateId)]) {
        const DIM: usize = BlockPalette::SIZE;
        debug_assert!(x < DIM);
        debug_assert!(z < DIM);

        let mut i = 0;
        while i < blocks.len() {
            let section_idx = blocks[i].0 / DIM;
            let mut guard = self.sections[section_idx].write();
            guard.states.enter_building_mode();
            let Some(cube) = guard.states.as_building_slice_mut() else {
                unreachable!("just entered building mode")
            };
            let xz_base = z * DIM + x;
            while i < blocks.len() && blocks[i].0 / DIM == section_idx {
                let (rel_y, value) = blocks[i];
                let local_y = rel_y % DIM;
                cube[local_y * DIM * DIM + xz_base] = value;
                i += 1;
            }
        }
    }

    pub fn write_block_batch(&self, blocks: &[(usize, usize, usize, BlockStateId)]) {
        const DIM: usize = BlockPalette::SIZE;
        let mut i = 0;
        while i < blocks.len() {
            let section_idx = blocks[i].1 / DIM;
            let mut guard = self.sections[section_idx].write();
            guard.states.enter_building_mode();
            let Some(cube) = guard.states.as_building_slice_mut() else {
                unreachable!("just entered building mode")
            };
            while i < blocks.len() && blocks[i].1 / DIM == section_idx {
                let (x, rel_y, z, value) = blocks[i];
                let local_y = rel_y % DIM;
                cube[local_y * DIM * DIM + z * DIM + x] = value;
                i += 1;
            }
        }
    }

    pub fn set_relative_block(
        &self,
        relative_x: usize,
        relative_y: usize,
        relative_z: usize,
        value: BlockStateId,
    ) {
        debug_assert!(relative_x < BlockPalette::SIZE);
        debug_assert!(relative_z < BlockPalette::SIZE);

        let idx = relative_y / BlockPalette::SIZE;
        let relative_y = relative_y % BlockPalette::SIZE;
        let mut guard = self.sections[idx].write();
        guard.set_block_state(relative_x, relative_y, relative_z, value);
    }

    pub(crate) fn set_relative_block_for_generation(
        &self,
        relative_x: usize,
        relative_y: usize,
        relative_z: usize,
        value: BlockStateId,
    ) {
        debug_assert!(relative_x < BlockPalette::SIZE);
        debug_assert!(relative_z < BlockPalette::SIZE);

        let idx = relative_y / BlockPalette::SIZE;
        let relative_y = relative_y % BlockPalette::SIZE;
        let mut guard = self.sections[idx].write();
        guard.set_block_state_for_generation(relative_x, relative_y, relative_z, value);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BlockStateSectionCounts {
    is_air: bool,
    has_fluid: bool,
}

#[derive(Debug)]
pub struct ChunkSection {
    pub states: BlockPalette,
    pub biomes: BiomePalette,
    non_empty_block_count: u16,
    fluid_count: u16,
}

impl ChunkSection {
    #[must_use]
    pub const fn new_with_biomes(states: BlockPalette, biomes: BiomePalette) -> Self {
        Self {
            states,
            biomes,
            non_empty_block_count: 0,
            fluid_count: 0,
        }
    }

    #[must_use]
    pub fn new_empty() -> Self {
        let plains_id = plains_biome_id();
        Self {
            states: BlockPalette::Homogeneous(BlockStateId(0)),
            biomes: BiomePalette::Homogeneous(plains_id),
            non_empty_block_count: 0,
            fluid_count: 0,
        }
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.non_empty_block_count == 0
    }

    #[must_use]
    pub fn maybe_has_block_light_sources(&self) -> bool {
        !self.is_empty()
            && self
                .states
                .maybe_has(|state| state.get_light_emission() > 0)
    }

    pub fn append_block_light_sources(
        &self,
        chunk_min_x: i32,
        section_min_y: i32,
        chunk_min_z: i32,
        sources: &mut Vec<BlockPos>,
    ) {
        if !self.maybe_has_block_light_sources() {
            return;
        }

        for local_index in 0..BlockPalette::VOLUME {
            let state = self.states.get_at_index(local_index);
            if state.get_light_emission() == 0 {
                continue;
            }

            sources.push(BlockPos::new(
                chunk_min_x + (local_index & 15) as i32,
                section_min_y + (local_index >> 8) as i32,
                chunk_min_z + ((local_index >> 4) & 15) as i32,
            ));
        }
    }

    #[must_use]
    pub const fn non_empty_block_count(&self) -> u16 {
        self.non_empty_block_count
    }

    #[must_use]
    pub const fn fluid_count(&self) -> u16 {
        self.fluid_count
    }

    #[must_use]
    pub const fn has_fluid(&self) -> bool {
        self.fluid_count > 0
    }

    pub fn recalculate_counts(&mut self) {
        self.recalculate_counts_from_palette(Self::block_state_section_counts);
    }

    fn recalculate_counts_from_palette(
        &mut self,
        mut counts_for_state: impl FnMut(BlockStateId) -> BlockStateSectionCounts,
    ) {
        self.states.finalize_building();

        let mut non_empty: u16 = 0;
        let mut fluid: u16 = 0;

        match &self.states {
            BlockPalette::Homogeneous(state) => {
                let counts = counts_for_state(*state);
                Self::accumulate_counter_traits(
                    &mut non_empty,
                    &mut fluid,
                    counts,
                    BLOCKS_PER_SECTION,
                );
            }
            BlockPalette::Heterogeneous(data) => {
                for &(state, count) in &data.palette {
                    let counts = counts_for_state(state);
                    Self::accumulate_counter_traits(&mut non_empty, &mut fluid, counts, count);
                }
            }
            BlockPalette::Building(_) => unreachable!("finalize_building was just called"),
        }

        self.non_empty_block_count = non_empty;
        self.fluid_count = fluid;
    }

    const fn accumulate_counter_traits(
        non_empty: &mut u16,
        fluid: &mut u16,
        counts: BlockStateSectionCounts,
        block_count: u16,
    ) {
        if !counts.is_air {
            *non_empty += block_count;
        }
        if counts.has_fluid {
            *fluid += block_count;
        }
    }

    pub fn set_block_state(
        &mut self,
        x: usize,
        y: usize,
        z: usize,
        new_state: BlockStateId,
    ) -> BlockStateId {
        self.ensure_counter_ready_for_delta();
        let old_state = self.states.set(x, y, z, new_state);

        if old_state != new_state {
            let old_counts = Self::block_state_section_counts(old_state);
            let new_counts = Self::block_state_section_counts(new_state);
            self.apply_count_change(old_counts, new_counts);
        }

        old_state
    }

    pub(crate) fn set_block_state_for_generation(
        &mut self,
        x: usize,
        y: usize,
        z: usize,
        new_state: BlockStateId,
    ) -> BlockStateId {
        self.states.enter_building_mode();
        self.states.set(x, y, z, new_state)
    }

    pub(crate) fn block_state_section_counts(state: BlockStateId) -> BlockStateSectionCounts {
        BlockStateSectionCounts {
            is_air: state.is_air(),
            has_fluid: state.has_fluid(),
        }
    }

    fn ensure_counter_ready_for_delta(&mut self) {
        if matches!(&self.states, BlockPalette::Building(_)) {
            tracing::debug!(
                "finalizing worldgen Building palette before applying a counter-aware block-state delta"
            );
            self.recalculate_counts();
        }
    }

    const fn apply_count_change(
        &mut self,
        old_counts: BlockStateSectionCounts,
        new_counts: BlockStateSectionCounts,
    ) {
        if !old_counts.is_air && new_counts.is_air {
            self.non_empty_block_count -= 1;
        } else if old_counts.is_air && !new_counts.is_air {
            self.non_empty_block_count += 1;
        }

        if old_counts.has_fluid && !new_counts.has_fluid {
            self.fluid_count -= 1;
        } else if !old_counts.has_fluid && new_counts.has_fluid {
            self.fluid_count += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::blocks::vanilla_blocks;
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

    fn plains_biomes() -> BiomePalette {
        BiomePalette::Homogeneous(plains_biome_id())
    }

    #[test]
    fn recount_uses_homogeneous_palette_frequency() {
        init_test_registry();

        let mut section = ChunkSection::new_with_biomes(
            BlockPalette::Homogeneous(vanilla_blocks::LAVA.default_state()),
            plains_biomes(),
        );

        section.recalculate_counts();

        assert_eq!(section.non_empty_block_count(), BLOCKS_PER_SECTION);
        assert_eq!(section.fluid_count(), BLOCKS_PER_SECTION);
    }

    #[test]
    fn recount_uses_heterogeneous_palette_frequencies() {
        init_test_registry();

        let air = vanilla_blocks::AIR.default_state();
        let stone = vanilla_blocks::STONE.default_state();
        let water = vanilla_blocks::WATER.default_state();
        let lava = vanilla_blocks::LAVA.default_state();
        let mut cube = Box::new([[[air; 16]; 16]; 16]);

        cube[0][0][0] = stone;
        cube[1][0][0] = stone;
        cube[2][0][0] = water;
        cube[3][0][0] = water;
        cube[4][0][0] = water;
        cube[5][0][0] = lava;

        let mut section =
            ChunkSection::new_with_biomes(BlockPalette::from_cube(cube), plains_biomes());

        section.recalculate_counts();

        assert_eq!(section.non_empty_block_count(), 6);
        assert_eq!(section.fluid_count(), 4);
    }

    #[test]
    fn counter_aware_write_recounts_building_palette_before_delta() {
        init_test_registry();

        let air = vanilla_blocks::AIR.default_state();
        let stone = vanilla_blocks::STONE.default_state();
        let mut section = ChunkSection::new_empty();

        section.set_block_state_for_generation(0, 0, 0, stone);
        assert_eq!(section.non_empty_block_count(), 0);

        let old_state = section.set_block_state(0, 0, 0, air);

        assert_eq!(old_state, stone);
        assert_eq!(section.non_empty_block_count(), 0);

        let old_state = section.set_block_state(0, 0, 0, stone);

        assert_eq!(old_state, air);
        assert_eq!(section.non_empty_block_count(), 1);
    }
}
