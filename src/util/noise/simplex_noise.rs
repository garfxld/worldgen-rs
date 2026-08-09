use crate::util::math::{corner_noise_3d, fast_floor};
use crate::util::rng::RandomSource;

#[expect(
    clippy::unreadable_literal,
    reason = "exact mathematical constant; underscores would obscure precision"
)]
const SQRT_3: f64 = 1.7320508075688772;
const F2: f64 = 0.5 * (SQRT_3 - 1.0);
const G2: f64 = (3.0 - SQRT_3) / 6.0;

#[derive(Debug, Clone)]
pub struct SimplexNoise {
    p: [i32; 512],
    pub xo: f64,
    pub yo: f64,
    pub zo: f64,
}

impl SimplexNoise {
    pub fn new(random: &mut RandomSource) -> Self {
        let xo = random.next_f64() * 256.0;
        let yo = random.next_f64() * 256.0;
        let zo = random.next_f64() * 256.0;

        let mut p = [0i32; 512];

        for (i, val) in p.iter_mut().enumerate().take(256) {
            *val = i as i32;
        }

        for i in 0..256 {
            let offset = random.next_i32_bounded((256 - i) as i32) as usize;
            p.swap(i, offset + i);
        }

        for i in 0..256 {
            p[i + 256] = p[i];
        }

        Self { p, xo, yo, zo }
    }

    #[inline]
    const fn p(&self, x: i32) -> i32 {
        self.p[(x & 0xFF) as usize]
    }

    #[must_use]
    pub fn get_value_2d(&self, xin: f64, yin: f64) -> f64 {
        let s = (xin + yin) * F2;
        let i = fast_floor(xin + s);
        let j = fast_floor(yin + s);
        let t = f64::from(i + j) * G2;
        let x0 = xin - (f64::from(i) - t);
        let y0 = yin - (f64::from(j) - t);

        let (i1, j1) = if x0 > y0 { (1, 0) } else { (0, 1) };

        let x1 = x0 - f64::from(i1) + G2;
        let y1 = y0 - f64::from(j1) + G2;
        let x2 = x0 - 1.0 + 2.0 * G2;
        let y2 = y0 - 1.0 + 2.0 * G2;

        let ii = i & 0xFF;
        let jj = j & 0xFF;
        let gi0 = (self.p(ii + self.p(jj)) % 12) as usize;
        let gi1 = (self.p(ii + i1 + self.p(jj + j1)) % 12) as usize;
        let gi2 = (self.p(ii + 1 + self.p(jj + 1)) % 12) as usize;

        let n0 = corner_noise_3d(gi0, x0, y0, 0.0, 0.5);
        let n1 = corner_noise_3d(gi1, x1, y1, 0.0, 0.5);
        let n2 = corner_noise_3d(gi2, x2, y2, 0.0, 0.5);

        70.0 * (n0 + n1 + n2)
    }

    const F3: f64 = 1.0 / 3.0;
    const G3: f64 = 1.0 / 6.0;

    #[must_use]
    #[expect(
        clippy::many_single_char_names,
        reason = "matches vanilla simplex noise math notation"
    )]
    pub fn get_value_3d(&self, xin: f64, yin: f64, zin: f64) -> f64 {
        let s = (xin + yin + zin) * Self::F3;
        let i = fast_floor(xin + s);
        let j = fast_floor(yin + s);
        let k = fast_floor(zin + s);
        let t = f64::from(i + j + k) * Self::G3;
        let x0 = xin - (f64::from(i) - t);
        let y0 = yin - (f64::from(j) - t);
        let z0 = zin - (f64::from(k) - t);

        let (i1, j1, k1, i2, j2, k2) = if x0 >= y0 {
            if y0 >= z0 {
                (1, 0, 0, 1, 1, 0)
            } else if x0 >= z0 {
                (1, 0, 0, 1, 0, 1)
            } else {
                (0, 0, 1, 1, 0, 1)
            }
        } else if y0 < z0 {
            (0, 0, 1, 0, 1, 1)
        } else if x0 < z0 {
            (0, 1, 0, 0, 1, 1)
        } else {
            (0, 1, 0, 1, 1, 0)
        };

        let x1 = x0 - f64::from(i1) + Self::G3;
        let y1 = y0 - f64::from(j1) + Self::G3;
        let z1 = z0 - f64::from(k1) + Self::G3;
        let x2 = x0 - f64::from(i2) + Self::F3;
        let y2 = y0 - f64::from(j2) + Self::F3;
        let z2 = z0 - f64::from(k2) + Self::F3;
        let x3 = x0 - 1.0 + 0.5;
        let y3 = y0 - 1.0 + 0.5;
        let z3 = z0 - 1.0 + 0.5;

        let ii = i & 0xFF;
        let jj = j & 0xFF;
        let kk = k & 0xFF;
        let gi0 = (self.p(ii + self.p(jj + self.p(kk))) % 12) as usize;
        let gi1 = (self.p(ii + i1 + self.p(jj + j1 + self.p(kk + k1))) % 12) as usize;
        let gi2 = (self.p(ii + i2 + self.p(jj + j2 + self.p(kk + k2))) % 12) as usize;
        let gi3 = (self.p(ii + 1 + self.p(jj + 1 + self.p(kk + 1))) % 12) as usize;

        let n0 = corner_noise_3d(gi0, x0, y0, z0, 0.6);
        let n1 = corner_noise_3d(gi1, x1, y1, z1, 0.6);
        let n2 = corner_noise_3d(gi2, x2, y2, z2, 0.6);
        let n3 = corner_noise_3d(gi3, x3, y3, z3, 0.6);

        32.0 * (n0 + n1 + n2 + n3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::rng::legacy_random::LegacyRandomSource;

    fn rng(seed: u64) -> RandomSource {
        RandomSource::Legacy(LegacyRandomSource::from_seed(seed))
    }

    #[test]
    fn deterministic_for_same_seed() {
        let noise1 = SimplexNoise::new(&mut rng(42));
        let noise2 = SimplexNoise::new(&mut rng(42));

        for i in 0..10 {
            let x = f64::from(i) * 13.7;
            let z = f64::from(i) * 7.3;
            assert!((noise1.get_value_2d(x, z) - noise2.get_value_2d(x, z)).abs() < 1e-15);
        }
    }

    #[test]
    fn spatial_variation() {
        let noise = SimplexNoise::new(&mut rng(0));
        let values: Vec<f64> = (0..20)
            .map(|i| noise.get_value_2d(f64::from(i) * 50.0, f64::from(i) * 30.0))
            .collect();
        let min = values.iter().copied().fold(f64::INFINITY, f64::min);
        let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        assert!(max - min > 0.01);
    }

    #[test]
    fn end_islands_init_produces_finite_value() {
        let mut random = rng(0);
        random.consume_count(17292);
        let noise = SimplexNoise::new(&mut random);
        let v = noise.get_value_2d(10.0, 10.0);
        assert!(v.is_finite());
        assert!(v.abs() > 1e-10);
    }
}
