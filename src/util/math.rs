use std::simd::num::{SimdFloat, SimdInt};
use std::sync::LazyLock;
use std::{
    ops,
    simd::{
        Mask, Select, Simd, SimdCast, SimdElement, StdFloat,
        cmp::{SimdPartialEq, SimdPartialOrd},
    },
};

#[inline(always)]
pub fn fast_floor(v: f64) -> i32 {
    let i = v as i32;
    if v < f64::from(i) { i - 1 } else { i }
}

#[inline(always)]
pub fn fast_floor_simd<F, I, const N: usize>(v: Simd<F, N>) -> Simd<I, N>
where
    F: SimdElement + SimdCast,
    I: SimdElement + SimdCast,
    Simd<F, N>: SimdFloat<Cast<I> = Simd<I, N>>
        + SimdPartialOrd
        + SimdPartialEq<Mask = Mask<<F as SimdElement>::Mask, N>>,
    Simd<I, N>: SimdInt<Cast<F> = Simd<F, N>> + ops::Sub<Output = Simd<I, N>>,
{
    let i = v.cast::<I>();
    let b = v.simd_lt(i.cast::<F>());
    b.select(i - Simd::splat(1).cast(), i)
}

#[inline(always)]
pub fn floor_f32(v: f32) -> i32 {
    let i = v as i32;
    if v < i as f32 { i - 1 } else { i }
}

#[inline(always)]
pub fn ceil_f32(v: f32) -> i32 {
    let i = v as i32;
    if v > i as f32 { i + 1 } else { i }
}

#[inline(always)]
pub fn floor_f64(v: f64) -> i32 {
    let i = v as i32;
    if v < f64::from(i) { i - 1 } else { i }
}

#[inline]
pub fn fast_lfloor(v: f64) -> i64 {
    let i = v as i64;
    if v < i as f64 { i - 1 } else { i }
}

const ROUND_OFF: f64 = 33_554_432.0;
const HALF_ROUND_OFF: f64 = ROUND_OFF / 2.0;

#[inline]
pub fn wrap_simd<F, const N: usize>(x: Simd<F, N>) -> Simd<F, N>
where
    F: SimdElement + SimdCast,
    Simd<F, N>: ops::Div<Output = Simd<F, N>>
        + ops::Add<Output = Simd<F, N>>
        + ops::Mul<Output = Simd<F, N>>
        + ops::Sub<Output = Simd<F, N>>
        + SimdPartialOrd<Mask = Mask<<F as SimdElement>::Mask, N>>
        + StdFloat,
{
    let in_fast_range = x.simd_ge(Simd::splat(-HALF_ROUND_OFF).cast())
        & x.simd_lt(Simd::splat(HALF_ROUND_OFF).cast());
    if in_fast_range.all() {
        return x;
    }

    let round_off = Simd::splat(ROUND_OFF).cast();
    x - (x / round_off + Simd::splat(0.5).cast()).floor() * round_off
}

#[inline]
pub fn wrap(x: f64) -> f64 {
    if (-HALF_ROUND_OFF..HALF_ROUND_OFF).contains(&x) {
        return x;
    }

    x - (x / ROUND_OFF + 0.5).floor() * ROUND_OFF
}

pub const GRADIENT: [[f64; 3]; 16] = [
    [1.0, 1.0, 0.0],
    [-1.0, 1.0, 0.0],
    [1.0, -1.0, 0.0],
    [-1.0, -1.0, 0.0],
    [1.0, 0.0, 1.0],
    [-1.0, 0.0, 1.0],
    [1.0, 0.0, -1.0],
    [-1.0, 0.0, -1.0],
    [0.0, 1.0, 1.0],
    [0.0, -1.0, 1.0],
    [0.0, 1.0, -1.0],
    [0.0, -1.0, -1.0],
    [1.0, 1.0, 0.0],
    [0.0, -1.0, 1.0],
    [-1.0, 1.0, 0.0],
    [0.0, -1.0, -1.0],
];

pub const GRADIENT_4: [[f64; 4]; 16] = [
    [1.0, 1.0, 0.0, 0.],
    [-1.0, 1.0, 0.0, 0.],
    [1.0, -1.0, 0.0, 0.],
    [-1.0, -1.0, 0.0, 0.],
    [1.0, 0.0, 1.0, 0.],
    [-1.0, 0.0, 1.0, 0.],
    [1.0, 0.0, -1.0, 0.],
    [-1.0, 0.0, -1.0, 0.],
    [0.0, 1.0, 1.0, 0.],
    [0.0, -1.0, 1.0, 0.],
    [0.0, 1.0, -1.0, 0.],
    [0.0, -1.0, -1.0, 0.],
    [1.0, 1.0, 0.0, 0.],
    [0.0, -1.0, 1.0, 0.],
    [-1.0, 1.0, 0.0, 0.],
    [0.0, -1.0, -1.0, 0.],
];

