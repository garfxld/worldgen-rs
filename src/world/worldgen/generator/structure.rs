use std::f64::consts::TAU;

use rayon::iter::{IntoParallelIterator, ParallelIterator};
use shulkr::util::Key;

use crate::util::rng::RandomSource;
use crate::world::chunk::proto::Chunk;
use crate::world::worldgen::region::WorldGenRegion;
use crate::world::worldgen::structure::structure::Structure;
use rustc_hash::FxHashMap;
use shulkr::util::BoundingBox;

use crate::registry::biome::BiomeRef;

use crate::util::pos::ChunkPos;
use crate::util::rng::legacy_random::LegacyRandomSource;
use crate::world::worldgen::beardifier::Beardifier;
use crate::world::worldgen::biome::climate::Sampler;
use crate::world::worldgen::densityfunction::context::SamplerContext;
use crate::world::worldgen::generator::{GENERATION_STEPS, Generator};
use crate::world::worldgen::structure::placement::structure_placement::{
    AbstractSpreadingStructurePlacement, StructurePlacement,
};
use crate::world::worldgen::structure::structure::BiomeHolderSet;
use crate::world::worldgen::structure::structure_set::{StructureSelectionEntry, StructureSet};
use crate::world::worldgen::structure::{self};

impl Generator {
    pub fn possible_structure_sets(&self) -> &[&'static StructureSet] {
        self.possible_structure_sets.get_or_init(|| {
            structure::registry()
                .sets()
                .iter()
                .copied()
                .filter(|set| {
                    set.structures.iter().any(|entry| {
                        entry.structure.biomes().is_some_and(|biomes| {
                            self.possible_biomes
                                .iter()
                                .copied()
                                .any(|biome| biomes.contains(biome))
                        })
                    })
                })
                .collect()
        })
    }

