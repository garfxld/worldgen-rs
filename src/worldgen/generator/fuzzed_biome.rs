use glam::{DVec3, IVec3};

use crate::util::pos::BlockPos;
use crate::worldgen::surface::SurfaceBiomeProvider;

#[inline]
const fn lcg_next(mut rval: i64, c: i64) -> i64 {
    rval = rval.wrapping_mul(
        rval.wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407),
    );
    rval.wrapping_add(c)
}

#[inline]
fn get_fiddle(rval: i64) -> f64 {
    let uniform = ((rval >> 24).rem_euclid(1024)) as f64 / 1024.0;
    (uniform - 0.5) * 0.9
}

pub struct FuzzedBiomeColumn<'a> {
    biome_data: &'a [u16],
    section_count: usize,
    biome_zoom_seed: i64,
    parent_x: i32,
    parent_z: i32,
    fract_x: f64,
    fract_z: f64,
    min_y: i32,
    chunk_quart_x: i32,
    chunk_quart_z: i32,
    neighbor_biomes: &'a dyn Fn(IVec3) -> u16,
    cached_parent_y: i32,
    candidates: [(f64, f64); 8],
    rval_after_cx: [i64; 2],
}

impl<'a> FuzzedBiomeColumn<'a> {
    #[expect(
        clippy::too_many_arguments,
        reason = "matches vanilla BiomeManager.getBiome signature"
    )]
    pub fn new(
        biome_data: &'a [u16],
        section_count: usize,
        biome_zoom_seed: i64,
        block_x: i32,
        block_z: i32,
        min_y: i32,
        chunk_quart_x: i32,
        chunk_quart_z: i32,
        neighbor_biomes: &'a dyn Fn(IVec3) -> u16,
    ) -> Self {
        let abs_x = block_x - 2;
        let abs_z = block_z - 2;
        let parent_x = abs_x >> 2;
        let parent_z = abs_z >> 2;
        Self {
            biome_data,
            section_count,
            biome_zoom_seed,
            parent_x,
            parent_z,
            fract_x: f64::from(abs_x & 3) / 4.0,
            fract_z: f64::from(abs_z & 3) / 4.0,
            min_y,
            chunk_quart_x,
            chunk_quart_z,
            neighbor_biomes,
            cached_parent_y: i32::MIN,
            candidates: [(0.0, 0.0); 8],
            rval_after_cx: [
                lcg_next(biome_zoom_seed, i64::from(parent_x)),
                lcg_next(biome_zoom_seed, i64::from(parent_x + 1)),
            ],
        }
    }

    #[inline]
    fn compute_cy_group(&mut self, cy: i32, high: bool) {
        let base_idx = if high { 2 } else { 0 };
        for cx_idx in 0..2usize {
            let cx = self.parent_x + cx_idx as i32;
            let dx = if cx_idx == 0 {
                self.fract_x
            } else {
                self.fract_x - 1.0
            };
            let rval_cy = lcg_next(self.rval_after_cx[cx_idx], i64::from(cy));
            for cz_off in 0..2usize {
                let cz = self.parent_z + cz_off as i32;
                let dz = if cz_off == 0 {
                    self.fract_z
                } else {
                    self.fract_z - 1.0
                };

                let mut rval = lcg_next(rval_cy, i64::from(cz));
                rval = lcg_next(rval, i64::from(cx));
                rval = lcg_next(rval, i64::from(cy));
                rval = lcg_next(rval, i64::from(cz));
                let fx = get_fiddle(rval);
                rval = lcg_next(rval, self.biome_zoom_seed);
                let fy = get_fiddle(rval);
                rval = lcg_next(rval, self.biome_zoom_seed);
                let fz = get_fiddle(rval);

                let xz_partial = (dx + fx) * (dx + fx) + (dz + fz) * (dz + fz);
                self.candidates[cx_idx * 4 + base_idx + cz_off] = (fy, xz_partial);
            }
        }
    }

    fn recompute_candidates(&mut self, parent_y: i32) {
        if self.cached_parent_y != i32::MIN && parent_y == self.cached_parent_y - 1 {
            self.candidates[2] = self.candidates[0];
            self.candidates[3] = self.candidates[1];
            self.candidates[6] = self.candidates[4];
            self.candidates[7] = self.candidates[5];
            self.compute_cy_group(parent_y, false);
        } else {
            self.compute_cy_group(parent_y, false);
            self.compute_cy_group(parent_y + 1, true);
        }
        self.cached_parent_y = parent_y;
    }

    #[inline]
    pub fn get(&mut self, block_y: i32) -> u16 {
        let abs_y = block_y - 2;
        let parent_y = abs_y >> 2;
        let fract_y = f64::from(abs_y & 3) / 4.0;

        if parent_y != self.cached_parent_y {
            self.recompute_candidates(parent_y);
        }

        let mut min_i = 0usize;
        let mut min_dist = f64::INFINITY;
        for i in 0..8usize {
            let (fy, xz_partial) = self.candidates[i];
            let dy = if (i & 2) == 0 { fract_y } else { fract_y - 1.0 };
            let dist = xz_partial + (dy + fy) * (dy + fy);
            if min_dist > dist {
                min_i = i;
                min_dist = dist;
            }
        }

        let biome_quart = IVec3::new(
            if (min_i & 4) == 0 {
                self.parent_x
            } else {
                self.parent_x + 1
            },
            if (min_i & 2) == 0 {
                parent_y
            } else {
                parent_y + 1
            },
            if (min_i & 1) == 0 {
                self.parent_z
            } else {
                self.parent_z + 1
            },
        );

        let in_chunk = biome_quart.x >= self.chunk_quart_x
            && biome_quart.x < self.chunk_quart_x + 4
            && biome_quart.z >= self.chunk_quart_z
            && biome_quart.z < self.chunk_quart_z + 4;

        if in_chunk {
            let min_qy = self.min_y >> 2;
            let total_quarts_y = self.section_count * 4;
            let local_qx = (biome_quart.x - self.chunk_quart_x) as usize;
            let local_qz = (biome_quart.z - self.chunk_quart_z) as usize;
            let qy_in_chunk = (biome_quart.y - min_qy).clamp(0, total_quarts_y as i32 - 1) as usize;
            let section_idx = qy_in_chunk / 4;
            let local_qy = qy_in_chunk % 4;
            self.biome_data[section_idx * 64 + local_qy * 16 + local_qz * 4 + local_qx]
        } else {
            (self.neighbor_biomes)(biome_quart)
        }
    }
}