#[inline]
pub fn dot(g: &[f64; 3], x: f64, y: f64, z: f64) -> f64 {
    g[0] * x + g[1] * y + g[2] * z
}

#[inline]
pub fn grad_dot_simd<F, const N: usize>(
    hashes: [usize; N],
    x: Simd<F, N>,
    y: Simd<F, N>,
    z: Simd<F, N>,
) -> Simd<F, N>
where
    F: SimdElement + SimdCast,
    Simd<F, N>: ops::Mul<Output = Simd<F, N>>
        + ops::Add<Output = Simd<F, N>>
        + ops::Sub<Output = Simd<F, N>>
        + ops::Neg<Output = Simd<F, N>>,
{
    #[cfg(target_feature = "avx512f")]
    {
        let hash_lanes = Simd::<i64, N>::from_array(hashes.map(|value| (value & 15) as i64));
        let u_component = hash_lanes.simd_lt(Simd::splat(8)).select(x, y);
        let v_component = hash_lanes.simd_lt(Simd::splat(4)).select(
            y,
            (hash_lanes.simd_eq(Simd::splat(12)) | hash_lanes.simd_eq(Simd::splat(14)))
                .select(x, z),
        );
        let signed_u = (hash_lanes & Simd::splat(1))
            .simd_eq(Simd::splat(0))
            .select(u_component, -u_component);
        let signed_v = (hash_lanes & Simd::splat(2))
            .simd_eq(Simd::splat(0))
            .select(v_component, -v_component);
        signed_u + signed_v
    }

    #[cfg(not(target_feature = "avx512f"))]
    {
        let gradients = hashes.map(|hash| GRADIENT[hash & 15]);
        let gx = Simd::from_array(gradients.map(|gradient| gradient[0])).cast::<F>();
        let gy = Simd::from_array(gradients.map(|gradient| gradient[1])).cast::<F>();
        let gz = Simd::from_array(gradients.map(|gradient| gradient[2])).cast::<F>();
        gx * x + gy * y + gz * z
    }
}

#[inline(always)]
pub fn grad_dot(hash: usize, x: f64, y: f64, z: f64) -> f64 {
    let g = &GRADIENT[hash & 15];
    g[0] * x + g[1] * y + g[2] * z
}

#[inline]
pub fn corner_noise_3d(index: usize, x: f64, y: f64, z: f64, base: f64) -> f64 {
    let t0 = base - x * x - y * y - z * z;
    if t0 < 0.0 {
        0.0
    } else {
        let t0 = t0 * t0;
        t0 * t0 * dot(&GRADIENT[index], x, y, z)
    }
}

#[inline]
pub fn clamped_lerp(min: f64, max: f64, factor: f64) -> f64 {
    if factor < 0.0 {
        min
    } else if factor > 1.0 {
        max
    } else {
        lerp(factor, min, max)
    }
}

#[inline]
pub fn clamped_lerp_simd<const N: usize>(
    min: Simd<f64, N>,
    max: Simd<f64, N>,
    factor: Simd<f64, N>,
) -> Simd<f64, N> {
    let zero = Simd::splat(0.0);
    let one = Simd::splat(1.0);
    let below = factor.simd_lt(zero);
    let above = factor.simd_gt(one);

    let lerped = min + factor * (max - min);

    let result = below.select(min, lerped);
    above.select(max, result)
}

#[inline]
pub fn clamp(value: f64, min: f64, max: f64) -> f64 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

#[inline]
pub fn inverse_lerp(value: f64, a: f64, b: f64) -> f64 {
    (value - a) / (b - a)
}

#[inline(always)]
pub fn lerp(alpha: f64, a: f64, b: f64) -> f64 {
    a + alpha * (b - a)
}

#[inline(always)]
pub fn lerp_simd<F, const N: usize>(alpha: Simd<F, N>, a: Simd<F, N>, b: Simd<F, N>) -> Simd<F, N>
where
    F: SimdElement,
    Simd<F, N>: ops::Mul<Output = Simd<F, N>>
        + ops::Add<Output = Simd<F, N>>
        + ops::Sub<Output = Simd<F, N>>,
{
    a + alpha * (b - a)
}

#[inline(always)]
pub fn lerp2(a1: f64, a2: f64, x00: f64, x10: f64, x01: f64, x11: f64) -> f64 {
    lerp(a2, lerp(a1, x00, x10), lerp(a1, x01, x11))
}

