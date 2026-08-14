use std::mem;

use super::gaussian::MarsagliaPolarGaussian;
use super::name_hash::NameHash;
use super::{RandomFactory, RandomSource, get_seed};

const MULTIPLIER: u64 = 0x0005_DEEC_E66D;
const INCREMENT: u64 = 0xB;
const MODULUS_MASK: u64 = 0xFFFF_FFFF_FFFF;

pub struct LegacyRandomSource {
    seed: i64,
    gaussian: MarsagliaPolarGaussian,
}

impl LegacyRandomSource {
    pub const fn from_seed(seed: u64) -> Self {
        Self {
            seed: (seed as i64 ^ MULTIPLIER as i64) & MODULUS_MASK as i64,
            gaussian: MarsagliaPolarGaussian::new(),
        }
    }

    pub const fn get_seed(&self) -> i64 {
        self.seed
    }

    pub const fn set_seed(&mut self, seed: i64) {
        self.seed = (seed ^ MULTIPLIER as i64) & MODULUS_MASK as i64;
        self.gaussian = MarsagliaPolarGaussian::new();
    }

    pub fn set_large_feature_seed(&mut self, seed: i64, chunk_x: i32, chunk_z: i32) {
        self.set_seed(seed);
        let x_mul = self.next_i64();
        let z_mul = self.next_i64();
        self.set_seed(
            i64::from(chunk_x).wrapping_mul(x_mul) ^ i64::from(chunk_z).wrapping_mul(z_mul) ^ seed,
        );
    }

    pub fn set_large_feature_with_salt(&mut self, seed: i64, x: i32, z: i32, salt: i32) {
        self.set_seed(
            i64::from(x)
                .wrapping_mul(341_873_128_712)
                .wrapping_add(i64::from(z).wrapping_mul(132_897_987_541))
                .wrapping_add(seed)
                .wrapping_add(i64::from(salt)),
        );
    }

    const fn next(&mut self, bits: u64) -> i32 {
        (self.next_random() >> (48 - bits)) as i32
    }

    const fn next_random(&mut self) -> i64 {
        let old_seed = self.seed as u64;
        let new_seed = (old_seed.wrapping_mul(MULTIPLIER).wrapping_add(INCREMENT)) & MODULUS_MASK;
        self.seed = new_seed as i64;
        new_seed as i64
    }

    const fn skip(&mut self, count: u64) {
        let mut acc_a: u64 = 1;
        let mut acc_c: u64 = 0;
        let mut base_a: u64 = MULTIPLIER;
        let mut base_c: u64 = INCREMENT;
        let mut k = count;
        while k > 0 {
            if k & 1 == 1 {
                acc_c = base_a.wrapping_mul(acc_c).wrapping_add(base_c) & MODULUS_MASK;
                acc_a = base_a.wrapping_mul(acc_a) & MODULUS_MASK;
            }
            base_c = base_a.wrapping_mul(base_c).wrapping_add(base_c) & MODULUS_MASK;
            base_a = base_a.wrapping_mul(base_a) & MODULUS_MASK;
            k >>= 1;
        }
        let s = self.seed as u64;
        self.seed = (acc_a.wrapping_mul(s).wrapping_add(acc_c) & MODULUS_MASK) as i64;
    }

    pub fn fork(&mut self) -> Self {
        Self::from_seed(self.next_i64() as u64)
    }

    pub const fn next_i32(&mut self) -> i32 {
        self.next(32)
    }

