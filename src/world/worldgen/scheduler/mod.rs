use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::OnceLock;

use parking_lot::{Mutex, RwLock};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::util::pos::ChunkPos;
use crate::world::chunk::proto::Chunk;
use crate::world::chunk::section::{Sections, new_section};
use crate::world::chunk::status::ChunkStatus;
use crate::world::worldgen::generator::Generator;
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::registry::registries;

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
            ChunkStatus::Terrain => {
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
                let biome_neighbor = |chunk_pos: ChunkPos| -> &Chunk {
                    neighbors
                        .iter()
                        .find(|neighbor| neighbor.pos() == chunk_pos)
                        .unwrap_or_else(|| {
                            panic!("neighbor {chunk_pos:?} not allocated when sampling biomes for {pos:?}")
                        })
                };
                let biomes = &registries::BIOME;
                let noise_biome = |quart_x: i32, quart_y: i32, quart_z: i32| {
                    let neighbor = biome_neighbor(ChunkPos::new(quart_x >> 2, quart_z >> 2));
                    let quart_min_y = neighbor.min_y() >> 2;
                    let quart_max_y = quart_min_y + (neighbor.height() >> 2) - 1;
                    let clamped_quart_y = quart_y.clamp(quart_min_y, quart_max_y);
                    let local_quart_y = (clamped_quart_y - quart_min_y) as usize;
                    let id = neighbor.sections().sections[local_quart_y / 4]
                        .read()
                        .get_biome(
                            (quart_x & 3) as usize,
                            local_quart_y % 4,
                            (quart_z & 3) as usize,
                        );
                    biomes
                        .by_id(id)
                        .unwrap_or_else(|| panic!("unknown biome id {id}"))
                };
                let mut possible_biomes = FxHashSet::default();
                for dz in -1..=1 {
                    for dx in -1..=1 {
                        biome_neighbor(ChunkPos::new(pos.0.x + dx, pos.0.y + dz))
                            .sections()
                            .for_each_biome_palette_entry(|id| {
                                possible_biomes.insert(id);
                            });
                    }
                }
                self.generator
                    .build_terrain(chunk, &beardifier, &noise_biome, &possible_biomes);
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
                    self.generator
                        .generation_context(center.min_y(), center.height()),
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
        let section_count = (self.generator.height() / 16) as usize;
        let sections = (0..section_count)
            .map(|_| new_section())
            .collect::<Vec<_>>()
            .into_boxed_slice();

        Chunk::new(
            Sections::from_owned(sections),
            pos,
            self.generator.min_y(),
            self.generator.height(),
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
    use crate::datapack;
    use crate::world::worldgen;
    use shulkr::util::Axis;
    use shulkr::util::Key;
    use shulkr::world::block::{Block, Properties};
    use std::thread;

    use crate::world::worldgen::dimension::{DimensionSpec, resolve};

    fn spec(source: &Datapack, dimension: &str) -> DimensionSpec {
        let id = Key::new(Key::VANILLA_NAMESPACE, dimension.to_owned());
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
    use crate::datapack::Datapack;
    use shulkr::util::BlockPosition;

    fn init_global_registry() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            worldgen::ensure_registry_initialized(&datapack::Datapack::builtin())
                .expect("vanilla datapack should load");
        });
    }

    #[test]
    fn generates_overworld_chunk_to_features_without_panicking() {
        init_global_registry();

        let source = Datapack::builtin();
        let generator = Generator::new(&source, spec(&source, "overworld"), 42)
            .expect("generator should build");
        let store = ChunkStore::new(generator);

        let pos = ChunkPos::new(0, 0);
        store.generate_to(&[pos], ChunkStatus::Features);

        let slot = store
            .get(pos)
            .expect("chunk (0,0) should exist after generate_to");
        assert_eq!(slot.status(), Some(ChunkStatus::Features));

        let chunk = slot.chunk().expect("chunk allocated");
        let coal = *Block::from_key(Key::const_vanilla("coal_ore")).expect("coal_ore block");
        let coal_state = coal.default_state();
        let placed = (chunk.min_y()..chunk.min_y() + chunk.height())
            .flat_map(|y| {
                (0..16).flat_map(move |x| (0..16).map(move |z| BlockPosition::new(x, y, z)))
            })
            .filter(|pos| chunk.get_block_state(*pos) == coal_state)
            .count();
        assert!(placed > 0, "features should have placed coal ore");
    }

    #[test]
    fn concurrent_features_generation_of_adjacent_chunks_does_not_race() {
        init_global_registry();

        let source = Datapack::builtin();
        let generator = Generator::new(&source, spec(&source, "overworld"), 42)
            .expect("generator should build");
        let store = Arc::new(ChunkStore::new(generator));

        let positions: Vec<ChunkPos> = (0..6)
            .flat_map(|dx| (0..6).map(move |dz| ChunkPos::new(1500 + dx, -1500 + dz)))
            .collect();

        let handles: Vec<_> = positions
            .into_iter()
            .map(|pos| {
                let store = Arc::clone(&store);
                thread::spawn(move || {
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
    fn generates_overworld_chunk_to_terrain_without_panicking() {
        init_global_registry();

        let source = Datapack::builtin();
        let generator = Generator::new(&source, spec(&source, "overworld"), 42)
            .expect("generator should build");
        let store = ChunkStore::new(generator);

        let pos = ChunkPos::new(0, 0);
        store.generate_to(&[pos], ChunkStatus::Terrain);

        let slot = store
            .get(pos)
            .expect("chunk (0,0) should exist after generate_to");
        assert_eq!(slot.status(), Some(ChunkStatus::Terrain));

        let neighbor_slot = store
            .get(ChunkPos::new(4, 0))
            .expect("structure-starts neighbor should be allocated at radius 8");
        assert_eq!(neighbor_slot.status(), Some(ChunkStatus::StructureStarts));
    }

    #[test]
    fn dark_forest_places_huge_mushrooms_and_glow_lichen() {
        init_global_registry();

        let source = Datapack::builtin();
        let generator = Generator::new(&source, spec(&source, "overworld"), 42)
            .expect("generator should build");
        let store = ChunkStore::new(generator);

        let positions: Vec<ChunkPos> = (-2..10)
            .flat_map(|dx| (-8..4).map(move |dz| ChunkPos::new(-18 + dx, -11 + dz)))
            .collect();
        store.generate_to(&positions, ChunkStatus::Features);

        let mushroom_stem =
            *Block::from_key(Key::const_vanilla("mushroom_stem")).expect("mushroom_stem block");
        let glow_lichen =
            *Block::from_key(Key::const_vanilla("glow_lichen")).expect("glow_lichen block");
        let oak_log = *Block::from_key(Key::const_vanilla("oak_log")).expect("oak_log block");

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
                        let block = state.as_block();
                        if block == mushroom_stem {
                            stem_count += 1;
                        } else if block == glow_lichen {
                            lichen_count += 1;
                        } else if block == oak_log && state.get(Properties::AXIS) != Some(Axis::Y) {
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
    use crate::datapack;
    use crate::world::worldgen;
    use crate::world::worldgen::dimension::{DimensionSpec, resolve};
    use crate::world::worldgen::structure::placement::structure_placement;
    use shulkr::util::Key;

    fn spec(source: &Datapack, dimension: &str) -> DimensionSpec {
        let id = Key::new(Key::VANILLA_NAMESPACE, dimension.to_owned());
        resolve(source, &id).expect("dimension should resolve")
    }

    use std::sync::Once;

    use super::*;
    use crate::datapack::Datapack;
    use crate::world::worldgen::densityfunction::context::SamplerContext;
    use shulkr::util::BlockPosition;

    fn init_global_registry() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            worldgen::ensure_registry_initialized(&datapack::Datapack::builtin())
                .expect("vanilla datapack should load");
        });
    }

    fn overworld_store(seed: u64) -> ChunkStore {
        let source = Datapack::builtin();
        let generator = Generator::new(&source, spec(&source, "overworld"), seed)
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
                    if state
                        .properties()
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
                        && state
                            .properties()
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
        use crate::world::chunk::heightmap::HeightmapType;

        init_global_registry();
        let store = overworld_store(42);

        let pos = ChunkPos::new(-122, 19);
        store.generate_to(&[pos], ChunkStatus::Terrain);
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
            .get(&Key::of("minecraft:village_plains".to_owned()))
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
        use crate::world::worldgen::structure;

        init_global_registry();
        let store = overworld_store(42);
        let set = structure::registry()
            .sets()
            .iter()
            .find(|set| set.key.to_string() == "minecraft:villages")
            .copied()
            .expect("villages set");

        let structure_placement::StructurePlacement::RandomSpread(placement) = &set.placement
        else {
            panic!("villages use random_spread");
        };
        let mut candidates = Vec::new();
        for x in -40..40 {
            for z in -40..40 {
                if placement.is_placement_chunk(store.generator().seed(), x, z) {
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
            .get(&Key::of("minecraft:village_plains".to_owned()))
            .expect("village_plains start at (-122, 19)");

        assert!(start.is_valid());
        assert!(
            start.pieces().len() > 1,
            "village should expand past its town centre, got {} pieces",
            start.pieces().len()
        );
        assert!(start.bounding_box().is_inside(BlockPosition::new(
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
        use crate::world::worldgen::structure;

        init_global_registry();
        let source = Datapack::builtin();
        let generator = Generator::new(&source, spec(&source, "overworld"), 42)
            .expect("generator should build");

        let any_biome = |_: BiomeRef| true;
        for (key, pos) in [
            ("minecraft:mansion", ChunkPos::new(-18, -11)),
            ("minecraft:stronghold", ChunkPos::new(4, 4)),
            ("minecraft:monument", ChunkPos::new(-82, 136)),
            ("minecraft:fortress", ChunkPos::new(0, 0)),
        ] {
            let id = Key::of(key.to_owned());
            let Some(target) = structure::registry().structure(&id) else {
                continue;
            };
            if matches!(target, structure::structure::Structure::Unported(_)) {
                continue;
            }
            let climate_sampler = generator
                .random_state()
                .create_climate_sampler(SamplerContext::cached());
            let start = target.generate(&generator, 42, pos, 0, &any_biome, &climate_sampler);
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
                    let Some(block) = Some(state.as_block()) else {
                        continue;
                    };
                    match block.key().path() {
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
