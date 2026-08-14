use crate::util::math::{corner_noise_3d, fast_floor};
use crate::util::rng::RandomSource;

const SQRT_3: f64 = 1.7320508075688772;
const F2: f64 = 0.5 * (SQRT_3 - 1.0);
const G2: f64 = (3.0 - SQRT_3) / 6.0;
const F3: f64 = 1.0 / 3.0;
const G3: f64 = 1.0 / 6.0;

#[derive(Debug, Clone)]
pub struct SimplexNoise {
    permutation: [i32; 512],
    pub xo: f64,
    pub yo: f64,
    pub zo: f64,
}

impl SimplexNoise {
    pub fn new(random: &mut RandomSource) -> Self {
        let xo = random.next_f64() * 256.0;
        let yo = random.next_f64() * 256.0;
        let zo = random.next_f64() * 256.0;

        let mut permutation = [0i32; 512];
        for (i, slot) in permutation.iter_mut().take(256).enumerate() {
            *slot = i as i32;
        }

        for i in 0..256 {
            let swap_with = i + random.next_i32_bounded((256 - i) as i32) as usize;
            permutation.swap(i, swap_with);
        }
        for i in 0..256 {
            permutation[i + 256] = permutation[i];
        }

        Self {
            permutation,
            xo,
            yo,
            zo,
        }
    }

    #[inline]
    const fn permutation_at(&self, index: i32) -> i32 {
        self.permutation[(index & 0xFF) as usize]
    }

    pub fn get_value_2d(&self, xin: f64, yin: f64) -> f64 {
        let skew = (xin + yin) * F2;
        let i = fast_floor(xin + skew);
        let j = fast_floor(yin + skew);
        let unskew = f64::from(i + j) * G2;
        let x0 = xin - (f64::from(i) - unskew);
        let y0 = yin - (f64::from(j) - unskew);

        let (i1, j1) = if x0 > y0 { (1, 0) } else { (0, 1) };

        let x1 = x0 - f64::from(i1) + G2;
        let y1 = y0 - f64::from(j1) + G2;
        let x2 = x0 - 1.0 + 2.0 * G2;
        let y2 = y0 - 1.0 + 2.0 * G2;

        let ii = i & 0xFF;
        let jj = j & 0xFF;
        let gi0 = (self.permutation_at(ii + self.permutation_at(jj)) % 12) as usize;
        let gi1 = (self.permutation_at(ii + i1 + self.permutation_at(jj + j1)) % 12) as usize;
        let gi2 = (self.permutation_at(ii + 1 + self.permutation_at(jj + 1)) % 12) as usize;

        let corner0 = corner_noise_3d(gi0, x0, y0, 0.0, 0.5);
        let corner1 = corner_noise_3d(gi1, x1, y1, 0.0, 0.5);
        let corner2 = corner_noise_3d(gi2, x2, y2, 0.0, 0.5);

        70.0 * (corner0 + corner1 + corner2)
    }

    pub fn get_value_3d(&self, xin: f64, yin: f64, zin: f64) -> f64 {
        let skew = (xin + yin + zin) * F3;
        let i = fast_floor(xin + skew);
        let j = fast_floor(yin + skew);
        let k = fast_floor(zin + skew);
        let unskew = f64::from(i + j + k) * G3;
        let x0 = xin - (f64::from(i) - unskew);
        let y0 = yin - (f64::from(j) - unskew);
        let z0 = zin - (f64::from(k) - unskew);

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

        let x1 = x0 - f64::from(i1) + G3;
        let y1 = y0 - f64::from(j1) + G3;
        let z1 = z0 - f64::from(k1) + G3;
        let x2 = x0 - f64::from(i2) + F3;
        let y2 = y0 - f64::from(j2) + F3;
        let z2 = z0 - f64::from(k2) + F3;
        let x3 = x0 - 1.0 + 0.5;
        let y3 = y0 - 1.0 + 0.5;
        let z3 = z0 - 1.0 + 0.5;

        let ii = i & 0xFF;
        let jj = j & 0xFF;
        let kk = k & 0xFF;
        let gi0 = (self.permutation_at(ii + self.permutation_at(jj + self.permutation_at(kk))) % 12)
            as usize;
        let gi1 = (self
            .permutation_at(ii + i1 + self.permutation_at(jj + j1 + self.permutation_at(kk + k1)))
            % 12) as usize;
        let gi2 = (self
            .permutation_at(ii + i2 + self.permutation_at(jj + j2 + self.permutation_at(kk + k2)))
            % 12) as usize;
        let gi3 = (self
            .permutation_at(ii + 1 + self.permutation_at(jj + 1 + self.permutation_at(kk + 1)))
            % 12) as usize;

        let corner0 = corner_noise_3d(gi0, x0, y0, z0, 0.6);
        let corner1 = corner_noise_3d(gi1, x1, y1, z1, 0.6);
        let corner2 = corner_noise_3d(gi2, x2, y2, z2, 0.6);
        let corner3 = corner_noise_3d(gi3, x3, y3, z3, 0.6);

        32.0 * (corner0 + corner1 + corner2 + corner3)
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
