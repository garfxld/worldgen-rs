use std::mem;

use super::gaussian::MarsagliaPolarGaussian;
use super::name_hash::NameHash;
use super::{RandomFactory, RandomSource, get_seed};

const GOLDEN_RATIO_64: u64 = 0x9E37_79B9_7F4A_7C15;
const SILVER_RATIO_64: u64 = 0x6A09_E667_F3BC_C909;

pub struct XoroshiroRandomSource {
    seed_lo: u64,
    seed_hi: u64,
    gaussian: MarsagliaPolarGaussian,
}

impl XoroshiroRandomSource {
    pub const fn from_seed(seed: u64) -> Self {
        let (lo, hi) = Self::upgrade_seed_to_128_bit(seed);
        let lo = mix_stafford_13(lo);
        let hi = mix_stafford_13(hi);
        Self::new(lo, hi)
    }

    const fn new(lo: u64, hi: u64) -> Self {
        let (lo, hi) = if (lo | hi) == 0 {
            (GOLDEN_RATIO_64, SILVER_RATIO_64)
        } else {
            (lo, hi)
        };
        Self {
            seed_lo: lo,
            seed_hi: hi,
            gaussian: MarsagliaPolarGaussian::new(),
        }
    }

    const fn upgrade_seed_to_128_bit(seed: u64) -> (u64, u64) {
        let lo = seed ^ SILVER_RATIO_64;
        let hi = lo.wrapping_add(GOLDEN_RATIO_64);
        (lo, hi)
    }

    const fn next(&mut self, bits: u64) -> u64 {
        self.next_random() >> (64 - bits)
    }

    const fn next_random(&mut self) -> u64 {
        let s0 = self.seed_lo;
        let mut s1 = self.seed_hi;
        let result = s0.wrapping_add(s1).rotate_left(17).wrapping_add(s0);
        s1 ^= s0;
        self.seed_lo = s0.rotate_left(49) ^ s1 ^ (s1 << 21);
        self.seed_hi = s1.rotate_left(28);
        result
    }

    pub const fn set_seed(&mut self, seed: i64) {
        *self = Self::from_seed(seed as u64);
    }

    pub const fn fork(&mut self) -> Self {
        Self::new(self.next_random(), self.next_random())
    }

    pub const fn next_i32(&mut self) -> i32 {
        self.next_random() as i32
    }

    pub fn next_i32_bounded(&mut self, bound: i32) -> i32 {
        let mut random_bits = (self.next_i32() as u64) & 0xFFFF_FFFF;
        let mut multiplied_random_bits = random_bits.wrapping_mul(bound as u64);
        let mut fractional_part = multiplied_random_bits & 0xFFFF_FFFF;
        if fractional_part < bound as u64 {
            let unbiased_buckets_start_index =
                u64::from(((!bound as u32).wrapping_add(1)) % bound as u32);
            while fractional_part < unbiased_buckets_start_index {
                random_bits = (self.next_i32() as u64) & 0xFFFF_FFFF;
                multiplied_random_bits = random_bits.wrapping_mul(bound as u64);
                fractional_part = multiplied_random_bits & 0xFFFF_FFFF;
            }
        }
        let integer_part = multiplied_random_bits >> 32;
        integer_part as i32
    }

    pub fn next_i32_between(&mut self, min: i32, max: i32) -> i32 {
        self.next_i32_bounded(max - min + 1) + min
    }

    pub fn next_i32_between_exclusive(&mut self, min: i32, max: i32) -> i32 {
        min + self.next_i32_bounded(max - min)
    }

    pub const fn next_i64(&mut self) -> i64 {
        self.next_random() as i64
    }

    pub fn next_f32(&mut self) -> f32 {
        self.next(24) as f32 * 5.960_464_5e-8
    }

    pub fn next_f64(&mut self) -> f64 {
        self.next(53) as f64 * f64::from(1.110_223e-16_f32)
    }

    pub const fn next_bool(&mut self) -> bool {
        (self.next_random() & 1) != 0
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
        RandomFactory::Xoroshiro(XoroshiroRandomFactory {
            seed_lo: self.next_random(),
            seed_hi: self.next_random(),
        })
    }

    pub fn consume_count(&mut self, count: i32) {
        for _ in 0..count {
            self.next_i32();
        }
    }
}