#[inline(always)]
pub fn lerp2_simd<F, const N: usize>(
    a1: Simd<F, N>,
    a2: Simd<F, N>,
    x00: Simd<F, N>,
    x10: Simd<F, N>,
    x01: Simd<F, N>,
    x11: Simd<F, N>,
) -> Simd<F, N>
where
    F: SimdElement,
    Simd<F, N>: ops::Mul<Output = Simd<F, N>>
        + ops::Add<Output = Simd<F, N>>
        + ops::Sub<Output = Simd<F, N>>,
{
    lerp_simd(a2, lerp_simd(a1, x00, x10), lerp_simd(a1, x01, x11))
}

#[inline(always)]
#[expect(
    clippy::too_many_arguments,
    reason = "matches vanilla's Mth.lerp3 signature with 8 grid corner values"
)]
pub fn lerp3(
    a1: f64,
    a2: f64,
    a3: f64,
    x000: f64,
    x100: f64,
    x010: f64,
    x110: f64,
    x001: f64,
    x101: f64,
    x011: f64,
    x111: f64,
) -> f64 {
    lerp(
        a3,
        lerp2(a1, a2, x000, x100, x010, x110),
        lerp2(a1, a2, x001, x101, x011, x111),
    )
}

#[inline]
#[expect(clippy::too_many_arguments, reason = "mirrors lerp3 with SIMD vectors")]
pub fn lerp3_simd<F, const N: usize>(
    a1: Simd<F, N>,
    a2: Simd<F, N>,
    a3: Simd<F, N>,
    x000: Simd<F, N>,
    x100: Simd<F, N>,
    x010: Simd<F, N>,
    x110: Simd<F, N>,
    x001: Simd<F, N>,
    x101: Simd<F, N>,
    x011: Simd<F, N>,
    x111: Simd<F, N>,
) -> Simd<F, N>
where
    F: SimdElement,
    Simd<F, N>: ops::Mul<Output = Simd<F, N>>
        + ops::Add<Output = Simd<F, N>>
        + ops::Sub<Output = Simd<F, N>>,
{
    lerp_simd(
        a3,
        lerp2_simd(a1, a2, x000, x100, x010, x110),
        lerp2_simd(a1, a2, x001, x101, x011, x111),
    )
}

#[inline]
pub fn map(value: f64, from_min: f64, from_max: f64, to_min: f64, to_max: f64) -> f64 {
    lerp(inverse_lerp(value, from_min, from_max), to_min, to_max)
}

#[inline]
pub fn map_clamped(value: f64, from_min: f64, from_max: f64, to_min: f64, to_max: f64) -> f64 {
    let t = (value - from_min) / (from_max - from_min);
    clamped_lerp(to_min, to_max, t)
}

#[inline(always)]
pub fn smoothstep(x: f64) -> f64 {
    x * x * x * (x * (x * 6.0 - 15.0) + 10.0)
}

#[inline]
pub fn smoothstep_simd<F, const N: usize>(x: Simd<F, N>) -> Simd<F, N>
where
    F: SimdElement + SimdCast,
    Simd<F, N>: ops::Mul<Output = Simd<F, N>>
        + ops::Sub<Output = Simd<F, N>>
        + ops::Add<Output = Simd<F, N>>,
{
    x * x
        * x
        * (x * (x * Simd::splat(6.0).cast() - Simd::splat(15.0).cast()) + Simd::splat(10.0).cast())
}

#[inline]
pub fn cube(x: f64) -> f64 {
    x * x * x
}

#[inline]
pub fn square(x: f64) -> f64 {
    x * x
}

const TRIG_INDEX_SCALE: f64 = 10_430.378_350_470_453;
const TRIG_TABLE_LEN: usize = 65_536;
const TRIG_TABLE_MASK: i64 = 0xFFFF;

static SIN_TABLE: LazyLock<Box<[f32; TRIG_TABLE_LEN]>> = LazyLock::new(|| {
    let mut table: Box<[f32; TRIG_TABLE_LEN]> = vec![0.0_f32; TRIG_TABLE_LEN]
        .into_boxed_slice()
        .try_into()
        .expect("65536-element vec");
    for (i, slot) in table.iter_mut().enumerate() {
        *slot = (i as f64 / TRIG_INDEX_SCALE).sin() as f32;
    }
    table
});

#[inline]
pub fn sin(angle: f64) -> f32 {
    let idx = (((angle * TRIG_INDEX_SCALE) as i64) & TRIG_TABLE_MASK) as usize;
    SIN_TABLE[idx]
}

