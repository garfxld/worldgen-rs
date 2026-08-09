use crate::util::rng::RandomSource;
use crate::util::rng::legacy_random::LegacyRandomSource;

use super::SimplexNoise;

/// Vanilla uses `-0.9F` (float literal) in a `double < float` comparison, which
/// promotes the float to double. `(double)(-0.9f)` is not the exact double
/// `-0.9`, so the f32->f64 promotion must be matched exactly.
const ISLAND_THRESHOLD: f64 = -0.9_f32 as f64;

#[derive(Debug, Clone)]
pub struct EndIslands {
    island_noise: SimplexNoise,
}

impl EndIslands {
    #[must_use]
    pub fn new(seed: u64) -> Self {
        let mut rng = RandomSource::Legacy(LegacyRandomSource::from_seed(seed));
        rng.consume_count(17292);
        let island_noise = SimplexNoise::new(&mut rng);
        Self { island_noise }
    }

    #[must_use]
    pub fn sample(&self, block_x: f64, _block_y: f64, block_z: f64) -> f64 {
        let block_x = block_x as i32;
        let block_z = block_z as i32;
        (f64::from(Self::get_height_value(
            &self.island_noise,
            block_x / 8,
            block_z / 8,
        )) - 8.0)
            / 128.0
    }

    fn get_height_value(island_noise: &SimplexNoise, section_x: i32, section_z: i32) -> f32 {
        let chunk_x = section_x / 2;
        let chunk_z = section_z / 2;
        let sub_section_x = section_x % 2;
        let sub_section_z = section_z % 2;

        let dist_sq = section_x
            .wrapping_mul(section_x)
            .wrapping_add(section_z.wrapping_mul(section_z));
        let dist = (dist_sq as f32).sqrt();
        let mut doffs = (100.0_f32 - dist * 8.0).clamp(-100.0, 80.0);

        for xo in -12..=12 {
            for zo in -12..=12 {
                let total_chunk_x = i64::from(chunk_x) + i64::from(xo);
                let total_chunk_z = i64::from(chunk_z) + i64::from(zo);

                if total_chunk_x * total_chunk_x + total_chunk_z * total_chunk_z > 4096
                    && island_noise.get_value_2d(total_chunk_x as f64, total_chunk_z as f64)
                        < ISLAND_THRESHOLD
                {
                    let island_size = ((total_chunk_x as f32).abs() * 3439.0
                        + (total_chunk_z as f32).abs() * 147.0)
                        % 13.0
                        + 9.0;
                    let xd = sub_section_x as f32 - (xo * 2) as f32;
                    let zd = sub_section_z as f32 - (zo * 2) as f32;
                    let new_doffs =
                        (100.0_f32 - (xd * xd + zd * zd).sqrt() * island_size).clamp(-100.0, 80.0);
                    // Must NOT use f32::max here — Rust's max returns the non-NaN
                    // argument, while Java's Math.max propagates NaN.
                    if new_doffs > doffs {
                        doffs = new_doffs;
                    }
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
