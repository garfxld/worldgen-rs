use std::mem;

use super::RandomFactory;
use super::gaussian::MarsagliaPolarGaussian;
use super::xoroshiro::XoroshiroRandomSource;

pub struct WorldgenRandom {
    source: XoroshiroRandomSource,
    gaussian: MarsagliaPolarGaussian,
}

impl WorldgenRandom {
    #[must_use]
    pub const fn from_seed(seed: u64) -> Self {
        Self {
            source: XoroshiroRandomSource::from_seed(seed),
            gaussian: MarsagliaPolarGaussian::new(),
        }
    }

    pub const fn set_seed(&mut self, seed: i64) {
        self.source.set_seed(seed);
    }

    pub fn set_decoration_seed(&mut self, seed: i64, block_x: i32, block_z: i32) -> i64 {
        self.set_seed(seed);
        let x_scale = self.next_i64() | 1;
        let z_scale = self.next_i64() | 1;
        let decoration_seed = i64::from(block_x)
            .wrapping_mul(x_scale)
            .wrapping_add(i64::from(block_z).wrapping_mul(z_scale))
            ^ seed;
        self.set_seed(decoration_seed);
        decoration_seed
    }

    pub fn set_feature_seed(&mut self, decoration_seed: i64, feature_index: i32, step: i32) {
        let feature_seed = decoration_seed
            .wrapping_add(i64::from(feature_index))
            .wrapping_add(10_000_i64.wrapping_mul(i64::from(step)));
        self.set_seed(feature_seed);
    }

    const fn next_bits(&mut self, bits: u64) -> u64 {
        self.source.next_i64() as u64 >> (64 - bits)
    }

    #[must_use]
    pub const fn fork(&mut self) -> Self {
        Self {
            source: self.source.fork(),
            gaussian: MarsagliaPolarGaussian::new(),
        }
    }

    pub const fn next_i32(&mut self) -> i32 {
        self.next_bits(32) as i32
    }

    pub fn next_i32_bounded(&mut self, bound: i32) -> i32 {
        if bound & bound.wrapping_sub(1) == 0 {
            (i64::from(bound).wrapping_mul(i64::from(self.next_bits(31) as i32)) >> 31) as i32
        } else {
            loop {
                let sample = self.next_bits(31) as i32;
                let modulo = sample % bound;
                if sample
                    .wrapping_sub(modulo)
                    .wrapping_add(bound.wrapping_sub(1))
                    >= 0
                {
                    return modulo;
                }
            }
        }
    }

    pub fn next_i32_between(&mut self, min: i32, max: i32) -> i32 {
        self.next_i32_bounded(max - min + 1) + min
    }

    pub fn next_i32_between_exclusive(&mut self, min: i32, max: i32) -> i32 {
        min + self.next_i32_bounded(max - min)
    }

    pub fn next_i64(&mut self) -> i64 {
        let upper = self.next_i32();
        let lower = self.next_i32();
        (i64::from(upper) << 32).wrapping_add(i64::from(lower))
    }

    pub fn next_f32(&mut self) -> f32 {
        self.next_bits(24) as f32 * 5.960_464_5e-8_f32
    }

    pub fn next_f64(&mut self) -> f64 {
        let combined = ((self.next_bits(26) as i64) << 27) + self.next_bits(27) as i64;
        combined as f64 * (1.0 / (1_i64 << 53) as f64)
    }

    pub const fn next_bool(&mut self) -> bool {
        self.next_bits(1) != 0
    }

    pub fn next_gaussian(&mut self) -> f64 {
        let mut gaussian = mem::take(&mut self.gaussian);
        let value = gaussian.next(|| self.next_f64());
        self.gaussian = gaussian;
        value
    }

    pub fn triangle(&mut self, min: f64, max: f64) -> f64 {
        min + max * (self.next_f64() - self.next_f64())
    }

    pub fn triangle_f32(&mut self, min: f32, max: f32) -> f32 {
        min + max * (self.next_f32() - self.next_f32())
    }

    pub const fn next_positional(&mut self) -> RandomFactory {
        self.source.next_positional()
    }

    pub fn consume_count(&mut self, count: i32) {
        for _ in 0..count {
            self.next_i32();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::WorldgenRandom;

    #[test]
    fn set_decoration_seed_matches_vanilla_trace() {
        let mut random = WorldgenRandom::from_seed(0);
        assert_eq!(
            random.set_decoration_seed(13_579, -6_695_392, 5_868_656),
            7_632_291_757_650_236_667,
        );
    }

    #[test]
    fn feature_seed_matches_vanilla_first_ore_dirt_origin() {
        let mut random = WorldgenRandom::from_seed(0);
        let decoration_seed = random.set_decoration_seed(13_579, -6_695_392, 5_868_656);
        random.set_feature_seed(decoration_seed, 0, 6);

        let x = -6_695_392 + random.next_i32_bounded(16);
        let z = 5_868_656 + random.next_i32_bounded(16);
        let y = random.next_i32_bounded(161);
        assert_eq!((x, y, z), (-6_695_386, 149, 5_868_662));
    }

    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "gaussian cache parity must match vanilla exactly"
    )]
    fn feature_seed_preserves_pending_gaussian() {
        let mut random = WorldgenRandom::from_seed(123);
        let _ = random.next_gaussian();
        random.set_feature_seed(456, 7, 8);

        let mut cached_reference = WorldgenRandom::from_seed(123);
        let _ = cached_reference.next_gaussian();
        assert_eq!(random.next_gaussian(), cached_reference.next_gaussian());

        let mut reseeded_reference = WorldgenRandom::from_seed(0);
        reseeded_reference.set_feature_seed(456, 7, 8);
        assert_eq!(random.next_gaussian(), reseeded_reference.next_gaussian());
    }
}