impl SurfaceBiomeProvider for FuzzedBiomeColumn<'_> {
    #[inline]
    fn biome_id(&mut self, block_y: i32) -> u16 {
        self.get(block_y)
    }
}

pub fn fuzzed_biome_at_block<F: FnMut(IVec3) -> u16>(
    biome_zoom_seed: i64,
    pos: BlockPos,
    mut quart_biome: F,
) -> u16 {
    let abs = pos.0 - IVec3::splat(2);
    let parent = IVec3::new(abs.x >> 2, abs.y >> 2, abs.z >> 2);
    let fract = DVec3::new(
        f64::from(abs.x & 3),
        f64::from(abs.y & 3),
        f64::from(abs.z & 3),
    ) / 4.0;

    let mut min_i = 0usize;
    let mut min_dist = f64::INFINITY;

    for i in 0..8usize {
        let x_even = (i & 4) == 0;
        let y_even = (i & 2) == 0;
        let z_even = (i & 1) == 0;
        let cx = if x_even { parent.x } else { parent.x + 1 };
        let cy = if y_even { parent.y } else { parent.y + 1 };
        let cz = if z_even { parent.z } else { parent.z + 1 };
        let dx = if x_even { fract.x } else { fract.x - 1.0 };
        let dy = if y_even { fract.y } else { fract.y - 1.0 };
        let dz = if z_even { fract.z } else { fract.z - 1.0 };

        let mut rval = lcg_next(biome_zoom_seed, i64::from(cx));
        rval = lcg_next(rval, i64::from(cy));
        rval = lcg_next(rval, i64::from(cz));
        rval = lcg_next(rval, i64::from(cx));
        rval = lcg_next(rval, i64::from(cy));
        rval = lcg_next(rval, i64::from(cz));
        let fx = get_fiddle(rval);
        rval = lcg_next(rval, biome_zoom_seed);
        let fy = get_fiddle(rval);
        rval = lcg_next(rval, biome_zoom_seed);
        let fz = get_fiddle(rval);

        let dist = (dx + fx).powi(2) + (dy + fy).powi(2) + (dz + fz).powi(2);
        if min_dist > dist {
            min_i = i;
            min_dist = dist;
        }
    }

    let b = IVec3::new(
        if (min_i & 4) == 0 {
            parent.x
        } else {
            parent.x + 1
        },
        if (min_i & 2) == 0 {
            parent.y
        } else {
            parent.y + 1
        },
        if (min_i & 1) == 0 {
            parent.z
        } else {
            parent.z + 1
        },
    );
    quart_biome(b)
}
