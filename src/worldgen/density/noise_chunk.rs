use std::simd::f64x4;

use crate::util::math::lerp;
use crate::worldgen::density::{Beardifier, ColumnCache, Noises};

const MAX_INTERP: usize = 16;
const MAX_SLICE_LEN: usize = 1024;

pub struct NoiseChunk {
    slices: Vec<Box<[f64; MAX_INTERP * MAX_SLICE_LEN]>>,
    interp_count: usize,
    corners_y: usize,
    block_ys: Vec<i32>,
    first_cell_x: i32,
    first_cell_z: i32,
    cell_min_y: i32,
    cell_count_y: usize,
    cell_count_xz: usize,
}

impl NoiseChunk {
    #[must_use]
    #[expect(
        clippy::missing_panics_doc,
        reason = "panic is a compile-time constant check"
    )]
    pub fn new(chunk_min_block_x: i32, chunk_min_block_z: i32, noises: &Noises) -> Self {
        let cell_width = noises.cell_width();
        let cell_height = noises.cell_height();
        let min_y = noises.min_y();
        let height = noises.height();

        let first_cell_x = chunk_min_block_x.div_euclid(cell_width);
        let first_cell_z = chunk_min_block_z.div_euclid(cell_width);
        let cell_min_y = min_y.div_euclid(cell_height);

        let cell_count_xz = (16 / cell_width) as usize;
        let cell_count_y = (height / cell_height) as usize;
        let corners_y = cell_count_y + 1;
        let z_corners = cell_count_xz + 1;
        let slice_len = z_corners * corners_y;

        let interp_count = noises.compiled().interpolated_count();
        assert!(
            slice_len <= MAX_SLICE_LEN,
            "slice_len {slice_len} exceeds MAX_SLICE_LEN {MAX_SLICE_LEN}"
        );
        assert!(
            interp_count <= MAX_INTERP,
            "interp_count {interp_count} exceeds MAX_INTERP {MAX_INTERP}"
        );

        let block_ys: Vec<i32> = (0..corners_y)
            .map(|cy| (cy as i32 + cell_min_y) * cell_height)
            .collect();

        let n_slices = cell_count_xz + 1;
        let mut slices = Vec::with_capacity(n_slices);
        for _ in 0..n_slices {
            #[expect(
                clippy::large_stack_arrays,
                reason = "fixed-size boxed array keeps the [f64; N] type the fill path relies on; cold per-chunk constructor"
            )]
            slices.push(Box::new([0.0; MAX_INTERP * MAX_SLICE_LEN]));
        }

        Self {
            slices,
            interp_count,
            corners_y,
            block_ys,
            first_cell_x,
            first_cell_z,
            cell_min_y,
            cell_count_y,
            cell_count_xz,
        }
    }

    fn blended_column(noises: &Noises, x: i32, block_ys: &[i32], z: i32, out: &mut [f64]) {
        match noises.blended() {
            Some(blended) => {
                for (out_v, &y) in out.iter_mut().zip(block_ys) {
                    *out_v = blended.compute(f64::from(x), f64::from(y), f64::from(z));
                }
            }
            None => out.fill(0.0),
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "slice filling needs the precomputed geometry and per-thread cache"
    )]
    fn fill_slice_into(
        slice: &mut [f64; MAX_INTERP * MAX_SLICE_LEN],
        cell_x: i32,
        block_ys: &[i32],
        blended_column: &mut [f64],
        interp_count: usize,
        corners_y: usize,
        cell_count_xz: usize,
        first_cell_z: i32,
        noises: &Noises,
        cache: &mut ColumnCache,
    ) {
        let cell_width = noises.cell_width();
        let block_x = cell_x * cell_width;
        let fill = noises.compiled().fill();
        let fill_4x = noises.compiled().fill_4x();

        let mut values = [0.0f64; MAX_INTERP];
        let mut values_4x = [0.0f64; 4 * MAX_INTERP];

        for cz in 0..=cell_count_xz {
            let cell_z = first_cell_z + cz as i32;
            let block_z = cell_z * cell_width;

            cache.ensure(block_x, block_z, noises);
            Self::blended_column(noises, block_x, block_ys, block_z, blended_column);
            let cache_ptr = cache.ptr(block_x, block_z, noises);

            let mut cy = 0;
            while cy + 4 <= corners_y {
                let ys = [
                    f64::from(block_ys[cy]),
                    f64::from(block_ys[cy + 1]),
                    f64::from(block_ys[cy + 2]),
                    f64::from(block_ys[cy + 3]),
                ];
                let blended = [
                    blended_column[cy],
                    blended_column[cy + 1],
                    blended_column[cy + 2],
                    blended_column[cy + 3],
                ];

                // SAFETY: `fill_4x` was compiled with `Shape::Fill4x`'s signature,
                // matching `(cache, x, ys_ptr[4], z, blended_ptr[4], out_ptr[4*interp_count])`.
                unsafe {
                    fill_4x(
                        cache_ptr,
                        f64::from(block_x),
                        ys.as_ptr(),
                        f64::from(block_z),
                        blended.as_ptr(),
                        values_4x[..4 * interp_count].as_mut_ptr(),
                    );
                }

                for lane in 0..4 {
                    let lane_cy = cy + lane;
                    let src = &values_4x[lane * interp_count..(lane + 1) * interp_count];
                    let corner_idx = cz * corners_y + lane_cy;
                    let base = corner_idx * MAX_INTERP;
                    slice[base..base + interp_count].copy_from_slice(src);
                }

                cy += 4;
            }

            while cy < corners_y {
                let block_y = block_ys[cy];

                // SAFETY: `fill` was compiled with `Shape::Fill`'s signature,
                // matching `(cache, x, y, z, blended, out_ptr[interp_count])`.
                unsafe {
                    fill(
                        cache_ptr,
                        f64::from(block_x),
                        f64::from(block_y),
                        f64::from(block_z),
                        blended_column[cy],
                        values[..interp_count].as_mut_ptr(),
                    );
                }

                let corner_idx = cz * corners_y + cy;
                let base = corner_idx * MAX_INTERP;
                slice[base..base + interp_count].copy_from_slice(&values[..interp_count]);

                cy += 1;
            }
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "single trilinear-interpolation kernel; splitting the loop nest would scatter the per-corner SAFETY invariants"
    )]
    #[expect(
        clippy::similar_names,
        reason = "factor_{x,y,z}_v vector splats deliberately mirror their scalar factor_{x,y,z} sources"
    )]
    pub fn fill<F>(
        &mut self,
        noises: &Noises,
        cache: &mut ColumnCache,
        beardifier: Option<&Beardifier>,
        mut place_block: F,
    ) where
        F: FnMut(usize, i32, usize, f64, &[f64], &mut ColumnCache),
    {
        let cell_width = noises.cell_width();
        let cell_height = noises.cell_height();
        let cell_count_xz = self.cell_count_xz;
        let cell_count_y = self.cell_count_y;
        let interp_count = self.interp_count;
        let corners_y = self.corners_y;
        let first_cell_x = self.first_cell_x;
        let first_cell_z = self.first_cell_z;
        let block_ys: &[i32] = &self.block_ys;
        let combine_final = noises.compiled().combine_final();

        let n_slices = cell_count_xz + 1;
        let mut local_blended = vec![0.0f64; corners_y];
        for cx_off in 0..n_slices {
            let cell_x = first_cell_x + cx_off as i32;
            Self::fill_slice_into(
                &mut self.slices[cx_off],
                cell_x,
                block_ys,
                &mut local_blended,
                interp_count,
                corners_y,
                cell_count_xz,
                first_cell_z,
                noises,
                cache,
            );
        }

        let zero_cache = vec![0.0f64; noises.compiled().slots()];
        let mut interpolated = [0.0f64; MAX_INTERP];

        for cell_x_idx in 0..cell_count_xz {
            for cell_z_idx in 0..cell_count_xz {
                for x_in_cell in 0..cell_width {
                    let factor_x = f64::from(x_in_cell) / f64::from(cell_width);
                    let local_x = (cell_x_idx as i32 * cell_width + x_in_cell) as usize;

                    for z_in_cell in 0..cell_width {
                        let factor_z = f64::from(z_in_cell) / f64::from(cell_width);
                        let local_z = (cell_z_idx as i32 * cell_width + z_in_cell) as usize;

                        let z0_base = cell_z_idx * corners_y;
                        let z1_base = (cell_z_idx + 1) * corners_y;

                        for cell_y_idx in (0..cell_count_y).rev() {
                            for y_in_cell in (0..cell_height).rev() {
                                let factor_y = f64::from(y_in_cell) / f64::from(cell_height);

                                let world_y =
                                    (self.cell_min_y + cell_y_idx as i32) * cell_height + y_in_cell;

                                let i0_base = (z0_base + cell_y_idx) * MAX_INTERP;
                                let i1_base = (z1_base + cell_y_idx) * MAX_INTERP;
                                let i0_next = i0_base + MAX_INTERP;
                                let i1_next = i1_base + MAX_INTERP;
                                let s0 = &*self.slices[cell_x_idx];
                                let s1 = &*self.slices[cell_x_idx + 1];
                                let factor_y_v = f64x4::splat(factor_y);
                                let factor_x_v = f64x4::splat(factor_x);
                                let factor_z_v = f64x4::splat(factor_z);

                                let mut ch_batch = 0;
                                while ch_batch + 4 <= interp_count {
                                    // SAFETY: ch_batch+3 < interp_count ≤ MAX_INTERP, all base indices in bounds.
                                    unsafe {
                                        let n000 = f64x4::from_slice(s0.get_unchecked(
                                            i0_base + ch_batch..i0_base + ch_batch + 4,
                                        ));
                                        let n001 = f64x4::from_slice(s0.get_unchecked(
                                            i1_base + ch_batch..i1_base + ch_batch + 4,
                                        ));
                                        let n100 = f64x4::from_slice(s1.get_unchecked(
                                            i0_base + ch_batch..i0_base + ch_batch + 4,
                                        ));
                                        let n101 = f64x4::from_slice(s1.get_unchecked(
                                            i1_base + ch_batch..i1_base + ch_batch + 4,
                                        ));
                                        let n010 = f64x4::from_slice(s0.get_unchecked(
                                            i0_next + ch_batch..i0_next + ch_batch + 4,
                                        ));
                                        let n011 = f64x4::from_slice(s0.get_unchecked(
                                            i1_next + ch_batch..i1_next + ch_batch + 4,
                                        ));
                                        let n110 = f64x4::from_slice(s1.get_unchecked(
                                            i0_next + ch_batch..i0_next + ch_batch + 4,
                                        ));
                                        let n111 = f64x4::from_slice(s1.get_unchecked(
                                            i1_next + ch_batch..i1_next + ch_batch + 4,
                                        ));

                                        let d00 = n000 + factor_y_v * (n010 - n000);
                                        let d10 = n100 + factor_y_v * (n110 - n100);
                                        let d01 = n001 + factor_y_v * (n011 - n001);
                                        let d11 = n101 + factor_y_v * (n111 - n101);
                                        let d0 = d00 + factor_x_v * (d10 - d00);
                                        let d1 = d01 + factor_x_v * (d11 - d01);
                                        let result = d0 + factor_z_v * (d1 - d0);
                                        let arr = result.to_array();
                                        let dst =
                                            interpolated.get_unchecked_mut(ch_batch..ch_batch + 4);
                                        dst.copy_from_slice(&arr);
                                    }
                                    ch_batch += 4;
                                }
                                while ch_batch < interp_count {
                                    let ch = ch_batch;
                                    // SAFETY: ch < interp_count ≤ MAX_INTERP; indices in bounds (see comment above).
                                    unsafe {
                                        let n000 = *s0.get_unchecked(i0_base + ch);
                                        let n001 = *s0.get_unchecked(i1_base + ch);
                                        let n100 = *s1.get_unchecked(i0_base + ch);
                                        let n101 = *s1.get_unchecked(i1_base + ch);
                                        let n010 = *s0.get_unchecked(i0_next + ch);
                                        let n011 = *s0.get_unchecked(i1_next + ch);
                                        let n110 = *s1.get_unchecked(i0_next + ch);
                                        let n111 = *s1.get_unchecked(i1_next + ch);

                                        let d00 = lerp(factor_y, n000, n010);
                                        let d10 = lerp(factor_y, n100, n110);
                                        let d01 = lerp(factor_y, n001, n011);
                                        let d11 = lerp(factor_y, n101, n111);
                                        let d0 = lerp(factor_x, d00, d10);
                                        let d1 = lerp(factor_x, d01, d11);
                                        *interpolated.get_unchecked_mut(ch) =
                                            lerp(factor_z, d0, d1);
                                    }
                                    ch_batch += 1;
                                }

                                // SAFETY: `combine_final` was compiled with `Shape::Combine`'s
                                // signature, matching `(cache, interpolated_ptr, x, y, z) -> f64`.
                                let mut density = unsafe {
                                    combine_final(
                                        zero_cache.as_ptr(),
                                        interpolated[..interp_count].as_ptr(),
                                        0.0,
                                        f64::from(world_y),
                                        0.0,
                                    )
                                };

                                let world_x = cell_x_idx as i32 * cell_width
                                    + x_in_cell
                                    + self.first_cell_x * cell_width;
                                let world_z = cell_z_idx as i32 * cell_width
                                    + z_in_cell
                                    + self.first_cell_z * cell_width;
                                if let Some(beard) = beardifier {
                                    density += beard.compute(world_x, world_y, world_z);
                                }

                                place_block(
                                    local_x,
                                    world_y,
                                    local_z,
                                    density,
                                    &interpolated[..interp_count],
                                    cache,
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test loads the checked-in vanilla datapack"
)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::worldgen::backend::ir::loader::DatapackSource;

    fn datapack_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/datapack/worldgen")
    }

    #[test]
    fn fills_an_overworld_chunk_with_finite_densities() {
        let source = DatapackSource::new(datapack_root());
        let registry = source.density_function_registry().unwrap();
        let (program, settings) = source.program("overworld", "Overworld", &registry).unwrap();
        let params = source.noise_parameters().unwrap();
        let noises = Noises::new(&program, &settings, &params, 42).unwrap();

        let mut chunk = NoiseChunk::new(0, 0, &noises);
        let mut cache = ColumnCache::new(&noises);
        cache.init_grid(0, 0, &noises);

        let mut deep_density = None;
        let mut sky_density = None;
        chunk.fill(
            &noises,
            &mut cache,
            None,
            |x, y, z, density, _interp, _cache| {
                assert!(density.is_finite(), "non-finite density at ({x}, {y}, {z})");
                if x == 0 && z == 0 && y == noises.min_y() + 4 {
                    deep_density = Some(density);
                }
                if x == 0 && z == 0 && y == noises.height() + noises.min_y() - 4 {
                    sky_density = Some(density);
                }
            },
        );

        assert!(
            deep_density.unwrap() > 0.0,
            "expected solid density near bedrock"
        );
        assert!(
            sky_density.unwrap() < 0.0,
            "expected air density high in the sky"
        );
    }
}
