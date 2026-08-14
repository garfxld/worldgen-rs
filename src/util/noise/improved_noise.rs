use std::ops;
use std::simd::Simd;
use std::simd::cmp::{SimdPartialEq, SimdPartialOrd};
#[cfg(target_feature = "avx512f")]
use std::simd::f64x4;
use std::simd::num::{SimdFloat, SimdInt, SimdUint};
use std::simd::ptr::SimdConstPtr;
use std::simd::{Mask, Select, SimdCast, SimdElement, StdFloat};

use crate::util::math::{
    GRADIENT, fast_floor, fast_floor_simd, grad_dot, grad_dot_simd, lerp2, lerp3, lerp3_simd,
    smoothstep, smoothstep_derivative, smoothstep_simd,
};
use crate::util::rng::RandomSource;

#[derive(Debug, Clone)]
pub struct ImprovedNoise {
    p: [u8; 256],
    pub xo: f64,
    pub yo: f64,
    pub zo: f64,
    yo_floor: i32,
    yo_fraction: f64,
    zo_floor: i32,
    zo_fraction: f64,
}

impl ImprovedNoise {
    pub const fn permutations(&self) -> &[u8; 256] {
        &self.p
    }

    pub fn new(random: &mut RandomSource) -> Self {
        let xo = random.next_f64() * 256.0;
        let yo = random.next_f64() * 256.0;
        let zo = random.next_f64() * 256.0;

        let mut p = [0u8; 256];
        #[expect(
            clippy::needless_range_loop,
            reason = "index is used as the initial permutation value"
        )]
        for i in 0..256 {
            p[i] = i as u8;
        }

        for i in 0..256 {
            let offset = random.next_i32_bounded((256 - i) as i32) as usize;
            p.swap(i, i + offset);
        }

        let yo_floor = fast_floor(yo);
        let yo_fraction = yo - f64::from(yo_floor);
        let zo_floor = fast_floor(zo);
        let zo_fraction = zo - f64::from(zo_floor);

        Self {
            p,
            xo,
            yo,
            zo,
            yo_floor,
            yo_fraction,
            zo_floor,
            zo_fraction,
        }
    }

    #[inline]
    pub fn noise(&self, x: f64, y: f64, z: f64) -> f64 {
        let x = x + self.xo;
        let y = y + self.yo;
        let z = z + self.zo;

        let xf = fast_floor(x);
        let yf = fast_floor(y);
        let zf = fast_floor(z);

        let xr = x - f64::from(xf);
        let yr = y - f64::from(yf);
        let zr = z - f64::from(zf);

        self.sample_and_lerp(xf, yf, zf, xr, yr, zr, yr)
    }

    #[inline]
    pub fn noise_simd<F, const N: usize>(
        &self,
        x: Simd<F, N>,
        y: Simd<F, N>,
        z: Simd<F, N>,
    ) -> Simd<F, N>
    where
        F: SimdElement + SimdCast,
        Simd<F, N>: SimdFloat<Cast<i32> = Simd<i32, N>>
            + SimdPartialOrd
            + SimdPartialEq<Mask = Mask<<F as SimdElement>::Mask, N>>
            + ops::Add<Output = Simd<F, N>>
            + ops::Sub<Output = Simd<F, N>>
            + ops::Mul<Output = Simd<F, N>>
            + ops::Neg<Output = Simd<F, N>>,
    {
        let x = x + Simd::splat(self.xo).cast();
        let y = y + Simd::splat(self.yo).cast();
        let z = z + Simd::splat(self.zo).cast();

        let xf = fast_floor_simd::<F, i32, N>(x);
        let yf = fast_floor_simd::<F, i32, N>(y);
        let zf = fast_floor_simd::<F, i32, N>(z);

        let xr = x - xf.cast();
        let yr = y - yf.cast();
        let zr = z - zf.cast();

        self.sample_and_lerp_simd(xf, yf, zf, xr, yr, zr, yr)
    }

    #[inline]
    pub fn noise_xz(&self, x: f64, z: f64) -> f64 {
        let x = x + self.xo;
        let z = z + self.zo;

        let xf = fast_floor(x);
        let zf = fast_floor(z);

        let xr = x - f64::from(xf);
        let zr = z - f64::from(zf);

        self.sample_and_lerp(
            xf,
            self.yo_floor,
            zf,
            xr,
            self.yo_fraction,
            zr,
            self.yo_fraction,
        )
    }

    #[inline]
    pub fn noise_xy(&self, x: f64, y: f64) -> f64 {
        let x = x + self.xo;
        let y = y + self.yo;

        let xf = fast_floor(x);
        let yf = fast_floor(y);

        let xr = x - f64::from(xf);
        let yr = y - f64::from(yf);

        self.sample_and_lerp(xf, yf, self.zo_floor, xr, yr, self.zo_fraction, yr)
    }

    pub fn noise_with_derivative(
        &self,
        x: f64,
        y: f64,
        z: f64,
        derivative_out: &mut [f64; 3],
    ) -> f64 {
        let x = x + self.xo;
        let y = y + self.yo;
        let z = z + self.zo;

        let xf = fast_floor(x);
        let yf = fast_floor(y);
        let zf = fast_floor(z);

        let xr = x - f64::from(xf);
        let yr = y - f64::from(yf);
        let zr = z - f64::from(zf);

        self.sample_with_derivative(xf, yf, zf, xr, yr, zr, derivative_out)
    }

    pub fn noise_with_y_scale(&self, x: f64, y: f64, z: f64, y_scale: f64, y_fudge: f64) -> f64 {
        let x = x + self.xo;
        let y = y + self.yo;
        let z = z + self.zo;

        let xf = fast_floor(x);
        let yf = fast_floor(y);
        let zf = fast_floor(z);

        let xr = x - f64::from(xf);
        let yr = y - f64::from(yf);
        let zr = z - f64::from(zf);

        let yr_fudge = if y_scale != 0.0 {
            let fudge_limit = if y_fudge >= 0.0 && y_fudge < yr {
                y_fudge
            } else {
                yr
            };
            (fudge_limit / y_scale + f64::from(1.0e-7_f32)).floor() * y_scale
        } else {
            0.0
        };
        self.sample_and_lerp(xf, yf, zf, xr, yr - yr_fudge, zr, yr)
    }

    #[expect(clippy::too_many_arguments, reason = "matches vanilla signature")]
    fn sample_and_lerp(
        &self,
        x: i32,
        y: i32,
        z: i32,
        xr: f64,
        yr: f64,
        zr: f64,
        yr_original: f64,
    ) -> f64 {
        let (d000, d100, d010, d110, d001, d101, d011, d111) = {
            #[cfg(target_feature = "avx512f")]
            {
                let x = x as u8;
                let y = y as u8;
                let z = z as u8;
                let x0 = self.p[x as usize];
                let x1 = self.p[x.wrapping_add(1) as usize];

                let xy00 = self.p[x0.wrapping_add(y) as usize];
                let xy01 = self.p[x0.wrapping_add(y).wrapping_add(1) as usize];
                let xy10 = self.p[x1.wrapping_add(y) as usize];
                let xy11 = self.p[x1.wrapping_add(y).wrapping_add(1) as usize];

                let h_z0 = [
                    self.p[xy00.wrapping_add(z) as usize] as usize,
                    self.p[xy10.wrapping_add(z) as usize] as usize,
                    self.p[xy01.wrapping_add(z) as usize] as usize,
                    self.p[xy11.wrapping_add(z) as usize] as usize,
                ];
                let h_z1 = [
                    self.p[xy00.wrapping_add(z).wrapping_add(1) as usize] as usize,
                    self.p[xy10.wrapping_add(z).wrapping_add(1) as usize] as usize,
                    self.p[xy01.wrapping_add(z).wrapping_add(1) as usize] as usize,
                    self.p[xy11.wrapping_add(z).wrapping_add(1) as usize] as usize,
                ];

                let xr_v = f64x4::from_array([xr, xr - 1.0, xr, xr - 1.0]);
                let yr_v = f64x4::from_array([yr, yr, yr - 1.0, yr - 1.0]);
                let zr_v0 = f64x4::splat(zr);
                let zr_v1 = f64x4::splat(zr - 1.0);

                let [d000, d100, d010, d110] = grad_dot_simd(h_z0, xr_v, yr_v, zr_v0).to_array();
                let [d001, d101, d011, d111] = grad_dot_simd(h_z1, xr_v, yr_v, zr_v1).to_array();
                (d000, d100, d010, d110, d001, d101, d011, d111)
            }

            #[cfg(not(target_feature = "avx512f"))]
            {
                let px1 = x.wrapping_add(1);
                let py1 = y.wrapping_add(1);
                let pz1 = z.wrapping_add(1);
                let xr1 = xr - 1.0;
                let yr1 = yr - 1.0;
                let zr1 = zr - 1.0;

                let d000 = permutation_gradient_dot(&self.p, x, y, z, xr, yr, zr);
                let d100 = permutation_gradient_dot(&self.p, px1, y, z, xr1, yr, zr);
                let d010 = permutation_gradient_dot(&self.p, x, py1, z, xr, yr1, zr);
                let d110 = permutation_gradient_dot(&self.p, px1, py1, z, xr1, yr1, zr);
                let d001 = permutation_gradient_dot(&self.p, x, y, pz1, xr, yr, zr1);
                let d101 = permutation_gradient_dot(&self.p, px1, y, pz1, xr1, yr, zr1);
                let d011 = permutation_gradient_dot(&self.p, x, py1, pz1, xr, yr1, zr1);
                let d111 = permutation_gradient_dot(&self.p, px1, py1, pz1, xr1, yr1, zr1);
                (d000, d100, d010, d110, d001, d101, d011, d111)
            }
        };

        let alpha_x = smoothstep(xr);
        let alpha_y = smoothstep(yr_original);
        let alpha_z = smoothstep(zr);
        lerp3(
            alpha_x, alpha_y, alpha_z, d000, d100, d010, d110, d001, d101, d011, d111,
        )
    }

    #[inline]
    fn permutation_gather<const N: usize>(&self, idx: Simd<u8, N>) -> Simd<u8, N> {
        let offset = idx.cast::<usize>();
        let p = Simd::splat(self.p.as_ptr()).wrapping_add(offset);
        // SAFETY: `idx` is a `Simd<u8, N>`, meaning each lane's index is at most 255.
        // `self.p` has length 256, so all offsets are guaranteed to be within bounds of `self.p`.
        unsafe { Simd::gather_ptr(p) }
    }

    #[expect(clippy::too_many_arguments, reason = "matches vanilla signature")]
    fn sample_and_lerp_simd<F, const N: usize>(
        &self,
        x: Simd<i32, N>,
        y: Simd<i32, N>,
        z: Simd<i32, N>,
        xr: Simd<F, N>,
        yr: Simd<F, N>,
        zr: Simd<F, N>,
        yr_original: Simd<F, N>,
    ) -> Simd<F, N>
    where
        F: SimdElement + SimdCast,
        Simd<F, N>: ops::Mul<Output = Simd<F, N>>
            + ops::Add<Output = Simd<F, N>>
            + ops::Sub<Output = Simd<F, N>>
            + ops::Neg<Output = Simd<F, N>>,
    {
        let x = x.cast::<u8>();
        let y = y.cast::<u8>();
        let z = z.cast::<u8>();
        let x0 = self.permutation_gather(x);
        let x1 = self.permutation_gather(x + Simd::splat(1));

        let xy00 = self.permutation_gather(x0 + y);
        let xy01 = self.permutation_gather(x0 + y + Simd::splat(1));
        let xy10 = self.permutation_gather(x1 + y);
        let xy11 = self.permutation_gather(x1 + y + Simd::splat(1));

        let h000 = self.permutation_gather(xy00 + z).cast::<usize>().to_array();
        let h100 = self.permutation_gather(xy10 + z).cast::<usize>().to_array();
        let h010 = self.permutation_gather(xy01 + z).cast::<usize>().to_array();
        let h110 = self.permutation_gather(xy11 + z).cast::<usize>().to_array();
        let h001 = self
            .permutation_gather(xy00 + z + Simd::splat(1))
            .cast::<usize>()
            .to_array();
        let h101 = self
            .permutation_gather(xy10 + z + Simd::splat(1))
            .cast::<usize>()
            .to_array();
        let h011 = self
            .permutation_gather(xy01 + z + Simd::splat(1))
            .cast::<usize>()
            .to_array();
        let h111 = self
            .permutation_gather(xy11 + z + Simd::splat(1))
            .cast::<usize>()
            .to_array();

        let d000 = grad_dot_simd(h000, xr, yr, zr);
        let d100 = grad_dot_simd(h100, xr - Simd::splat(1.0).cast::<F>(), yr, zr);
        let d010 = grad_dot_simd(h010, xr, yr - Simd::splat(1.0).cast::<F>(), zr);
        let d110 = grad_dot_simd(
            h110,
            xr - Simd::splat(1.0).cast::<F>(),
            yr - Simd::splat(1.0).cast::<F>(),
            zr,
        );
        let d001 = grad_dot_simd(h001, xr, yr, zr - Simd::splat(1.0).cast::<F>());
        let d101 = grad_dot_simd(
            h101,
            xr - Simd::splat(1.0).cast::<F>(),
            yr,
            zr - Simd::splat(1.0).cast::<F>(),
        );
        let d011 = grad_dot_simd(
            h011,
            xr,
            yr - Simd::splat(1.0).cast::<F>(),
            zr - Simd::splat(1.0).cast::<F>(),
        );
        let d111 = grad_dot_simd(
            h111,
            xr - Simd::splat(1.0).cast::<F>(),
            yr - Simd::splat(1.0).cast::<F>(),
            zr - Simd::splat(1.0).cast::<F>(),
        );

        let x_alpha = smoothstep_simd(xr);
        let y_alpha = smoothstep_simd(yr_original);
        let z_alpha = smoothstep_simd(zr);

        lerp3_simd(
            x_alpha, y_alpha, z_alpha, d000, d100, d010, d110, d001, d101, d011, d111,
        )
    }

    #[inline]
    pub fn noise_with_y_scale_simd<const N: usize>(
        &self,
        x: f64,
        ys: Simd<f64, N>,
        z: f64,
        y_scale: f64,
        y_fudges: Simd<f64, N>,
    ) -> Simd<f64, N> {
        let x = x + self.xo;
        let z = z + self.zo;
        let xf = fast_floor(x);
        let zf = fast_floor(z);
        let xr = x - f64::from(xf);
        let zr = z - f64::from(zf);

        let ys = ys + Simd::splat(self.yo);
        let ys_floor = fast_floor_simd::<f64, i32, N>(ys);
        let yrs = ys - ys_floor.cast();

        let yr_fudge: Simd<f64, N> = if y_scale == 0.0 {
            Simd::splat(0.0)
        } else {
            let y_scale_v: Simd<f64, N> = Simd::splat(y_scale);
            let zero: Simd<f64, N> = Simd::splat(0.0);
            let mask = y_fudges.simd_ge(zero) & y_fudges.simd_lt(yrs);
            let fudge_limits = mask.select(y_fudges, yrs);
            let epsilon: Simd<f64, N> = Simd::splat(f64::from(1.0e-7_f32));
            ((fudge_limits / y_scale_v) + epsilon).floor() * y_scale_v
        };

        let yrs_adjusted = yrs - yr_fudge;

        self.sample_and_lerp_y_simd(xf, zf, xr, zr, ys_floor, yrs_adjusted, yrs)
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors scalar sample_and_lerp with Nx SIMD y-batching"
    )]
    #[inline]
    fn sample_and_lerp_y_simd<const N: usize>(
        &self,
        xf: i32,
        zf: i32,
        xr: f64,
        zr: f64,
        ys_floor: Simd<i32, N>,
        yrs: Simd<f64, N>,
        yrs_original: Simd<f64, N>,
    ) -> Simd<f64, N> {
        let xf = xf as u8;
        let zf = zf as u8;
        let x0 = self.p[xf as usize];
        let x1 = self.p[xf.wrapping_add(1) as usize];

        let yf = ys_floor.cast();

        let mut h000 = [0usize; N];
        let mut h100 = [0usize; N];
        let mut h010 = [0usize; N];
        let mut h110 = [0usize; N];
        let mut h001 = [0usize; N];
        let mut h101 = [0usize; N];
        let mut h011 = [0usize; N];
        let mut h111 = [0usize; N];

        for i in 0..N {
            let y = yf[i];
            let xy00 = self.p[x0.wrapping_add(y) as usize];
            let xy01 = self.p[x0.wrapping_add(y).wrapping_add(1) as usize];
            let xy10 = self.p[x1.wrapping_add(y) as usize];
            let xy11 = self.p[x1.wrapping_add(y).wrapping_add(1) as usize];
            h000[i] = self.p[xy00.wrapping_add(zf) as usize] as usize;
            h100[i] = self.p[xy10.wrapping_add(zf) as usize] as usize;
            h010[i] = self.p[xy01.wrapping_add(zf) as usize] as usize;
            h110[i] = self.p[xy11.wrapping_add(zf) as usize] as usize;
            h001[i] = self.p[xy00.wrapping_add(zf).wrapping_add(1) as usize] as usize;
            h101[i] = self.p[xy10.wrapping_add(zf).wrapping_add(1) as usize] as usize;
            h011[i] = self.p[xy01.wrapping_add(zf).wrapping_add(1) as usize] as usize;
            h111[i] = self.p[xy11.wrapping_add(zf).wrapping_add(1) as usize] as usize;
        }

        let xr_v: Simd<f64, N> = Simd::splat(xr);
        let zr_v: Simd<f64, N> = Simd::splat(zr);
        let one: Simd<f64, N> = Simd::splat(1.0);
        let xr_m1 = xr_v - one;
        let yr_m1 = yrs - one;
        let zr_m1 = zr_v - one;

        let d000 = grad_dot_simd(h000, xr_v, yrs, zr_v);
        let d100 = grad_dot_simd(h100, xr_m1, yrs, zr_v);
        let d010 = grad_dot_simd(h010, xr_v, yr_m1, zr_v);
        let d110 = grad_dot_simd(h110, xr_m1, yr_m1, zr_v);
        let d001 = grad_dot_simd(h001, xr_v, yrs, zr_m1);
        let d101 = grad_dot_simd(h101, xr_m1, yrs, zr_m1);
        let d011 = grad_dot_simd(h011, xr_v, yr_m1, zr_m1);
        let d111 = grad_dot_simd(h111, xr_m1, yr_m1, zr_m1);

        let x_alpha: Simd<f64, N> = Simd::splat(smoothstep(xr));
        let y_alpha = smoothstep_simd(yrs_original);
        let z_alpha: Simd<f64, N> = Simd::splat(smoothstep(zr));

        lerp3_simd(
            x_alpha, y_alpha, z_alpha, d000, d100, d010, d110, d001, d101, d011, d111,
        )
    }

    #[expect(clippy::too_many_arguments, reason = "matches vanilla signature")]
    fn sample_with_derivative(
        &self,
        x: i32,
        y: i32,
        z: i32,
        xr: f64,
        yr: f64,
        zr: f64,
        derivative_out: &mut [f64; 3],
    ) -> f64 {
        let x = x as u8;
        let y = y as u8;
        let z = z as u8;

        let x0 = self.p[x as usize];
        let x1 = self.p[x.wrapping_add(1) as usize];
        let xy00 = self.p[x0.wrapping_add(y) as usize];
        let xy01 = self.p[x0.wrapping_add(y).wrapping_add(1) as usize];
        let xy10 = self.p[x1.wrapping_add(y) as usize];
        let xy11 = self.p[x1.wrapping_add(y).wrapping_add(1) as usize];

        let h000 = self.p[xy00.wrapping_add(z) as usize] as usize;
        let h100 = self.p[xy10.wrapping_add(z) as usize] as usize;
        let h010 = self.p[xy01.wrapping_add(z) as usize] as usize;
        let h110 = self.p[xy11.wrapping_add(z) as usize] as usize;
        let h001 = self.p[xy00.wrapping_add(z).wrapping_add(1) as usize] as usize;
        let h101 = self.p[xy10.wrapping_add(z).wrapping_add(1) as usize] as usize;
        let h011 = self.p[xy01.wrapping_add(z).wrapping_add(1) as usize] as usize;
        let h111 = self.p[xy11.wrapping_add(z).wrapping_add(1) as usize] as usize;

        let g000 = Simd::from_array(GRADIENT[h000 & 15]);
        let g100 = Simd::from_array(GRADIENT[h100 & 15]);
        let g010 = Simd::from_array(GRADIENT[h010 & 15]);
        let g110 = Simd::from_array(GRADIENT[h110 & 15]);
        let g001 = Simd::from_array(GRADIENT[h001 & 15]);
        let g101 = Simd::from_array(GRADIENT[h101 & 15]);
        let g011 = Simd::from_array(GRADIENT[h011 & 15]);
        let g111 = Simd::from_array(GRADIENT[h111 & 15]);

        let d000 = grad_dot(h000, xr, yr, zr);
        let d100 = grad_dot(h100, xr - 1.0, yr, zr);
        let d010 = grad_dot(h010, xr, yr - 1.0, zr);
        let d110 = grad_dot(h110, xr - 1.0, yr - 1.0, zr);
        let d001 = grad_dot(h001, xr, yr, zr - 1.0);
        let d101 = grad_dot(h101, xr - 1.0, yr, zr - 1.0);
        let d011 = grad_dot(h011, xr, yr - 1.0, zr - 1.0);
        let d111 = grad_dot(h111, xr - 1.0, yr - 1.0, zr - 1.0);

        let alpha_x = smoothstep(xr);
        let alpha_y = smoothstep(yr);
        let alpha_z = smoothstep(zr);

        let d1_v = lerp3_simd(
            Simd::splat(alpha_x),
            Simd::splat(alpha_y),
            Simd::splat(alpha_z),
            g000,
            g100,
            g010,
            g110,
            g001,
            g101,
            g011,
            g111,
        );

        let d2x = lerp2(
            alpha_y,
            alpha_z,
            d100 - d000,
            d110 - d010,
            d101 - d001,
            d111 - d011,
        );
        let d2y = lerp2(
            alpha_z,
            alpha_x,
            d010 - d000,
            d011 - d001,
            d110 - d100,
            d111 - d101,
        );
        let d2z = lerp2(
            alpha_x,
            alpha_y,
            d001 - d000,
            d101 - d100,
            d011 - d010,
            d111 - d110,
        );

        let x_sd = smoothstep_derivative(xr);
        let y_sd = smoothstep_derivative(yr);
        let z_sd = smoothstep_derivative(zr);

        derivative_out[0] += d1_v[0] + x_sd * d2x;
        derivative_out[1] += d1_v[1] + y_sd * d2y;
        derivative_out[2] += d1_v[2] + z_sd * d2z;
        lerp3(
            alpha_x, alpha_y, alpha_z, d000, d100, d010, d110, d001, d101, d011, d111,
        )
    }
}