#[inline]
pub fn cos(angle: f64) -> f32 {
    let idx = (((angle * TRIG_INDEX_SCALE + 16_384.0) as i64) & TRIG_TABLE_MASK) as usize;
    SIN_TABLE[idx]
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::f64::consts::{FRAC_PI_2, PI};
    use std::simd::f64x4;

    #[test]
    fn floor_rounds_toward_negative_infinity() {
        assert_eq!(fast_floor(3.9), 3);
        assert_eq!(fast_floor(3.0), 3);
        assert_eq!(fast_floor(-3.1), -4);
        assert_eq!(fast_floor(-3.0), -3);
        assert_eq!(fast_lfloor(3.9), 3);
        assert_eq!(fast_lfloor(-3.1), -4);
    }

    #[test]
    fn floor_simd_matches_scalar_floor() {
        let inputs = f64x4::from_array([3.9, -3.1, 0.0, -0.5]);
        let expected = [3, -4, 0, -1];
        let actual: [i32; 4] = fast_floor_simd(inputs).to_array();
        assert_eq!(actual, expected);
    }

    #[test]
    fn wrap_is_identity_within_range() {
        for x in [0.0, 42.5, -42.5, HALF_ROUND_OFF - 1.0, -HALF_ROUND_OFF] {
            assert!((wrap(x) - x).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn wrap_keeps_far_coordinates_bounded() {
        for x in [1e9, -1e9, ROUND_OFF * 3.0, -ROUND_OFF * 3.0] {
            assert!(wrap(x).abs() <= HALF_ROUND_OFF);
        }
    }

    #[test]
    fn wrap_simd_matches_scalar_wrap_lane_by_lane() {
        let inputs = [12.0, -1e9, HALF_ROUND_OFF, ROUND_OFF * 2.5];
        let actual = wrap_simd(f64x4::from_array(inputs)).to_array();
        for (input, lane) in inputs.into_iter().zip(actual) {
            assert_eq!(lane, wrap(input));
        }
    }

    #[test]
    fn lerp_reproduces_endpoints_and_midpoint() {
        assert!((lerp(0.0, 5.0, 9.0) - 5.0).abs() < f64::EPSILON);
        assert!((lerp(1.0, 5.0, 9.0) - 9.0).abs() < f64::EPSILON);
        assert!((lerp(0.5, 5.0, 9.0) - 7.0).abs() < f64::EPSILON);
    }

    #[test]
    fn lerp2_reproduces_each_grid_corner() {
        let (x00, x10, x01, x11) = (1.0, 2.0, 3.0, 4.0);
        assert!((lerp2(0.0, 0.0, x00, x10, x01, x11) - x00).abs() < f64::EPSILON);
        assert!((lerp2(1.0, 0.0, x00, x10, x01, x11) - x10).abs() < f64::EPSILON);
        assert!((lerp2(0.0, 1.0, x00, x10, x01, x11) - x01).abs() < f64::EPSILON);
        assert!((lerp2(1.0, 1.0, x00, x10, x01, x11) - x11).abs() < f64::EPSILON);
    }

    #[test]
    fn clamped_lerp_saturates_outside_zero_one() {
        assert!((clamped_lerp(2.0, 8.0, -1.0) - 2.0).abs() < f64::EPSILON);
        assert!((clamped_lerp(2.0, 8.0, 2.0) - 8.0).abs() < f64::EPSILON);
        assert!((clamped_lerp(2.0, 8.0, 0.5) - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn smoothstep_is_zero_one_at_endpoints_and_symmetric() {
        assert!((smoothstep(0.0) - 0.0).abs() < f64::EPSILON);
        assert!((smoothstep(1.0) - 1.0).abs() < f64::EPSILON);
        for x in [0.1, 0.3, 0.5, 0.7, 0.9] {
            assert!((smoothstep(x) + smoothstep(1.0 - x) - 1.0).abs() < 1e-12);
        }
    }

    #[test]
    fn sin_cos_match_known_angles_within_table_resolution() {
        let tolerance = 1e-4;
        assert!((f64::from(sin(0.0))).abs() < tolerance);
        assert!((f64::from(cos(0.0)) - 1.0).abs() < tolerance);
        assert!((f64::from(sin(PI / 2.0)) - 1.0).abs() < tolerance);
        assert!((f64::from(cos(PI)) + 1.0).abs() < tolerance);
    }

    #[test]
    fn cos_is_sin_quarter_turn_ahead() {
        for angle in [0.2_f64, 1.3, 2.5, -0.8] {
            assert!((f64::from(cos(angle)) - f64::from(sin(angle + FRAC_PI_2))).abs() < 1e-4);
        }
    }

    #[test]
    fn sin_is_periodic_over_a_full_turn() {
        for angle in [0.4_f64, 1.1, 3.0] {
            let full_turn = 2.0 * PI;
            assert!((f64::from(sin(angle)) - f64::from(sin(angle + full_turn))).abs() < 1e-4);
        }
    }
}
