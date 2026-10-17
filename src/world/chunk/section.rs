use std::sync::OnceLock;

use parking_lot::RwLock;
use shulkr::util::Key;
use shulkr::world::block::{Block, BlockState};
use shulkr::world::chunk::{SECTION_SIZE, SECTION_VOLUME};
use shulkr::world::palette::Palette;

pub use shulkr::world::chunk::ChunkSection;

use shulkr::registry::registries;

use shulkr::util::BlockPosition;

pub fn state_from_id(state_id: u16) -> BlockState {
    BlockState::from_id(state_id).expect("stored state ids come from registered block states")
}

#[inline]
const fn block_index(x: usize, y: usize, z: usize) -> usize {
    (y * SECTION_SIZE + z) * SECTION_SIZE + x
}

fn plains_biome_id() -> u16 {
    static PLAINS_ID: OnceLock<u16> = OnceLock::new();
    *PLAINS_ID.get_or_init(|| {
        registries::BIOME
            .by_key(&Key::const_vanilla("plains"))
            .expect("vanilla biome registry should contain plains")
            .id()
    })
}

pub fn new_section() -> ChunkSection {
    let mut section = ChunkSection::new();
    let mut biomes = Palette::biomes();
    biomes.fill(plains_biome_id());
    section.set_biomes(biomes);
    section
}

#[derive(Debug)]
pub struct Sections {
    pub sections: Box<[RwLock<ChunkSection>]>,
}

const fn split_by_section(relative_y: usize) -> (usize, usize) {
    (relative_y / SECTION_SIZE, relative_y % SECTION_SIZE)
}

impl Sections {
    pub fn from_owned(sections: Box<[ChunkSection]>) -> Self {
        let sections = sections.into_vec().into_iter().map(RwLock::new).collect();
        Self { sections }
    }

    pub fn get_relative_block(
        &self,
        relative_x: usize,
        relative_y: usize,
        relative_z: usize,
    ) -> Option<BlockState> {
        debug_assert!(relative_x < SECTION_SIZE);
        debug_assert!(relative_z < SECTION_SIZE);

        let (section_index, relative_y) = split_by_section(relative_y);
        self.sections.get(section_index).map(|section| {
            state_from_id(section.read().get_block(relative_x, relative_y, relative_z))
        })
    }

    pub fn read_column_into(&self, x: usize, z: usize, buf: &mut Vec<BlockState>) {
        debug_assert!(x < SECTION_SIZE);
        debug_assert!(z < SECTION_SIZE);

        let total = self.sections.len() * SECTION_SIZE;
        if buf.len() != total {
            buf.resize(total, Block::AIR.default_state());
        }
        for (i, section) in self.sections.iter().enumerate() {
            let guard = section.read();
            let base = i * SECTION_SIZE;
            for (y, slot) in buf[base..base + SECTION_SIZE].iter_mut().enumerate() {
                *slot = state_from_id(guard.get_block(x, y, z));
            }
        }
    }

    pub fn get_relative_biome(
        &self,
        relative_x: usize,
        relative_y: usize,
        relative_z: usize,
    ) -> Option<u16> {
        let (section_index, relative_y) = split_by_section(relative_y);
        self.sections.get(section_index).map(|section| {
            section
                .read()
                .get_biome(relative_x / 4, relative_y / 4, relative_z / 4)
        })
    }

    pub fn for_each_biome_palette_entry(&self, mut visitor: impl FnMut(u16)) {
        for section in &self.sections {
            for biome_id in section.read().biomes().distinct() {
                visitor(biome_id);
            }
        }
    }

    pub fn recalculate_counts(&self) {
        for section in &self.sections {
            let mut guard = section.write();
            if guard.counts_are_stale() {
                guard.recalculate_counts();
            }
        }
    }

    fn write_grouped_by_section(
        &self,
        entries: usize,
        section_of: impl Fn(usize) -> usize,
        mut apply: impl FnMut(&mut [u16], usize),
    ) {
        let mut i = 0;
        while i < entries {
            let section_idx = section_of(i);
            let mut guard = self.sections[section_idx].write();
            let states = guard.block_states_mut();
            while i < entries && section_of(i) == section_idx {
                apply(states, i);
                i += 1;
            }
        }
    }

    pub fn write_column_blocks(&self, x: usize, z: usize, blocks: &[(usize, BlockState)]) {
        debug_assert!(x < SECTION_SIZE);
        debug_assert!(z < SECTION_SIZE);

        let xz_base = z * SECTION_SIZE + x;
        self.write_grouped_by_section(
            blocks.len(),
            |i| blocks[i].0 / SECTION_SIZE,
            |states, i| {
                let (rel_y, value) = blocks[i];
                states[(rel_y % SECTION_SIZE) * SECTION_SIZE * SECTION_SIZE + xz_base] =
                    value.state_id();
            },
        );
    }

