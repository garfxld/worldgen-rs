use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::OnceLock;

use glam::IVec3;
use parking_lot::{Mutex, RwLock};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::chunk::proto::{Chunk, GenerationChunk};
use crate::chunk::section::{ChunkSection, Sections};
use crate::chunk::status::ChunkStatus;
use crate::util::pos::ChunkPos;
use crate::worldgen::generator::Generator;

pub struct ChunkSlot {
    chunk: OnceLock<Chunk>,
    status: Mutex<Option<ChunkStatus>>,
}

impl ChunkSlot {
    const fn new() -> Self {
        Self {
            chunk: OnceLock::new(),
            status: Mutex::new(None),
        }
    }

    #[must_use]
    pub fn chunk(&self) -> Option<&Chunk> {
        self.chunk.get()
    }

    #[must_use]
    pub fn status(&self) -> Option<ChunkStatus> {
        *self.status.lock()
    }
}

pub struct ChunkStore {
    generator: Generator,
    slots: RwLock<FxHashMap<ChunkPos, Arc<ChunkSlot>>>,
}

impl ChunkStore {
    #[must_use]
    pub fn new(generator: Generator) -> Self {
        Self {
            generator,
            slots: RwLock::new(FxHashMap::default()),
        }
    }

    #[must_use]
    pub const fn generator(&self) -> &Generator {
        &self.generator
    }

    fn slot(&self, pos: ChunkPos) -> Arc<ChunkSlot> {
        if let Some(slot) = self.slots.read().get(&pos) {
            return Arc::clone(slot);
        }
        Arc::clone(
            self.slots
                .write()
                .entry(pos)
                .or_insert_with(|| Arc::new(ChunkSlot::new())),
        )
    }

    pub fn generate_to(&self, positions: &[ChunkPos], target: ChunkStatus) {
        let levels = compute_levels(positions, target);

        for index in 0..=target.get_index() {
            let status =
                ChunkStatus::from_index(index).expect("index within 0..=target.get_index()");
            let Some(needed) = levels.get(&status) else {
                continue;
            };
            let needed: Vec<ChunkPos> = needed.iter().copied().collect();
            needed
                .par_iter()
                .for_each(|&pos| self.advance_one(pos, status));
        }
    }

    #[must_use]
    pub fn get(&self, pos: ChunkPos) -> Option<Arc<ChunkSlot>> {
        self.slots.read().get(&pos).cloned()
    }

    fn advance_one(&self, pos: ChunkPos, target: ChunkStatus) {
        let slot = self.slot(pos);
        let mut status = slot.status.lock();
        if status.is_some_and(|current| current >= target) {
            return;
        }
        self.run_step(pos, target, &slot);
        *status = Some(target);
    }

    #[expect(
        clippy::similar_names,
        reason = "matches vanilla's local_qx/local_qy/local_qz naming"
    )]
    fn run_step(&self, pos: ChunkPos, target: ChunkStatus, slot: &ChunkSlot) {
        match target {
            ChunkStatus::Empty => {
                slot.chunk
                    .set(self.build_empty_chunk(pos))
                    .unwrap_or_else(|_| panic!("chunk {pos:?} generated to Empty twice"));
            }
            ChunkStatus::StructureStarts | ChunkStatus::StructureReferences => {}
            ChunkStatus::Biomes => {
                self.generator.create_biomes(chunk_at(slot, pos));
            }
            ChunkStatus::Noise => {
                let chunk = chunk_at(slot, pos);
                self.generator
                    .fill_from_noise(GenerationChunk::from_chunk(chunk), None);
            }
            ChunkStatus::Surface => {
                let chunk = chunk_at(slot, pos);
                let neighbor_biomes = |q: IVec3| -> u16 {
                    let neighbor_pos = ChunkPos::new(q.x >> 2, q.z >> 2);
                    let neighbor_slot = self.slot(neighbor_pos);
                    let neighbor_chunk = neighbor_slot
                        .chunk()
                        .unwrap_or_else(|| panic!("neighbor {neighbor_pos:?} not allocated when sampling biomes for {pos:?}"));
                    let min_qy = neighbor_chunk.min_y() >> 2;
                    let total_quarts_y = (neighbor_chunk.sections().sections.len() * 4) as i32;
                    let local_qx = (q.x - neighbor_pos.0.x * 4) as usize;
                    let local_qz = (q.z - neighbor_pos.0.y * 4) as usize;
                    let qy_clamped = (q.y - min_qy).clamp(0, total_quarts_y - 1) as usize;
                    let section_idx = qy_clamped / 4;
                    let local_qy = qy_clamped % 4;
                    neighbor_chunk.sections().sections[section_idx]
                        .read()
                        .biomes
                        .get(local_qx, local_qy, local_qz)
                };
                self.generator
                    .build_surface(GenerationChunk::from_chunk(chunk), &neighbor_biomes);
            }
            other => unimplemented!("ChunkStatus::{other:?} not ported yet"),
        }
    }

    fn build_empty_chunk(&self, pos: ChunkPos) -> Chunk {
        let noises = self.generator.noises();
        let section_count = (noises.height() / 16) as usize;
        let sections = (0..section_count)
            .map(|_| ChunkSection::new_empty())
            .collect::<Vec<_>>()
            .into_boxed_slice();

        Chunk::new(
            Sections::from_owned(sections),
            pos,
            noises.min_y(),
            noises.height(),
        )
    }
}

