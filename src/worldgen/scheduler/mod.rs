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
use crate::worldgen::region::WorldGenRegion;

const FEATURE_REGION_READ_RADIUS: i32 = 8;
const STRUCTURE_REGION_RADIUS: i32 = 8;

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

    pub fn chunk(&self) -> Option<&Chunk> {
        self.chunk.get()
    }

    pub fn status(&self) -> Option<ChunkStatus> {
        *self.status.lock()
    }
}

pub struct ChunkStore {
    generator: Generator,
    slots: RwLock<FxHashMap<ChunkPos, Arc<ChunkSlot>>>,
}

impl ChunkStore {
    pub fn new(generator: Generator) -> Self {
        Self {
            generator,
            slots: RwLock::new(FxHashMap::default()),
        }
    }

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
            let status = ChunkStatus::try_from(index).expect("index within 0..=target.get_index()");
            let Some(needed) = levels.get(&status) else {
                continue;
            };

            if status < ChunkStatus::Features {
                let pending: Vec<ChunkPos> = needed
                    .as_slice()
                    .iter()
                    .copied()
                    .filter(|&pos| {
                        self.get(pos)
                            .is_none_or(|slot| slot.status().is_none_or(|current| current < status))
                    })
                    .collect();
                if pending.len() > 1 {
                    pending
                        .par_iter()
                        .for_each(|&pos| self.advance_one(pos, status));
                    continue;
                }
                for &pos in &pending {
                    self.advance_one(pos, status);
                }
                continue;
            }
            for &pos in needed.as_slice() {
                self.advance_one(pos, status);
            }
        }
    }

    pub fn get(&self, pos: ChunkPos) -> Option<Arc<ChunkSlot>> {
        self.slots.read().get(&pos).cloned()
    }

    fn advance_one(&self, pos: ChunkPos, target: ChunkStatus) {
        if target == ChunkStatus::Features {
            self.advance_one_features(pos);
            return;
        }
        let slot = self.slot(pos);
        let mut status = slot.status.lock();
        if status.is_some_and(|current| current >= target) {
            return;
        }
        self.run_step(pos, target, &slot);
        *status = Some(target);
    }

    fn advance_one_features(&self, pos: ChunkPos) {
        let mut positions: Vec<ChunkPos> = (-1..=1)
            .flat_map(|dz| (-1..=1).map(move |dx| ChunkPos::new(pos.0.x + dx, pos.0.y + dz)))
            .collect();
        positions.sort_by_key(|p| (p.0.x, p.0.y));

        let slots: Vec<Arc<ChunkSlot>> = positions.iter().map(|&p| self.slot(p)).collect();
        let mut guards: Vec<_> = slots.iter().map(|slot| slot.status.lock()).collect();

        let center_index = positions
            .iter()
            .position(|&p| p == pos)
            .expect("pos is included in its own 3x3 neighborhood");
        if guards[center_index].is_some_and(|current| current >= ChunkStatus::Features) {
            return;
        }

        self.run_step(pos, ChunkStatus::Features, &slots[center_index]);
        *guards[center_index] = Some(ChunkStatus::Features);
    }

    fn run_step(&self, pos: ChunkPos, target: ChunkStatus, slot: &ChunkSlot) {
        match target {
            ChunkStatus::Empty => {
                slot.chunk
                    .set(self.build_empty_chunk(pos))
                    .unwrap_or_else(|_| panic!("chunk {pos:?} generated to Empty twice"));
            }
            ChunkStatus::StructureStarts => {
                self.generator.create_structures(chunk_at(slot, pos));
            }
            ChunkStatus::StructureReferences => {
                let center = chunk_at(slot, pos);
                let neighbor_slots: Vec<Arc<ChunkSlot>> = (-STRUCTURE_REGION_RADIUS
                    ..=STRUCTURE_REGION_RADIUS)
                    .flat_map(|dz| {
                        (-STRUCTURE_REGION_RADIUS..=STRUCTURE_REGION_RADIUS).map(move |dx| (dx, dz))
                    })
                    .map(|(dx, dz)| self.slot(ChunkPos::new(pos.0.x + dx, pos.0.y + dz)))
                    .collect();
                let neighbors: Vec<&Chunk> = neighbor_slots
                    .iter()
                    .map(|neighbor| {
                        neighbor
                            .chunk()
                            .unwrap_or_else(|| panic!("neighbor of {pos:?} not allocated"))
                    })
                    .collect();
                self.generator.create_references(center, &neighbors);
            }
            ChunkStatus::Biomes => {
                self.generator.create_biomes(chunk_at(slot, pos));
            }
            ChunkStatus::Noise => {
                let chunk = chunk_at(slot, pos);
                let neighbor_slots: Vec<Arc<ChunkSlot>> = (-STRUCTURE_REGION_RADIUS
                    ..=STRUCTURE_REGION_RADIUS)
                    .flat_map(|dz| {
                        (-STRUCTURE_REGION_RADIUS..=STRUCTURE_REGION_RADIUS).map(move |dx| (dx, dz))
                    })
                    .map(|(dx, dz)| self.slot(ChunkPos::new(pos.0.x + dx, pos.0.y + dz)))
                    .collect();
                let neighbors: Vec<&Chunk> = neighbor_slots
                    .iter()
                    .filter_map(|neighbor| neighbor.chunk())
                    .collect();
                let beardifier = self.generator.beardifier_for_chunk(chunk, &neighbors);
                self.generator
                    .fill_from_noise(GenerationChunk::from_chunk(chunk), Some(&beardifier));
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
            ChunkStatus::Carvers => {
                let chunk = chunk_at(slot, pos);
                self.generator
                    .apply_carvers(GenerationChunk::from_chunk(chunk));
            }
            ChunkStatus::Features => {
                let read_radius = FEATURE_REGION_READ_RADIUS;
                let neighbor_slots: Vec<Arc<ChunkSlot>> = (-read_radius..=read_radius)
                    .flat_map(|dz| (-read_radius..=read_radius).map(move |dx| (dx, dz)))
                    .map(|(dx, dz)| self.slot(ChunkPos::new(pos.0.x + dx, pos.0.y + dz)))
                    .collect();
                let chunks: Vec<&Chunk> = neighbor_slots
                    .iter()
                    .map(|neighbor| {
                        neighbor
                            .chunk()
                            .unwrap_or_else(|| panic!("neighbor of {pos:?} not allocated"))
                    })
                    .collect();

                let center = chunk_at(slot, pos);
                let region = WorldGenRegion::new(
                    pos,
                    read_radius,
                    chunks,
                    center.min_y(),
                    center.height(),
                    self.generator.sea_level(),
                    self.generator.seed(),
                    self.generator.biome_zoom_seed(),
                    self.generator.region_random(pos),
                );
                self.generator.apply_biome_decoration(&region);
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

#[derive(Default)]
struct OrderedChunks {
    order: Vec<ChunkPos>,
    seen: FxHashSet<ChunkPos>,
}

impl OrderedChunks {
    fn insert(&mut self, pos: ChunkPos) {
        if self.seen.insert(pos) {
            self.order.push(pos);
        }
    }

    fn extend(&mut self, positions: impl IntoIterator<Item = ChunkPos>) {
        for pos in positions {
            self.insert(pos);
        }
    }

    fn as_slice(&self) -> &[ChunkPos] {
        &self.order
    }
}

fn compute_levels(
    positions: &[ChunkPos],
    target: ChunkStatus,
) -> BTreeMap<ChunkStatus, OrderedChunks> {
    let mut levels: BTreeMap<ChunkStatus, OrderedChunks> = BTreeMap::new();
    levels
        .entry(target)
        .or_default()
        .extend(positions.iter().copied());

    let mut status = target;
    loop {
        let at_status: Vec<ChunkPos> = levels
            .get(&status)
            .map_or_else(Vec::new, |chunks| chunks.order.clone());

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

pub(crate) fn chebyshev_neighbors(pos: ChunkPos, radius: usize) -> impl Iterator<Item = ChunkPos> {
    let radius = radius as i32;
    (-radius..=radius)
        .flat_map(move |dx| (-radius..=radius).map(move |dz| (dx, dz)))
        .map(move |(dx, dz)| ChunkPos::new(pos.0.x + dx, pos.0.y + dz))
}

#[cfg(test)]
mod tests {
    use crate::worldgen::dimension::{DimensionSpec, resolve};

    fn spec(source: &DatapackSource, dimension: &str) -> DimensionSpec {
        let id = Identifier::new(Identifier::VANILLA_NAMESPACE, dimension.to_owned());
        resolve(source, &id).expect("dimension should resolve")
    }
    #[test]
    fn features_layer_scans_x_major_z_inner_like_vanilla() {
        use super::{ChunkPos, ChunkStatus, chebyshev_neighbors, compute_levels};

        let center = ChunkPos::new(9, 3);
        let requested: Vec<ChunkPos> = chebyshev_neighbors(center, 1).collect();
        let levels = compute_levels(&requested, ChunkStatus::Features);
        let features = levels
            .get(&ChunkStatus::Features)
            .expect("features level present");

        let expected: Vec<ChunkPos> = (-1..=1)
            .flat_map(|dx| (-1..=1).map(move |dz| ChunkPos::new(9 + dx, 3 + dz)))
            .collect();
        assert_eq!(features.as_slice(), expected.as_slice());
    }

    use std::sync::Once;

    use super::*;
    use crate::registry::blocks::properties::{Axis, Properties};
    use crate::registry::{REGISTRY, Registries};
    use crate::util::ident::Identifier;
    use crate::util::pos::BlockPos;
    use crate::worldgen::backend::ir::loader::DatapackSource;

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
    fn base_height_column_matches_filled_chunk_heightmap() {
        use crate::chunk::heightmap::HeightmapType;

        init_global_registry();

        let source = DatapackSource::builtin();
        let generator = Generator::new(&source, &spec(&source, "overworld"), 42)
            .expect("generator should build");

        let pos = ChunkPos::new(0, 0);
        let store = ChunkStore::new(generator);
        store.generate_to(&[pos], ChunkStatus::Noise);
        let slot = store.get(pos).expect("chunk should exist");
        let chunk = slot.chunk().expect("chunk allocated at Empty");

        for (x, z) in [(0usize, 0usize), (3, 11), (8, 8), (15, 15)] {
            let expected = chunk.generation_height_at(HeightmapType::WorldSurfaceWg, x, z);
            let actual = store.generator().get_base_height(
                x as i32,
                z as i32,
                HeightmapType::WorldSurfaceWg,
            );
            assert_eq!(actual, expected, "base height mismatch at ({x}, {z})");
        }
    }

    #[test]
    fn generates_overworld_chunk_to_noise_matching_old_tree_seed_42() {
        init_global_registry();

        let source = DatapackSource::builtin();
        let generator = Generator::new(&source, &spec(&source, "overworld"), 42)
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

    #[test]
    fn generates_overworld_chunk_to_features_without_panicking() {
        init_global_registry();

        let source = DatapackSource::builtin();
        let generator = Generator::new(&source, &spec(&source, "overworld"), 42)
            .expect("generator should build");
        let store = ChunkStore::new(generator);

        let pos = ChunkPos::new(0, 0);
        store.generate_to(&[pos], ChunkStatus::Features);

        let slot = store
            .get(pos)
            .expect("chunk (0,0) should exist after generate_to");
        assert_eq!(slot.status(), Some(ChunkStatus::Features));

        let chunk = slot.chunk().expect("chunk allocated");
        let coal = REGISTRY
            .blocks
            .by_key(&Identifier::const_vanilla("coal_ore"))
            .expect("coal_ore block");
        let coal_state = REGISTRY.blocks.get_default_state_id(coal);
        let placed = (chunk.min_y()..chunk.min_y() + chunk.height())
            .flat_map(|y| (0..16).flat_map(move |x| (0..16).map(move |z| BlockPos::new(x, y, z))))
            .filter(|pos| chunk.get_block_state(*pos) == coal_state)
            .count();
        assert!(placed > 0, "features should have placed coal ore");
    }

    #[test]
    fn concurrent_features_generation_of_adjacent_chunks_does_not_race() {
        init_global_registry();

        let source = DatapackSource::builtin();
        let generator = Generator::new(&source, &spec(&source, "overworld"), 42)
            .expect("generator should build");
        let store = Arc::new(ChunkStore::new(generator));

        let positions: Vec<ChunkPos> = (0..6)
            .flat_map(|dx| (0..6).map(move |dz| ChunkPos::new(1500 + dx, -1500 + dz)))
            .collect();

        let handles: Vec<_> = positions
            .into_iter()
            .map(|pos| {
                let store = Arc::clone(&store);
                std::thread::spawn(move || {
                    store.generate_to(&[pos], ChunkStatus::Features);
                })
            })
            .collect();

        for handle in handles {
            handle
                .join()
                .expect("chunk generation thread should not panic");
        }
    }

    #[test]
    fn generates_overworld_chunk_to_carvers_without_panicking() {
        init_global_registry();

        let source = DatapackSource::builtin();
        let generator = Generator::new(&source, &spec(&source, "overworld"), 42)
            .expect("generator should build");
        let store = ChunkStore::new(generator);

        let pos = ChunkPos::new(0, 0);
        store.generate_to(&[pos], ChunkStatus::Carvers);

        let slot = store
            .get(pos)
            .expect("chunk (0,0) should exist after generate_to");
        assert_eq!(slot.status(), Some(ChunkStatus::Carvers));

        let neighbor_slot = store
            .get(ChunkPos::new(4, 0))
            .expect("structure-starts neighbor should be allocated at radius 8");
        assert_eq!(neighbor_slot.status(), Some(ChunkStatus::StructureStarts));
    }

    #[test]
    fn dark_forest_places_huge_mushrooms_and_glow_lichen() {
        init_global_registry();

        let source = DatapackSource::builtin();
        let generator = Generator::new(&source, &spec(&source, "overworld"), 42)
            .expect("generator should build");
        let store = ChunkStore::new(generator);

        let positions: Vec<ChunkPos> = (-2..10)
            .flat_map(|dx| (-8..4).map(move |dz| ChunkPos::new(-18 + dx, -11 + dz)))
            .collect();
        store.generate_to(&positions, ChunkStatus::Features);

        let mushroom_stem = REGISTRY
            .blocks
            .by_key(&Identifier::const_vanilla("mushroom_stem"))
            .expect("mushroom_stem block");
        let glow_lichen = REGISTRY
            .blocks
            .by_key(&Identifier::const_vanilla("glow_lichen"))
            .expect("glow_lichen block");
        let oak_log = REGISTRY
            .blocks
            .by_key(&Identifier::const_vanilla("oak_log"))
            .expect("oak_log block");

        let mut stem_count = 0;
        let mut lichen_count = 0;
        let mut fallen_log_count = 0;
        for &pos in &positions {
            let slot = store
                .get(pos)
                .expect("chunk should exist after generate_to");
            let chunk = slot.chunk().expect("chunk allocated");
            for relative_y in 0..chunk.height() as usize {
                for x in 0..16 {
                    for z in 0..16 {
                        let Some(state) = chunk.get_relative_block(x, relative_y, z) else {
                            continue;
                        };
                        let block = REGISTRY.blocks.by_state_id(state);
                        if block == Some(mushroom_stem) {
                            stem_count += 1;
                        } else if block == Some(glow_lichen) {
                            lichen_count += 1;
                        } else if block == Some(oak_log)
                            && REGISTRY.blocks.try_get_property(state, &Properties::AXIS)
                                != Some(Axis::Y)
                        {
                            // A horizontal-axis oak_log is the signature of a fallen tree's
                            // trunk segments (standing oak trunks are always axis=y).
                            fallen_log_count += 1;
                        }
                    }
                }
            }
        }

        assert!(
            stem_count > 0,
            "expected at least one huge mushroom stem block"
        );
        assert!(lichen_count > 0, "expected at least one glow lichen block");
        assert!(
            fallen_log_count > 0,
            "expected at least one fallen oak log segment"
        );
    }
}

#[cfg(test)]
mod structure_tests {
    use crate::util::ident::Identifier;
    use crate::worldgen::dimension::{DimensionSpec, resolve};

    fn spec(source: &DatapackSource, dimension: &str) -> DimensionSpec {
        let id = Identifier::new(Identifier::VANILLA_NAMESPACE, dimension.to_owned());
        resolve(source, &id).expect("dimension should resolve")
    }

    use std::sync::Once;

    use super::*;
    use crate::registry::{REGISTRY, Registries};
    use crate::util::pos::BlockPos;
    use crate::worldgen::backend::ir::loader::DatapackSource;

    fn init_global_registry() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let registries = Registries::new_vanilla().expect("vanilla datapack should load");
            let _ = REGISTRY.init(registries);
        });
    }

    fn overworld_store(seed: u64) -> ChunkStore {
        let source = DatapackSource::builtin();
        let generator = Generator::new(&source, &spec(&source, "overworld"), seed)
            .expect("generator should build");
        ChunkStore::new(generator)
    }

    #[test]
    fn structure_pieces_on_dry_land_are_not_waterlogged() {
        init_global_registry();
        let store = overworld_store(42);

        let pos = ChunkPos::new(-122, 19);
        store.generate_to(&[pos], ChunkStatus::Features);
        let slot = store.get(pos).expect("slot");
        let chunk = slot.chunk().expect("chunk");

        let mut waterlogged = 0;
        for relative_y in 0..chunk.height() as usize {
            for x in 0..16 {
                for z in 0..16 {
                    let Some(state) = chunk.get_relative_block(x, relative_y, z) else {
                        continue;
                    };
                    let y = chunk.min_y() + relative_y as i32;
                    if y < 60 {
                        continue;
                    }
                    if REGISTRY
                        .blocks
                        .get_properties(state)
                        .iter()
                        .any(|(property, value)| *property == "waterlogged" && *value == "true")
                    {
                        waterlogged += 1;
                    }
                }
            }
        }

        let mut waterloggable = 0;
        for relative_y in 0..chunk.height() as usize {
            for x in 0..16 {
                for z in 0..16 {
                    let Some(state) = chunk.get_relative_block(x, relative_y, z) else {
                        continue;
                    };
                    if chunk.min_y() + relative_y as i32 >= 60
                        && REGISTRY
                            .blocks
                            .get_properties(state)
                            .iter()
                            .any(|(property, _)| *property == "waterlogged")
                    {
                        waterloggable += 1;
                    }
                }
            }
        }

        assert!(
            waterloggable > 10,
            "test needs waterloggable village blocks to be meaningful, found {waterloggable}"
        );
        assert_eq!(
            waterlogged, 0,
            "village blocks above sea level must not be waterlogged"
        );
    }

    #[test]
    fn beardifier_raises_terrain_under_a_village_piece() {
        use crate::chunk::heightmap::HeightmapType;

        init_global_registry();
        let store = overworld_store(42);

        let pos = ChunkPos::new(-122, 19);
        store.generate_to(&[pos], ChunkStatus::Noise);
        let slot = store.get(pos).expect("slot");
        let chunk = slot.chunk().expect("chunk");

        let neighbor_slots: Vec<Arc<ChunkSlot>> = (-8..=8)
            .flat_map(|dz| (-8..=8).map(move |dx| (dx, dz)))
            .map(|(dx, dz)| store.slot(ChunkPos::new(pos.0.x + dx, pos.0.y + dz)))
            .collect();
        let neighbors: Vec<&Chunk> = neighbor_slots
            .iter()
            .filter_map(|neighbor| neighbor.chunk())
            .collect();
        let beardifier = store.generator().beardifier_for_chunk(chunk, &neighbors);
        assert!(
            !beardifier.is_empty(),
            "village chunk should have beard-affected structure pieces"
        );

        let starts = chunk.structure_starts();
        let start = starts
            .get(&"minecraft:village_plains".parse().expect("valid id"))
            .expect("village start");
        let piece_box = start.pieces()[0].bounding_box();
        let center = piece_box.center();
        assert_ne!(
            beardifier.compute(center.x(), piece_box.min_y(), center.z()),
            0.0,
            "beard density must be non-zero inside a rigid village piece"
        );

        let local_x = (center.x() - pos.0.x * 16).clamp(0, 15) as usize;
        let local_z = (center.z() - pos.0.y * 16).clamp(0, 15) as usize;
        let surface = chunk.generation_height_at(HeightmapType::WorldSurfaceWg, local_x, local_z);
        assert!(
            (surface - piece_box.min_y()).abs() <= 6,
            "terrain under the village should sit near the piece floor: surface {surface}, piece {}",
            piece_box.min_y()
        );
    }

    #[test]
    fn random_spread_placement_selects_one_chunk_per_grid_cell() {
        use crate::worldgen::structure;

        init_global_registry();
        let store = overworld_store(42);
        let set = structure::registry()
            .sets()
            .iter()
            .find(|set| set.key.to_string() == "minecraft:villages")
            .copied()
            .expect("villages set");

        let mut candidates = Vec::new();
        for x in -40..40 {
            for z in -40..40 {
                if set
                    .placement
                    .is_placement_chunk(store.generator().seed(), x, z, None)
                {
                    candidates.push(ChunkPos::new(x, z));
                }
            }
        }

        assert_eq!(
            candidates,
            vec![
                ChunkPos::new(-29, -21),
                ChunkPos::new(-19, 10),
                ChunkPos::new(9, 11),
                ChunkPos::new(10, -27),
                ChunkPos::new(36, 25),
            ]
        );
    }

    #[test]
    fn jigsaw_start_assembles_a_plains_village() {
        init_global_registry();
        let store = overworld_store(42);

        let pos = ChunkPos::new(-122, 19);
        store.generate_to(&[pos], ChunkStatus::StructureStarts);
        let slot = store.get(pos).expect("slot");
        let chunk = slot.chunk().expect("chunk");
        let starts = chunk.structure_starts();
        let start = starts
            .get(&"minecraft:village_plains".parse().expect("valid id"))
            .expect("village_plains start at (-122, 19)");

        assert!(start.is_valid());
        assert!(
            start.pieces().len() > 1,
            "village should expand past its town centre, got {} pieces",
            start.pieces().len()
        );
        assert!(start.bounding_box().is_inside(BlockPos::new(
            pos.0.x * 16 + 8,
            start.bounding_box().min_y(),
            pos.0.y * 16 + 8
        )));
    }

    #[test]
    fn newly_ported_structures_produce_valid_starts() {
        init_global_registry();
        let store = overworld_store(42);

        let mut positions = Vec::new();
        for x in -40..40 {
            for z in -40..40 {
                positions.push(ChunkPos::new(x, z));
            }
        }
        store.generate_to(&positions, ChunkStatus::StructureStarts);

        let mut seen: rustc_hash::FxHashSet<String> = rustc_hash::FxHashSet::default();
        let mut sources: rustc_hash::FxHashMap<String, ChunkPos> = rustc_hash::FxHashMap::default();
        for pos in &positions {
            let Some(slot) = store.get(*pos) else {
                continue;
            };
            let Some(chunk) = slot.chunk() else {
                continue;
            };
            for (key, start) in chunk.structure_starts().iter() {
                if start.is_valid() {
                    seen.insert(key.to_string());
                    sources.entry(key.to_string()).or_insert(*pos);
                }
            }
        }

        for expected in [
            "minecraft:swamp_hut",
            "minecraft:shipwreck",
            "minecraft:ocean_ruin_cold",
            "minecraft:ruined_portal",
            "minecraft:buried_treasure",
            "minecraft:mineshaft",
        ] {
            assert!(
                seen.contains(expected),
                "expected at least one {expected} start in the scanned area, saw {seen:?}"
            );
        }

        for key in [
            "minecraft:shipwreck",
            "minecraft:ocean_ruin_cold",
            "minecraft:mineshaft",
        ] {
            let pos = sources[key];
            store.generate_to(&[pos], ChunkStatus::Features);
            let slot = store.get(pos).expect("slot");
            assert!(
                slot.chunk().is_some(),
                "{key} chunk at {pos:?} should place"
            );
        }
    }

    #[test]
    fn rare_structures_assemble_when_generated_directly() {
        use crate::registry::biome::BiomeRef;
        use crate::worldgen::structure;

        init_global_registry();
        let source = DatapackSource::builtin();
        let generator = Generator::new(&source, &spec(&source, "overworld"), 42)
            .expect("generator should build");

        let any_biome = |_: BiomeRef| true;
        for (key, pos) in [
            ("minecraft:mansion", ChunkPos::new(-18, -11)),
            ("minecraft:stronghold", ChunkPos::new(4, 4)),
            ("minecraft:monument", ChunkPos::new(-82, 136)),
            ("minecraft:fortress", ChunkPos::new(0, 0)),
        ] {
            let id = key.parse().expect("valid id");
            let Some(target) = structure::registry().structure(&id) else {
                continue;
            };
            if matches!(target, structure::structure::Structure::Unported(_)) {
                continue;
            }
            let start = target.generate(&generator, 42, pos, 0, &any_biome);
            assert!(
                start.is_valid(),
                "{key} should assemble at least one piece at {pos:?}"
            );
        }
    }

    #[test]
    fn structure_pieces_are_written_during_the_features_step() {
        init_global_registry();
        let store = overworld_store(42);

        let pos = ChunkPos::new(-122, 19);
        store.generate_to(&[pos], ChunkStatus::Features);
        let slot = store.get(pos).expect("slot");
        let chunk = slot.chunk().expect("chunk");

        let mut village_blocks = 0;
        let mut jigsaw_blocks = 0;
        for relative_y in 0..chunk.height() as usize {
            for x in 0..16 {
                for z in 0..16 {
                    let Some(state) = chunk.get_relative_block(x, relative_y, z) else {
                        continue;
                    };
                    let Some(block) = REGISTRY.blocks.by_state_id(state) else {
                        continue;
                    };
                    match block.key.path.as_ref() {
                        "jigsaw" => jigsaw_blocks += 1,
                        "oak_planks" | "cobblestone" | "oak_log" | "oak_stairs" | "dirt_path"
                        | "oak_fence" | "cobblestone_stairs" => village_blocks += 1,
                        _ => {}
                    }
                }
            }
        }

        assert_eq!(jigsaw_blocks, 0, "jigsaw blocks must be replaced on place");
        assert!(
            village_blocks > 50,
            "expected village blocks in the chunk, found {village_blocks}"
        );
    }
}
