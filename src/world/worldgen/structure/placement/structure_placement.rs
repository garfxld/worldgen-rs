use glam::IVec3;
use shulkr::util::Key;

use crate::util::pos::ChunkPos;
use crate::util::rng::legacy_random::LegacyRandomSource;
use crate::world::worldgen::structure::structure::BiomeHolderSet;

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
                let mut random = LegacyRandomSource::from_seed(
                    (chunk_x ^ (chunk_z << 4)) as i64 as u64 ^ seed as u64,
                );
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
    pub other_set: Key,
    pub chunk_count: i32,
}

#[derive(Debug, Clone)]
pub enum StructurePlacement {
    RandomSpread(RandomSpreadStructurePlacement),
    ConcentricRings(ConcentricRingsStructurePlacement),
    DimensionOrigin,
}

#[derive(Debug, Clone)]
pub struct AbstractSpreadingStructurePlacement {
    pub locate_offset: IVec3,
    pub frequency_reduction_method: FrequencyReductionMethod,
    pub frequency: f32,
    pub salt: i32,
    pub exclusion_zone: Option<ExclusionZone>,
}

#[derive(Debug, Clone)]
pub struct RandomSpreadStructurePlacement {
    pub spreading: AbstractSpreadingStructurePlacement,
    pub spacing: i32,
    pub separation: i32,
    pub spread_type: RandomSpreadType,
}

#[derive(Debug, Clone)]
pub struct ConcentricRingsStructurePlacement {
    pub spreading: AbstractSpreadingStructurePlacement,
    pub distance: i32,
    pub spread: i32,
    pub count: i32,
    pub preferred_biomes: BiomeHolderSet,
}

impl StructurePlacement {
    pub const fn spreading(&self) -> Option<&AbstractSpreadingStructurePlacement> {
        match self {
            Self::RandomSpread(placement) => Some(&placement.spreading),
            Self::ConcentricRings(placement) => Some(&placement.spreading),
            Self::DimensionOrigin => None,
        }
    }

    pub const fn locate_offset(&self) -> IVec3 {
        match self.spreading() {
            Some(spreading) => spreading.locate_offset,
            None => IVec3::ZERO,
        }
    }
}

impl AbstractSpreadingStructurePlacement {
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
}

impl RandomSpreadStructurePlacement {
    pub fn get_potential_structure_chunk(
        &self,
        seed: i64,
        source_x: i32,
        source_z: i32,
    ) -> ChunkPos {
        let spaced_grid_x = source_x.div_euclid(self.spacing);
        let spaced_grid_z = source_z.div_euclid(self.spacing);
        let mut random = LegacyRandomSource::from_seed(0);
        random.set_large_feature_with_salt(seed, spaced_grid_x, spaced_grid_z, self.spreading.salt);
        let limit = self.spacing - self.separation;
        let spread_x = self.spread_type.evaluate(&mut random, limit);
        let spread_z = self.spread_type.evaluate(&mut random, limit);
        ChunkPos::new(
            spaced_grid_x * self.spacing + spread_x,
            spaced_grid_z * self.spacing + spread_z,
        )
    }

    pub fn is_placement_chunk(&self, seed: i64, source_x: i32, source_z: i32) -> bool {
        let pos = self.get_potential_structure_chunk(seed, source_x, source_z);
        pos.0.x == source_x && pos.0.y == source_z
    }
}
