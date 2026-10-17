use crate::util::interval::Interval;
use crate::util::rng::RandomSource;

use super::gradient_noise::{GRADIENT, GradientNoise, floor};

pub const STANDARD_DEVIATION: f64 = 0.42544;
const SQRT_3: f64 = 1.732_050_807_568_877_2;
const F2: f64 = 0.5 * (SQRT_3 - 1.0);
const G2: f64 = (3.0 - SQRT_3) / 6.0;

#[derive(Debug, Clone)]
pub struct SimplexNoise {
    pub base: GradientNoise,
}

fn corner_noise_3d(index: i32, x: f64, y: f64, z: f64, base: f64) -> f64 {
    let mut t0 = base - x * x - y * y - z * z;
    if t0 < 0.0 {
        0.0
    } else {
        t0 *= t0;
        t0 * t0 * GRADIENT[index as usize].dot_f64(x, y, z)
    }
}

impl SimplexNoise {
    pub fn new(random: &mut RandomSource) -> Self {
        Self {
            base: GradientNoise::new(random, 256.0),
        }
    }

    pub fn new_discarding_offset(random: &mut RandomSource, discard_noise_offset: bool) -> Self {
        Self {
            base: GradientNoise::new(random, if discard_noise_offset { 0.0 } else { 256.0 }),
        }
    }

    pub fn range() -> Interval {
        Interval::of_symmetric(2.0)
    }

    pub fn get2(&self, x_in: f64, y_in: f64) -> f32 {
        let p = &self.base;
        let xin = x_in + p.offset_x;
        let yin = y_in + p.offset_y;
        let s = (xin + yin) * F2;
        let i = floor(xin + s);
        let j = floor(yin + s);
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
        let gi0 = p.permute(ii + p.permute(jj)) % 12;
        let gi1 = p.permute(ii + i1 + p.permute(jj + j1)) % 12;
        let gi2 = p.permute(ii + 1 + p.permute(jj + 1)) % 12;
        let n0 = corner_noise_3d(gi0, x0, y0, 0.0, 0.5);
        let n1 = corner_noise_3d(gi1, x1, y1, 0.0, 0.5);
        let n2 = corner_noise_3d(gi2, x2, y2, 0.0, 0.5);
        (70.0 * (n0 + n1 + n2)) as f32
    }

    pub fn get(&self, x_in: f64, y_in: f64, z_in: f64) -> f32 {
        let p = &self.base;
        let xin = x_in + p.offset_x;
        let yin = y_in + p.offset_y;
        let zin = z_in + p.offset_z;
        let s = (xin + yin + zin) * 0.333_333_333_333_333_3;
        let i = floor(xin + s);
        let j = floor(yin + s);
        let k = floor(zin + s);
        let t = f64::from(i + j + k) * 0.166_666_666_666_666_66;
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
        let x1 = x0 - f64::from(i1) + 0.166_666_666_666_666_66;
        let y1 = y0 - f64::from(j1) + 0.166_666_666_666_666_66;
        let z1 = z0 - f64::from(k1) + 0.166_666_666_666_666_66;
        let x2 = x0 - f64::from(i2) + 0.333_333_333_333_333_3;
        let y2 = y0 - f64::from(j2) + 0.333_333_333_333_333_3;
        let z2 = z0 - f64::from(k2) + 0.333_333_333_333_333_3;
        let x3 = x0 - 1.0 + 0.5;
        let y3 = y0 - 1.0 + 0.5;
        let z3 = z0 - 1.0 + 0.5;
        let ii = i & 0xFF;
        let jj = j & 0xFF;
        let kk = k & 0xFF;
        let gi0 = p.permute(ii + p.permute(jj + p.permute(kk))) % 12;
        let gi1 = p.permute(ii + i1 + p.permute(jj + j1 + p.permute(kk + k1))) % 12;
        let gi2 = p.permute(ii + i2 + p.permute(jj + j2 + p.permute(kk + k2))) % 12;
        let gi3 = p.permute(ii + 1 + p.permute(jj + 1 + p.permute(kk + 1))) % 12;
        let n0 = corner_noise_3d(gi0, x0, y0, z0, 0.6);
        let n1 = corner_noise_3d(gi1, x1, y1, z1, 0.6);
        let n2 = corner_noise_3d(gi2, x2, y2, z2, 0.6);
        let n3 = corner_noise_3d(gi3, x3, y3, z3, 0.6);
        (32.0 * (n0 + n1 + n2 + n3)) as f32
    }
}