    pub fn next_i32_bounded(&mut self, bound: i32) -> i32 {
        if bound & bound.wrapping_sub(1) == 0 {
            (i64::from(bound).wrapping_mul(i64::from(self.next(31))) >> 31) as i32
        } else {
            loop {
                let sample = self.next(31);
                let modulo = sample % bound;
                if sample.wrapping_sub(modulo).wrapping_add(bound.wrapping_sub(1)) >= 0 {
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
        self.next(24) as f32 * 5.960_464_5e-8_f32
    }

    pub fn next_f64(&mut self) -> f64 {
        let upper = self.next(26);
        let lower = self.next(27);
        let combined = (i64::from(upper) << 27) + i64::from(lower);
        combined as f64 * (1.0 / (1_i64 << 53) as f64)
    }

    pub const fn next_bool(&mut self) -> bool {
        self.next(1) != 0
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

    pub fn next_positional(&mut self) -> RandomFactory {
        RandomFactory::Legacy(LegacyRandomFactory::new(self.next_i64()))
    }

    pub const fn consume_count(&mut self, count: i32) {
        if count > 0 {
            self.skip(count as u64);
        }
    }
}

#[derive(Clone)]
pub struct LegacyRandomFactory {
    seed: i64,
}

impl LegacyRandomFactory {
    pub const fn new(seed: i64) -> Self {
        Self { seed }
    }

    pub fn at(&self, x: i32, y: i32, z: i32) -> RandomSource {
        let positional_seed = get_seed(x, y, z);
        RandomSource::Legacy(LegacyRandomSource::from_seed(
            (positional_seed as u64) ^ (self.seed as u64),
        ))
    }

    pub const fn with_hash_of(&self, hash: &NameHash) -> RandomSource {
        RandomSource::Legacy(LegacyRandomSource::from_seed(
            (hash.legacy_hash_code as u64) ^ (self.seed as u64),
        ))
    }

    pub const fn with_seed(&self, seed: u64) -> RandomSource {
        RandomSource::Legacy(LegacyRandomSource::from_seed(seed))
    }
}

#[cfg(test)]
mod tests {
    use super::LegacyRandomSource;
    use crate::util::rng::{RandomFactory, name_hash::NameHash};

    #[test]
    fn next_i32_matches_java() {
        let mut rand = LegacyRandomSource::from_seed(0);
        let values = [
            -1_155_484_576,
            -723_955_400,
            1_033_096_058,
            -1_690_734_402,
            -1_557_280_266,
        ];
        for value in values {
            assert_eq!(rand.next_i32(), value);
        }
    }

    #[test]
    fn next_i32_bounded_matches_java() {
        let mut rand = LegacyRandomSource::from_seed(0);
        let values = [0, 13, 4, 2, 5, 8, 11, 6, 9, 14];
        for value in values {
            assert_eq!(rand.next_i32_bounded(0xf), value);
        }

        let mut power_of_two = LegacyRandomSource::from_seed(0);
        let expected = [1, 1, 0, 1, 1, 0, 1, 0, 1, 1];
        for value in expected {
            assert_eq!(power_of_two.next_i32_bounded(2), value);
        }
    }

    #[test]
    fn next_i32_between_matches_java() {
        let mut rand = LegacyRandomSource::from_seed(0);
        let values = [1, 5, 2, 12, 12, 6, 12, 10, 4, 3];
        for value in values {
            assert_eq!(rand.next_i32_between(1, 12), value);
        }
    }

    #[test]
    fn next_f64_matches_java() {
        let mut rand = LegacyRandomSource::from_seed(0);
        let values = [
            0.730_967_787_376_657,
            0.240_536_415_671_485_87,
            0.637_417_425_350_108_3,
        ];
        for value in values {
            assert_eq!(rand.next_f64(), value);
        }
    }

    #[test]
    fn next_f32_matches_java() {
        let mut rand = LegacyRandomSource::from_seed(0);
        let values: [f32; 3] = [0.730_967_76, 0.831_441, 0.240_536_39];
        for value in values {
            assert_eq!(rand.next_f32(), value);
        }
    }

    #[test]
    fn next_i64_matches_java() {
        let mut rand = LegacyRandomSource::from_seed(0);
        let values: [i64; 3] = [
            -4_962_768_465_676_381_896,
            4_437_113_781_045_784_766,
            -6_688_467_811_848_818_630,
        ];
        for value in values {
            assert_eq!(rand.next_i64(), value);
        }
    }

    #[test]
    fn next_bool_matches_java() {
        let mut rand = LegacyRandomSource::from_seed(0);
        let values = [true, true, false, true, true, false];
        for value in values {
            assert_eq!(rand.next_bool(), value);
        }
    }

    #[test]
    fn next_gaussian_matches_java() {
        let mut rand = LegacyRandomSource::from_seed(0);
        let values = [
            0.802_533_063_739_030_5,
            -0.901_546_088_417_512_2,
            2.080_920_790_428_163,
        ];
        for value in values {
            assert_eq!(rand.next_gaussian(), value);
        }
    }

    #[test]
    fn triangle_matches_java() {
        let mut rand = LegacyRandomSource::from_seed(0);
        let values = [
            124.521_568_585_258_56,
            104.349_021_011_623_72,
            113.216_343_916_027_6,
        ];
        for value in values {
            assert_eq!(rand.triangle(100_f64, 50_f64), value);
        }
    }

    #[test]
    fn consume_count_skip_matches_naive_loop() {
        const COUNTS: &[i32] = &[0, 1, 2, 7, 31, 32, 33, 100, 262, 1023, 1024, 17292];
        const SEEDS: &[u64] = &[0, 1, 0xDEAD_BEEF, 0x1234_5678_9ABC_DEF0];

        for &seed in SEEDS {
            for &count in COUNTS {
                let mut fast = LegacyRandomSource::from_seed(seed);
                let mut slow = LegacyRandomSource::from_seed(seed);
                fast.consume_count(count);
                for _ in 0..count {
                    slow.next_i32();
                }
                assert_eq!(
                    fast.next_i64(),
                    slow.next_i64(),
                    "mismatch at seed={seed:#x} count={count}"
                );
            }
        }
    }

    #[test]
    fn consume_count_negative_is_noop() {
        let mut a = LegacyRandomSource::from_seed(42);
        let mut b = LegacyRandomSource::from_seed(42);
        a.consume_count(-1);
        a.consume_count(i32::MIN);
        assert_eq!(a.next_i64(), b.next_i64());
    }

    #[test]
    fn fork_and_positional_factory_match_vanilla_trace() {
        let mut original = LegacyRandomSource::from_seed(0);
        {
            let RandomFactory::Legacy(factory) = original.next_positional() else {
                unreachable!()
            };
            assert_eq!(factory.seed, -4_962_768_465_676_381_896_i64);

            let mut rand = factory.with_hash_of(&NameHash::new("minecraft:offset"));
            assert_eq!(rand.next_i32(), 103_436_829);
        }

        let mut original = LegacyRandomSource::from_seed(0);
        let mut forked = original.fork();
        {
            let factory = forked.next_positional();

            let mut rand1 = factory.with_hash_of(&NameHash::new("TEST STRING"));
            assert_eq!(rand1.next_i32(), -1_170_413_697);

            let mut rand2 = factory.with_seed(10);
            assert_eq!(rand2.next_i32(), -1_157_793_070);

            let mut rand3 = factory.at(1, 11, -111);
            assert_eq!(rand3.next_i32(), -1_213_890_343);
        }

        assert_eq!(original.next_i32(), 1_033_096_058);
        assert_eq!(forked.next_i32(), -888_301_832);
    }

    #[test]
    fn set_seed_matches_from_seed() {
        let mut fresh = LegacyRandomSource::from_seed(12345);
        let mut reseeded = LegacyRandomSource::from_seed(0);
        reseeded.set_seed(12345);
        for _ in 0..10 {
            assert_eq!(fresh.next_i64(), reseeded.next_i64());
        }
    }

    #[test]
    fn set_large_feature_with_salt_trivial_seed() {
        let mut rng = LegacyRandomSource::from_seed(0);
        rng.set_large_feature_with_salt(0, 0, 0, 10_387_312);
        let mut expected = LegacyRandomSource::from_seed(0);
        expected.set_seed(10_387_312);
        for _ in 0..5 {
            assert_eq!(rng.next_i32(), expected.next_i32());
        }
    }

    #[test]
    fn set_large_feature_with_salt_combines_coords() {
        let mut rng = LegacyRandomSource::from_seed(0);
        rng.set_large_feature_with_salt(123_456_789, 5, -3, 10_387_312);
        let expected_seed: i64 =
            5_i64 * 341_873_128_712 + (-3_i64) * 132_897_987_541 + 123_456_789 + 10_387_312;
        let mut expected = LegacyRandomSource::from_seed(0);
        expected.set_seed(expected_seed);
        for _ in 0..5 {
            assert_eq!(rng.next_i32(), expected.next_i32());
        }
    }

    #[test]
    fn set_large_feature_seed_xors_chunk_multiples() {
        let x_mul = -4_962_768_465_676_381_896_i64;
        let z_mul = 4_437_113_781_045_784_766_i64;
        let expected_seed = 3_i64.wrapping_mul(x_mul) ^ 5_i64.wrapping_mul(z_mul);

        let mut rng = LegacyRandomSource::from_seed(0);
        rng.set_large_feature_seed(0, 3, 5);
        let mut expected = LegacyRandomSource::from_seed(0);
        expected.set_seed(expected_seed);
        for _ in 0..5 {
            assert_eq!(rng.next_i32(), expected.next_i32());
        }
    }
}