#[cfg(not(target_feature = "avx512f"))]
#[inline]
fn permutation_gradient_dot(
    p: &[u8; 256],
    px: i32,
    py: i32,
    pz: i32,
    fx: f64,
    fy: f64,
    fz: f64,
) -> f64 {
    let qx = (px & 0xFF) as u8;
    let qy = (py & 0xFF) as u8;
    let qz = (pz & 0xFF) as u8;
    let a = p[qx as usize];
    let b = p[a.wrapping_add(qy) as usize];
    let hash = p[b.wrapping_add(qz) as usize];
    grad_dot(hash as usize, fx, fy, fz)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::rng::xoroshiro::XoroshiroRandomSource;

    fn rng(seed: u64) -> RandomSource {
        RandomSource::Xoroshiro(XoroshiroRandomSource::from_seed(seed))
    }

    #[test]
    fn deterministic_for_same_seed() {
        let noise1 = ImprovedNoise::new(&mut rng(12345));
        let noise2 = ImprovedNoise::new(&mut rng(12345));

        assert_eq!(noise1.p, noise2.p);
        assert!(
            (noise1.noise(100.0, 64.0, 100.0) - noise2.noise(100.0, 64.0, 100.0)).abs() < 1e-15
        );
    }

    #[test]
    fn zero_axis_helpers_match_full_noise() {
        let noise = ImprovedNoise::new(&mut rng(42));
        assert!((noise.noise_xz(12.5, -7.25) - noise.noise(12.5, 0.0, -7.25)).abs() < 1e-15);
        assert!((noise.noise_xy(12.5, -7.25) - noise.noise(12.5, -7.25, 0.0)).abs() < 1e-15);
    }

    #[test]
    fn noise_with_y_scale_zero_matches_plain_noise() {
        let noise = ImprovedNoise::new(&mut rng(42));
        for (x, y, z) in [
            (0.0, 0.0, 0.0),
            (1.25, 64.5, -30.75),
            (-1000.0, -20.25, 4096.5),
        ] {
            assert!(
                (noise.noise(x, y, z) - noise.noise_with_y_scale(x, y, z, 0.0, 0.0)).abs() < 1e-15
            );
        }
    }
}