fn chunk_at(slot: &ChunkSlot, pos: ChunkPos) -> &Chunk {
    slot.chunk
        .get()
        .unwrap_or_else(|| panic!("chunk {pos:?} not allocated at Empty before later status ran"))
}

fn compute_levels(
    positions: &[ChunkPos],
    target: ChunkStatus,
) -> BTreeMap<ChunkStatus, FxHashSet<ChunkPos>> {
    let mut levels: BTreeMap<ChunkStatus, FxHashSet<ChunkPos>> = BTreeMap::new();
    levels
        .entry(target)
        .or_default()
        .extend(positions.iter().copied());

    let mut status = target;
    loop {
        let at_status: Vec<ChunkPos> = levels.get(&status).into_iter().flatten().copied().collect();

        if let Some(parent) = status.parent() {
            levels
                .entry(parent)
                .or_default()
                .extend(at_status.iter().copied());
        }
        for &(dep_status, radius) in status.requirements() {
            let entry = levels.entry(dep_status).or_default();
            for &pos in &at_status {
                entry.extend(chebyshev_neighbors(pos, radius));
            }
        }

        match status.parent() {
            Some(parent) => status = parent,
            None => break,
        }
    }

    levels
}

fn chebyshev_neighbors(pos: ChunkPos, radius: usize) -> impl Iterator<Item = ChunkPos> {
    let radius = radius as i32;
    (-radius..=radius)
        .flat_map(move |dx| (-radius..=radius).map(move |dz| (dx, dz)))
        .map(move |(dx, dz)| ChunkPos::new(pos.0.x + dx, pos.0.y + dz))
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Once;

    use super::*;
    use crate::registry::{REGISTRY, Registries};
    use crate::worldgen::backend::ir::loader::DatapackSource;
    use crate::worldgen::generator::BiomeSourceDimension;

    fn datapack_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/datapack/worldgen")
    }

    fn init_global_registry() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let registries = Registries::new_vanilla().expect("vanilla datapack should load");
            let _ = REGISTRY.init(registries);
        });
    }

    const REFERENCE_COLUMNS: [(usize, usize, i32, i32); 9] = [
        (0, 0, 42, 62),
        (0, 8, 58, 62),
        (0, 15, 59, 62),
        (8, 0, 52, 62),
        (8, 8, 44, 62),
        (8, 15, 56, 62),
        (15, 0, 58, 62),
        (15, 8, 50, 62),
        (15, 15, 48, 62),
    ];

    #[test]
    fn generates_overworld_chunk_to_noise_matching_old_tree_seed_42() {
        init_global_registry();

        let source = DatapackSource::new(datapack_root());
        let generator = Generator::new(
            &source,
            "overworld",
            "Overworld",
            BiomeSourceDimension::Overworld,
            42,
        )
        .expect("generator should build");
        let store = ChunkStore::new(generator);

        let pos = ChunkPos::new(0, 0);
        store.generate_to(&[pos], ChunkStatus::Noise);

        let slot = store
            .get(pos)
            .expect("chunk (0,0) should exist after generate_to");
        assert_eq!(slot.status(), Some(ChunkStatus::Noise));
        let chunk = slot.chunk().expect("chunk allocated at Empty");

        let min_y = chunk.min_y();
        let height = chunk.height();

        for &(x, z, expected_stone_top, expected_water_top) in &REFERENCE_COLUMNS {
            let mut stone_top = None;
            let mut water_top = None;
            for relative_y in (0..height as usize).rev() {
                let Some(state) = chunk.get_relative_block(x, relative_y, z) else {
                    continue;
                };
                let Some(block) = REGISTRY.blocks.by_state_id(state) else {
                    continue;
                };
                let name = block.key.path.as_ref();
                if water_top.is_none() && name == "water" {
                    water_top = Some(min_y + relative_y as i32);
                }
                if stone_top.is_none() && name == "stone" {
                    stone_top = Some(min_y + relative_y as i32);
                }
            }
            assert_eq!(
                stone_top,
                Some(expected_stone_top),
                "stone top mismatch at ({x}, {z})"
            );
            assert_eq!(
                water_top,
                Some(expected_water_top),
                "water top mismatch at ({x}, {z})"
            );
        }
    }
}
