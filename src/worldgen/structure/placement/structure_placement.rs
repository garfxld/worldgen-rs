use glam::IVec3;

use crate::util::ident::Identifier;
use crate::worldgen::structure::structure::BiomeHolderSet;
use crate::util::pos::{BlockPos, ChunkPos};
use crate::util::rng::legacy_random::LegacyRandomSource;

const HIGHLY_ARBITRARY_RANDOM_SALT: i32 = 10_387_320;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RandomSpreadType {
    Linear,
    Triangular,
}

impl RandomSpreadType {
    pub fn evaluate(self, random: &mut LegacyRandomSource, limit: i32) -> i32 {
        match self {
            Self::Linear => random.next_i32_bounded(limit),
            Self::Triangular => {
                (random.next_i32_bounded(limit) + random.next_i32_bounded(limit)) / 2
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrequencyReductionMethod {
    Default,
    LegacyType1,
    LegacyType2,
    LegacyType3,
}

impl FrequencyReductionMethod {
    pub fn should_generate(
        self,
        seed: i64,
        salt: i32,
        source_x: i32,
        source_z: i32,
        probability: f32,
    ) -> bool {
        match self {
            Self::Default => {
                let mut random = LegacyRandomSource::from_seed(0);
                random.set_large_feature_with_salt(seed, source_x, source_z, salt);
                random.next_f32() < probability
            }
            Self::LegacyType1 => {
                let chunk_x = source_x >> 4;
                let chunk_z = source_z >> 4;
                let mut random =
                    LegacyRandomSource::from_seed((chunk_x ^ (chunk_z << 4)) as i64 as u64 ^ seed as u64);
                random.next_i32();
                random.next_i32_bounded((1.0 / probability) as i32) == 0
            }
            Self::LegacyType2 => {
                let mut random = LegacyRandomSource::from_seed(0);
                random.set_large_feature_with_salt(
                    seed,
                    source_x,
                    source_z,
                    HIGHLY_ARBITRARY_RANDOM_SALT,
                );
                random.next_f32() < probability
            }
            Self::LegacyType3 => {
                let mut random = LegacyRandomSource::from_seed(0);
                random.set_large_feature_seed(seed, source_x, source_z);
                random.next_f64() < f64::from(probability)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExclusionZone {
    pub other_set: Identifier,
    pub chunk_count: i32,
}

#[derive(Debug, Clone)]
pub struct StructurePlacement {
    pub locate_offset: IVec3,
    pub frequency_reduction_method: FrequencyReductionMethod,
    pub frequency: f32,
    pub salt: i32,
    pub exclusion_zone: Option<ExclusionZone>,
    pub kind: StructurePlacementKind,
}

#[derive(Debug, Clone)]
pub enum StructurePlacementKind {
    RandomSpread {
        spacing: i32,
        separation: i32,
        spread_type: RandomSpreadType,
    },
    ConcentricRings {
        distance: i32,
        spread: i32,
        count: i32,
        preferred_biomes: BiomeHolderSet,
    },
}

impl StructurePlacement {
    pub fn apply_additional_chunk_restrictions(
        &self,
        source_x: i32,
        source_z: i32,
        level_seed: i64,
    ) -> bool {
        self.frequency >= 1.0
            || self.frequency_reduction_method.should_generate(
                level_seed,
                self.salt,
                source_x,
                source_z,
                self.frequency,
            )
    }

    pub fn get_potential_structure_chunk(
        &self,
        seed: i64,
        source_x: i32,
        source_z: i32,
    ) -> Option<ChunkPos> {
        let StructurePlacementKind::RandomSpread {
            spacing,
            separation,
            spread_type,
        } = self.kind
        else {
            return None;
        };

        let spaced_grid_x = source_x.div_euclid(spacing);
        let spaced_grid_z = source_z.div_euclid(spacing);
        let mut random = LegacyRandomSource::from_seed(0);
        random.set_large_feature_with_salt(seed, spaced_grid_x, spaced_grid_z, self.salt);
        let limit = spacing - separation;
        let spread_x = spread_type.evaluate(&mut random, limit);
        let spread_z = spread_type.evaluate(&mut random, limit);
        Some(ChunkPos::new(
            spaced_grid_x * spacing + spread_x,
            spaced_grid_z * spacing + spread_z,
        ))
    }

    pub fn is_placement_chunk(
        &self,
        seed: i64,
        source_x: i32,
        source_z: i32,
        ring_positions: Option<&[ChunkPos]>,
    ) -> bool {
        match self.kind {
            StructurePlacementKind::RandomSpread { .. } => self
                .get_potential_structure_chunk(seed, source_x, source_z)
                .is_some_and(|pos| pos.0.x == source_x && pos.0.y == source_z),
            StructurePlacementKind::ConcentricRings { .. } => ring_positions
                .is_some_and(|positions| {
                    positions
                        .iter()
                        .any(|pos| pos.0.x == source_x && pos.0.y == source_z)
                }),
        }
    }

    pub const fn get_locate_pos(&self, chunk_pos: ChunkPos) -> BlockPos {
        BlockPos::new(
            chunk_pos.0.x * 16 + self.locate_offset.x,
            self.locate_offset.y,
            chunk_pos.0.y * 16 + self.locate_offset.z,
        )
    }
}
