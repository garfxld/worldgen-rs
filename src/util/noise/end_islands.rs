use crate::util::rng::RandomSource;
use crate::util::rng::legacy_random::LegacyRandomSource;

use super::SimplexNoise;

const ISLAND_THRESHOLD: f32 = -0.9;

#[derive(Debug, Clone)]
pub struct EndIslands {
    island_noise: SimplexNoise,
}

impl EndIslands {
    pub fn new(seed: u64) -> Self {
        let mut island_random = RandomSource::Legacy(LegacyRandomSource::from_seed(seed));
        island_random.consume_count(17292);
        Self {
            island_noise: SimplexNoise::new(&mut island_random),
        }
    }

    pub fn sample(&self, block_x: f64, _block_y: f64, block_z: f64) -> f64 {
        let block_x = block_x as i32;
        let block_z = block_z as i32;
        let height = Self::height_at(&self.island_noise, block_x / 8, block_z / 8);
        (f64::from(height) - 8.0) / 128.0
    }

    fn height_at(island_noise: &SimplexNoise, section_x: i32, section_z: i32) -> f32 {
        let chunk_x = section_x / 2;
        let chunk_z = section_z / 2;
        let sub_x = section_x % 2;
        let sub_z = section_z % 2;

        let dist_sq = section_x
            .wrapping_mul(section_x)
            .wrapping_add(section_z.wrapping_mul(section_z));
        let base = 100.0_f32 - (dist_sq as f32).sqrt() * 8.0;
        let mut doffs = base.clamp(-100.0, 80.0);

        for dx in -12..=12 {
            for dz in -12..=12 {
                let island_x = i64::from(chunk_x) + i64::from(dx);
                let island_z = i64::from(chunk_z) + i64::from(dz);
                if island_x * island_x + island_z * island_z <= 4096
                    || island_noise.get_value_2d(island_x as f64, island_z as f64)
                        >= f64::from(ISLAND_THRESHOLD)
                {
                    continue;
                }

                let island_size =
                    ((island_x as f32).abs() * 3439.0 + (island_z as f32).abs() * 147.0) % 13.0
                        + 9.0;
                let dxf = sub_x as f32 - (dx * 2) as f32;
                let dzf = sub_z as f32 - (dz * 2) as f32;
                let candidate =
                    (100.0_f32 - (dxf * dxf + dzf * dzf).sqrt() * island_size).clamp(-100.0, 80.0);

                if candidate > doffs {
                    doffs = candidate;
                }
            }
        }

        doffs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_for_same_seed() {
        let e1 = EndIslands::new(42);
        let e2 = EndIslands::new(42);
        assert!((e1.sample(0.0, 0.0, 0.0) - e2.sample(0.0, 0.0, 0.0)).abs() < 1e-15);
    }

    #[test]
    fn sample_is_within_expected_range() {
        let e = EndIslands::new(0);
        for x in (-2000..2000).step_by(400) {
            for z in (-2000..2000).step_by(400) {
                let v = e.sample(f64::from(x), 0.0, f64::from(z));
                assert!(
                    (-0.84375..=0.5625).contains(&v),
                    "value {v} out of range at ({x}, {z})"
                );
            }
        }
    }
}
