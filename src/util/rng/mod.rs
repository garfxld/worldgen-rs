pub mod gaussian;
pub mod legacy_random;
pub mod name_hash;
pub mod worldgen_random;
pub mod xoroshiro;

use legacy_random::{LegacyRandomFactory, LegacyRandomSource};
use name_hash::NameHash;
use xoroshiro::{XoroshiroRandomFactory, XoroshiroRandomSource};

pub enum RandomSource {
    Xoroshiro(XoroshiroRandomSource),
    Legacy(LegacyRandomSource),
}

impl RandomSource {
    pub fn fork(&mut self) -> Self {
        match self {
            Self::Xoroshiro(r) => Self::Xoroshiro(r.fork()),
            Self::Legacy(r) => Self::Legacy(r.fork()),
        }
    }

    pub const fn next_i32(&mut self) -> i32 {
        match self {
            Self::Xoroshiro(r) => r.next_i32(),
            Self::Legacy(r) => r.next_i32(),
        }
    }

    pub fn next_i32_bounded(&mut self, bound: i32) -> i32 {
        match self {
            Self::Xoroshiro(r) => r.next_i32_bounded(bound),
            Self::Legacy(r) => r.next_i32_bounded(bound),
        }
    }

    pub fn next_i32_between(&mut self, min: i32, max: i32) -> i32 {
        self.next_i32_bounded(max - min + 1) + min
    }

    pub fn next_i32_between_exclusive(&mut self, min: i32, max: i32) -> i32 {
        min + self.next_i32_bounded(max - min)
    }

    pub fn next_i64(&mut self) -> i64 {
        match self {
            Self::Xoroshiro(r) => r.next_i64(),
            Self::Legacy(r) => r.next_i64(),
        }
    }

    pub fn next_f32(&mut self) -> f32 {
        match self {
            Self::Xoroshiro(r) => r.next_f32(),
            Self::Legacy(r) => r.next_f32(),
        }
    }

    pub fn next_f64(&mut self) -> f64 {
        match self {
            Self::Xoroshiro(r) => r.next_f64(),
            Self::Legacy(r) => r.next_f64(),
        }
    }

    pub const fn next_bool(&mut self) -> bool {
        match self {
            Self::Xoroshiro(r) => r.next_bool(),
            Self::Legacy(r) => r.next_bool(),
        }
    }

    pub fn next_gaussian(&mut self) -> f64 {
        match self {
            Self::Xoroshiro(r) => r.next_gaussian(),
            Self::Legacy(r) => r.next_gaussian(),
        }
    }

    pub fn triangle(&mut self, min: f64, max: f64) -> f64 {
        min + max * (self.next_f64() - self.next_f64())
    }

    pub fn triangle_f32(&mut self, min: f32, max: f32) -> f32 {
        min + max * (self.next_f32() - self.next_f32())
    }

    pub fn next_positional(&mut self) -> RandomFactory {
        match self {
            Self::Xoroshiro(r) => r.next_positional(),
            Self::Legacy(r) => r.next_positional(),
        }
    }

    pub fn consume_count(&mut self, count: i32) {
        match self {
            Self::Xoroshiro(r) => r.consume_count(count),
            Self::Legacy(r) => r.consume_count(count),
        }
    }
}

#[derive(Clone)]
pub enum RandomFactory {
    Xoroshiro(XoroshiroRandomFactory),
    Legacy(LegacyRandomFactory),
}

impl RandomFactory {
    pub fn at(&self, x: i32, y: i32, z: i32) -> RandomSource {
        match self {
            Self::Xoroshiro(f) => f.at(x, y, z),
            Self::Legacy(f) => f.at(x, y, z),
        }
    }

    pub const fn with_hash_of(&self, hash: &NameHash) -> RandomSource {
        match self {
            Self::Xoroshiro(f) => f.with_hash_of(hash),
            Self::Legacy(f) => f.with_hash_of(hash),
        }
    }

    pub const fn with_seed(&self, seed: u64) -> RandomSource {
        match self {
            Self::Xoroshiro(f) => f.with_seed(seed),
            Self::Legacy(f) => f.with_seed(seed),
        }
    }
}

pub fn get_seed(x: i32, y: i32, z: i32) -> i64 {
    let l = i64::from(x.wrapping_mul(3_129_871))
        ^ (i64::from(z).wrapping_mul(116_129_781_i64))
        ^ i64::from(y);
    let l = l
        .wrapping_mul(l)
        .wrapping_mul(42_317_861_i64)
        .wrapping_add(l.wrapping_mul(11));
    l >> 16
}