const fn mix_stafford_13(z: u64) -> u64 {
    let z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    let z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[derive(Clone)]
pub struct XoroshiroRandomFactory {
    seed_lo: u64,
    seed_hi: u64,
}

impl XoroshiroRandomFactory {
    pub fn at(&self, x: i32, y: i32, z: i32) -> RandomSource {
        let positional_seed = get_seed(x, y, z) as u64;
        let random_seed = positional_seed ^ self.seed_lo;

        RandomSource::Xoroshiro(XoroshiroRandomSource::new(random_seed, self.seed_hi))
    }

    pub const fn with_hash_of(&self, hash: &NameHash) -> RandomSource {
        let [hash_lo, hash_hi] = hash.md5_digest;
        RandomSource::Xoroshiro(XoroshiroRandomSource::new(
            hash_lo ^ self.seed_lo,
            hash_hi ^ self.seed_hi,
        ))
    }

    pub const fn with_seed(&self, seed: u64) -> RandomSource {
        RandomSource::Xoroshiro(XoroshiroRandomSource::new(
            seed ^ self.seed_lo,
            seed ^ self.seed_hi,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIX_STAFFORD_13_CASES: &[(u64, i64)] = &[
        (0, 0),
        (1, 6238072747940578789),
        (64, -8456553050427055661),
        (4096, -1125827887270283392),
        (262144, -120227641678947436),
        (16777216, 6406066033425044679),
        (1073741824, 3143522559155490559),
        (16, -2773008118984693571),
        (1024, 8101005175654470197),
        (65536, -3551754741763842827),
    ];

    #[test]
    fn mix_stafford_13_matches_java() {
        for &(input, expected) in MIX_STAFFORD_13_CASES {
            assert_eq!(mix_stafford_13(input), expected as u64);
        }
    }

    #[test]
    fn next_i32_sequence_matches_java() {
        const EXPECTED: [i32; 5] = [-160476802, 781697906, 653572596, 1337520923, -505875771];
        let mut rng = XoroshiroRandomSource::from_seed(0);
        for &expected in &EXPECTED {
            assert_eq!(rng.next_i32(), expected);
        }
    }

    #[test]
    fn next_i32_bounded_matches_java() {
        const SMALL: [i32; 5] = [9, 1, 1, 3, 8];
        let mut rng = XoroshiroRandomSource::from_seed(0);
        for &expected in &SMALL {
            assert_eq!(rng.next_i32_bounded(10), expected);
        }
    }

    #[test]
    fn next_i32_between_uses_inclusive_bound() {
        let mut inclusive = XoroshiroRandomSource::from_seed(0);
        let mut exclusive = XoroshiroRandomSource::from_seed(0);
        assert_eq!(
            inclusive.next_i32_between(50, 100),
            exclusive.next_i32_between_exclusive(50, 100) + 1
        );
    }

    #[test]
    fn next_f64_matches_java() {
        const EXPECTED: [f64; 3] = [0.16474369376959186, 0.7997457290026366, 0.2511961888876212];
        let mut rng = XoroshiroRandomSource::from_seed(0);
        for &expected in &EXPECTED {
            assert_eq!(rng.next_f64(), expected);
        }
    }

    #[test]
    fn next_f32_matches_java() {
        const EXPECTED: [f32; 3] = [0.16474366, 0.7997457, 0.25119615];
        let mut rng = XoroshiroRandomSource::from_seed(0);
        for &expected in &EXPECTED {
            assert_eq!(rng.next_f32(), expected);
        }
    }

    #[test]
    fn next_i64_matches_java() {
        const EXPECTED: [i64; 3] = [
            3038984756725240190,
            -3694039286755638414,
            4633751808701151732,
        ];
        let mut rng = XoroshiroRandomSource::from_seed(0);
        for &expected in &EXPECTED {
            assert_eq!(rng.next_i64(), expected);
        }
    }

    #[test]
    fn next_bool_matches_java() {
        const EXPECTED: [bool; 6] = [false, false, false, true, true, true];
        let mut rng = XoroshiroRandomSource::from_seed(0);
        for &expected in &EXPECTED {
            assert_eq!(rng.next_bool(), expected);
        }
    }

    #[test]
    fn next_gaussian_matches_java() {
        const EXPECTED: [f64; 3] = [
            -0.48540690699780015,
            0.43399227545320296,
            -0.3283265251019599,
        ];
        let mut rng = XoroshiroRandomSource::from_seed(0);
        for &expected in &EXPECTED {
            assert_eq!(rng.next_gaussian(), expected);
        }
    }

    #[test]
    fn triangle_matches_java() {
        const EXPECTED: [f64; 3] = [6.824989823834776, 10.670356470906125, 6.71516367803936];
        let mut rng = XoroshiroRandomSource::from_seed(0);
        for &expected in &EXPECTED {
            assert_eq!(rng.triangle(10.0, 5.0), expected);
        }
    }

    #[test]
    fn fork_creates_independent_rng() {
        let mut rng = XoroshiroRandomSource::from_seed(0);
        let mut forked = rng.fork();

        assert_eq!(forked.next_i32(), 542195535);
        assert_eq!(rng.next_i32(), 653572596);
    }

    #[test]
    fn positional_factory_produces_java_matching_sources() {
        let mut rng = XoroshiroRandomSource::from_seed(0);
        let mut forked = rng.fork();
        assert_eq!(forked.next_i32(), 542195535);

        let factory = forked.next_positional();

        let RandomSource::Xoroshiro(mut rand1) =
            factory.with_hash_of(&NameHash::new("TEST STRING"))
        else {
            panic!("expected Xoroshiro variant");
        };
        assert_eq!(rand1.next_i32(), -641435713);

        let RandomSource::Xoroshiro(mut rand2) = factory.with_seed(42069) else {
            panic!("expected Xoroshiro variant");
        };
        assert_eq!(rand2.next_i32(), -340700677);

        let RandomSource::Xoroshiro(mut rand3) = factory.at(1337, 80085, -69420) else {
            panic!("expected Xoroshiro variant");
        };
        assert_eq!(rand3.next_i32(), 790449132);

        assert_eq!(rng.next_i32(), 653572596);
        assert_eq!(forked.next_i32(), 435917842);
    }

    #[test]
    fn zero_seed_falls_back_to_golden_silver_ratio() {
        let mut rng = XoroshiroRandomSource::new(0, 0);
        assert_eq!(rng.next_i64(), 6807859099481836695);
    }
}