    pub fn write_block_batch(&self, blocks: &[(usize, usize, usize, BlockState)]) {
        self.write_grouped_by_section(
            blocks.len(),
            |i| blocks[i].1 / SECTION_SIZE,
            |states, i| {
                let (x, rel_y, z, value) = blocks[i];
                states[block_index(x, rel_y % SECTION_SIZE, z)] = value.state_id();
            },
        );
    }

    pub fn set_relative_block(
        &self,
        relative_x: usize,
        relative_y: usize,
        relative_z: usize,
        value: BlockState,
    ) {
        debug_assert!(relative_x < SECTION_SIZE);
        debug_assert!(relative_z < SECTION_SIZE);

        let (idx, relative_y) = split_by_section(relative_y);
        self.sections[idx]
            .write()
            .set_block(relative_x, relative_y, relative_z, value.state_id());
    }
}

pub fn maybe_has(section: &ChunkSection, mut predicate: impl FnMut(BlockState) -> bool) -> bool {
    section
        .distinct_blocks()
        .any(|state_id| predicate(state_from_id(state_id)))
}

pub fn maybe_has_block_light_sources(section: &ChunkSection) -> bool {
    !section.is_empty() && maybe_has(section, |state| state.get_light_emission() > 0)
}

pub fn append_block_light_sources(
    section: &ChunkSection,
    chunk_min_x: i32,
    section_min_y: i32,
    chunk_min_z: i32,
    sources: &mut Vec<BlockPosition>,
) {
    if !maybe_has_block_light_sources(section) {
        return;
    }

    for local_index in 0..SECTION_VOLUME {
        let state = state_from_id(section.get_block_at_index(local_index));
        if state.get_light_emission() == 0 {
            continue;
        }

        sources.push(BlockPosition::new(
            chunk_min_x + (local_index & 15) as i32,
            section_min_y + (local_index >> 8) as i32,
            chunk_min_z + ((local_index >> 4) & 15) as i32,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datapack;
    use crate::world::worldgen;

    fn init_test_registry() {
        use std::sync::Once;
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            worldgen::ensure_registry_initialized(&datapack::Datapack::builtin())
                .expect("vanilla datapack should load");
        });
    }

    fn sections_of(count: usize) -> Sections {
        Sections::from_owned((0..count).map(|_| new_section()).collect())
    }

    #[test]
    fn a_new_section_defaults_to_plains_and_no_stored_states() {
        init_test_registry();

        let section = new_section();

        assert!(section.block_states().is_none());
        assert_eq!(section.get_biome(0, 0, 0), plains_biome_id());
    }

    #[test]
    fn generation_writes_then_recount_produce_the_vanilla_counts() {
        init_test_registry();

        let stone = Block::STONE.default_state();
        let water = Block::WATER.default_state();
        let lava = Block::LAVA.default_state();
        let sections = sections_of(1);

        sections.write_block_batch(&[
            (0, 0, 0, stone),
            (0, 1, 0, stone),
            (0, 2, 0, water),
            (0, 3, 0, water),
            (0, 4, 0, water),
            (0, 5, 0, lava),
        ]);
        sections.recalculate_counts();

        let section = sections.sections[0].read();
        assert_eq!(section.non_empty_block_count(), 6);
        assert_eq!(section.fluid_count(), 4);
    }

    #[test]
    fn column_writes_land_in_the_right_sections() {
        init_test_registry();

        let stone = Block::STONE.default_state();
        let sections = sections_of(2);

        sections.write_column_blocks(3, 5, &[(4, stone), (20, stone)]);

        assert_eq!(sections.get_relative_block(3, 4, 5), Some(stone));
        assert_eq!(sections.get_relative_block(3, 20, 5), Some(stone));
        assert_eq!(
            sections.get_relative_block(3, 6, 5),
            Some(Block::AIR.default_state())
        );
    }

    #[test]
    fn column_reads_match_point_reads() {
        init_test_registry();

        let stone = Block::STONE.default_state();
        let sections = sections_of(2);
        for y in 0..32 {
            sections.set_relative_block(11, y, 2, stone);
        }

        let mut column = Vec::new();
        sections.read_column_into(11, 2, &mut column);

        assert_eq!(column.len(), 32);
        for (y, state) in column.into_iter().enumerate() {
            assert_eq!(Some(state), sections.get_relative_block(11, y, 2));
        }
    }

    #[test]
    fn many_distinct_states_round_trip() {
        init_test_registry();

        let sections = sections_of(1);
        let cell = |i: u16| ((i % 16) as usize, (i / 16) as usize);

        for i in 1..256u16 {
            let (x, y) = cell(i);
            sections.set_relative_block(x, y, 0, state_from_id(i));
        }
        for i in 1..256u16 {
            let (x, y) = cell(i);
            assert_eq!(sections.get_relative_block(x, y, 0), Some(state_from_id(i)));
        }
    }
}