    pub fn structures_by_step(&self) -> &[Vec<(Key, &'static Structure)>] {
        self.structures_by_step.get_or_init(|| {
            let mut sorted: Vec<(Key, &'static Structure)> = structure::registry()
                .structures()
                .map(|(key, value)| (key.clone(), value))
                .collect();
            sorted.sort_by_key(|entry| entry.0.to_string());

            let mut by_step: Vec<Vec<(Key, &'static Structure)>> =
                vec![Vec::new(); GENERATION_STEPS];
            for entry in sorted {
                by_step[entry.1.step() as usize].push(entry);
            }
            by_step
        })
    }

    pub fn place_structures_for_step(
        &self,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        decoration_seed: i64,
        step: usize,
    ) {
        let Some(structures) = self.structures_by_step().get(step) else {
            return;
        };
        if structures.is_empty() {
            return;
        }

        let center = region.center();
        let chunk = region
            .chunk(center)
            .expect("center chunk should be inside the region");
        let writable_area = BoundingBox::new(
            center.0.x * 16,
            region.min_y() + 1,
            center.0.y * 16,
            center.0.x * 16 + 15,
            region.max_y() - 1,
            center.0.y * 16 + 15,
        );

        for (index, (key, _)) in structures.iter().enumerate() {
            random.set_feature_seed(decoration_seed, index as i32, step as i32);

            let sources = chunk
                .structure_references()
                .get(key)
                .cloned()
                .unwrap_or_default();
            for source_key in sources {
                let source_pos = unpack_chunk_pos(source_key);
                let Some(source_chunk) = region.chunk(source_pos) else {
                    continue;
                };
                let mut starts = source_chunk.structure_starts();
                let Some(start) = starts.get_mut(key) else {
                    continue;
                };
                if start.is_valid() {
                    start.place_in_chunk(region, random, writable_area);
                }
            }
        }
    }

    pub fn create_structures(&self, chunk: &Chunk) {
        let source_chunk_pos = chunk.pos();
        let climate_sampler = self
            .random_state
            .create_climate_sampler(SamplerContext::cached());

        for set in self.possible_structure_sets() {
            let already_started = set.structures.iter().any(|entry| {
                chunk
                    .structure_starts()
                    .get(&entry.key)
                    .is_some_and(|start| start.is_valid())
            });
            if already_started {
                continue;
            }

            if !self.is_structure_chunk(set, source_chunk_pos.0.x, source_chunk_pos.0.y) {
                continue;
            }

            if set.structures.len() == 1 {
                self.try_generate_structure(
                    &set.structures[0],
                    chunk,
                    source_chunk_pos,
                    &climate_sampler,
                );
                continue;
            }

            let mut options: Vec<&StructureSelectionEntry> = set.structures.iter().collect();
            let mut random = LegacyRandomSource::from_seed(0);
            random.set_large_feature_seed(self.seed(), source_chunk_pos.0.x, source_chunk_pos.0.y);
            let mut total: i32 = options.iter().map(|entry| entry.weight).sum();

            while !options.is_empty() {
                let mut choice = random.next_i32_bounded(total);
                let mut index = 0;
                for entry in &options {
                    choice -= entry.weight;
                    if choice < 0 {
                        break;
                    }
                    index += 1;
                }
                let index = index.min(options.len() - 1);

                let selected = options[index];
                if self.try_generate_structure(selected, chunk, source_chunk_pos, &climate_sampler)
                {
                    break;
                }

                total -= selected.weight;
                options.remove(index);
            }
        }
    }

    pub fn ring_positions(&self, set: &StructureSet) -> Option<&[ChunkPos]> {
        self.ring_positions
            .get_or_init(|| {
                let mut all = FxHashMap::default();
                for possible in self.possible_structure_sets() {
                    let StructurePlacement::ConcentricRings(rings) = &possible.placement else {
                        continue;
                    };
                    all.insert(
                        possible.key.clone(),
                        self.generate_ring_positions(
                            rings.distance,
                            rings.spread,
                            rings.count,
                            &rings.preferred_biomes,
                        ),
                    );
                }
                all
            })
            .get(&set.key)
            .map(Vec::as_slice)
    }

    fn generate_ring_positions(
        &self,
        distance: i32,
        spread: i32,
        count: i32,
        preferred_biomes: &BiomeHolderSet,
    ) -> Vec<ChunkPos> {
        if count == 0 {
            return Vec::new();
        }

        let mut random = RandomSource::Legacy(LegacyRandomSource::from_seed(0));
        random.set_seed(self.seed());
        let mut angle = random.next_f64() * TAU;
        let mut position_in_circle = 0;
        let mut circle = 0;
        let mut spread = spread;
        let mut searches = Vec::with_capacity(count as usize);

        for index in 0..count {
            let dist = f64::from(4 * distance + distance * circle * 6)
                + (random.next_f64() - 0.5) * (f64::from(distance) * 2.5);
            let initial_x = (angle.cos() * dist).round() as i32;
            let initial_z = (angle.sin() * dist).round() as i32;
            searches.push((initial_x, initial_z, random.fork()));

            angle += TAU / f64::from(spread);
            position_in_circle += 1;
            if position_in_circle == spread {
                circle += 1;
                position_in_circle = 0;
                spread += 2 * spread / (circle + 1);
                spread = spread.min(count - index);
                angle += random.next_f64() * TAU;
            }
        }

        searches
            .into_par_iter()
            .map(|(initial_x, initial_z, mut search_random)| {
                let allowed = |biome: BiomeRef| preferred_biomes.contains(biome);
                let found = self.find_biome_horizontal(
                    initial_x * 16 + 8,
                    0,
                    initial_z * 16 + 8,
                    112,
                    &allowed,
                    &mut search_random,
                );
                match found {
                    Some((pos, _)) => ChunkPos::new(pos.x() >> 4, pos.z() >> 4),
                    None => ChunkPos::new(initial_x, initial_z),
                }
            })
            .collect()
    }

    fn is_structure_chunk(&self, set: &StructureSet, source_x: i32, source_z: i32) -> bool {
        let placement_chunk = match &set.placement {
            StructurePlacement::DimensionOrigin => {
                let origin = self.dimension_origin();
                return origin.0.x == source_x && origin.0.y == source_z;
            }
            StructurePlacement::RandomSpread(placement) => {
                placement.is_placement_chunk(self.seed(), source_x, source_z)
            }
            StructurePlacement::ConcentricRings(_) => {
                self.ring_positions(set).is_some_and(|positions| {
                    positions
                        .iter()
                        .any(|pos| pos.0.x == source_x && pos.0.y == source_z)
                })
            }
        };
        let spreading = set
            .placement
            .spreading()
            .expect("spreading placements carry spreading settings");
        placement_chunk
            && spreading.apply_additional_chunk_restrictions(source_x, source_z, self.seed())
            && self.apply_interactions_with_other_structures(spreading, source_x, source_z)
    }

    fn apply_interactions_with_other_structures(
        &self,
        placement: &AbstractSpreadingStructurePlacement,
        source_x: i32,
        source_z: i32,
    ) -> bool {
        let Some(exclusion_zone) = placement.exclusion_zone.as_ref() else {
            return true;
        };
        let Some(other_set) = structure::registry()
            .sets()
            .iter()
            .find(|set| set.key == exclusion_zone.other_set)
        else {
            return true;
        };

        let range = exclusion_zone.chunk_count;
        for test_x in (source_x - range)..=(source_x + range) {
            for test_z in (source_z - range)..=(source_z + range) {
                if self.is_structure_chunk(other_set, test_x, test_z) {
                    return false;
                }
            }
        }
        true
    }

    fn try_generate_structure(
        &self,
        entry: &StructureSelectionEntry,
        chunk: &Chunk,
        source_chunk_pos: ChunkPos,
        climate_sampler: &Sampler,
    ) -> bool {
        let references = chunk
            .structure_starts()
            .get(&entry.key)
            .map_or(0, |start| start.references());

        let biomes = entry.structure.biomes();
        let valid_biome = |biome: BiomeRef| biomes.is_some_and(|allowed| allowed.contains(biome));
        let start = entry.structure.generate(
            self,
            self.seed(),
            source_chunk_pos,
            references,
            &valid_biome,
            climate_sampler,
        );

        if start.is_valid() {
            chunk.structure_starts().insert(entry.key.clone(), start);
            true
        } else {
            false
        }
    }

    pub fn beardifier_for_chunk(&self, center: &Chunk, neighbors: &[&Chunk]) -> Beardifier {
        let references = center.structure_references().clone();
        if references.is_empty() {
            return Beardifier::empty();
        }

        Beardifier::for_structures_in_chunk(center.pos(), |visit| {
            for (key, sources) in &references {
                for source_key in sources {
                    let source_pos = unpack_chunk_pos(*source_key);
                    let Some(source_chunk) =
                        neighbors.iter().find(|chunk| chunk.pos() == source_pos)
                    else {
                        continue;
                    };
                    let starts = source_chunk.structure_starts();
                    if let Some(start) = starts.get(key)
                        && start.is_valid()
                    {
                        visit(start);
                    }
                }
            }
        })
    }

    pub fn create_references(&self, center: &Chunk, neighbors: &[&Chunk]) {
        let chunk_pos = center.pos();
        let target_block_x = chunk_pos.0.x * 16;
        let target_block_z = chunk_pos.0.y * 16;

        for source in neighbors {
            let source_pos = source.pos();
            let source_key = pack_chunk_pos(source_pos);
            for (key, start) in source.structure_starts().iter() {
                if !start.is_valid() {
                    continue;
                }
                if start.bounding_box().intersects_xz(
                    target_block_x,
                    target_block_z,
                    target_block_x + 15,
                    target_block_z + 15,
                ) {
                    add_reference(center, key, source_key);
                }
            }
        }
    }
}

fn add_reference(chunk: &Chunk, structure: &Key, source: i64) {
    let mut references = chunk.structure_references();
    let entry = references.entry(structure.clone()).or_default();
    if !entry.contains(&source) {
        entry.push(source);
    }
}

pub const fn pack_chunk_pos(pos: ChunkPos) -> i64 {
    (pos.0.x as i64 & 0xFFFF_FFFF) | ((pos.0.y as i64 & 0xFFFF_FFFF) << 32)
}

pub const fn unpack_chunk_pos(key: i64) -> ChunkPos {
    ChunkPos::new(key as i32, (key >> 32) as i32)
}
