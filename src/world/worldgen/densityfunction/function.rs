use std::hash::{Hash, Hasher};
use std::sync::{Arc, OnceLock};

use shulkr::util::Axis;

use crate::util::interval::Interval;
use crate::world::worldgen::synth::normal_noise::NormalNoise;
use crate::world::worldgen::synth::smeared_perlin_noise::SmearedPerlinNoise;

use super::spline::CubicSpline;
use crate::util::interval;
use std::mem;

pub type Df = Arc<DensityFunction>;

pub const AXIS_X: u8 = 1;
pub const AXIS_Y: u8 = 2;
pub const AXIS_Z: u8 = 4;
pub const NO_AXES: u8 = 0;
pub const ALL_AXES: u8 = 7;

pub const fn axes_from(axis: Axis) -> u8 {
    match axis {
        Axis::X => AXIS_X,
        Axis::Y => AXIS_Y,
        Axis::Z => AXIS_Z,
    }
}

#[derive(Debug)]
pub struct Holder {
    pub key: String,
    pub value: Arc<OnceLock<Df>>,
}

impl Holder {
    pub fn value(&self) -> &Df {
        self.value
            .get()
            .unwrap_or_else(|| panic!("unbound density function reference {}", self.key))
    }
}

#[derive(Debug, Clone)]
pub struct NoiseHolder {
    pub key: String,
    pub noise: Arc<NormalNoise>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DistanceMetric {
    Euclidean,
    EuclideanSquared,
    Manhattan,
    Chebyshev,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TilingMode {
    ClampToEdge,
    Repeat,
    MirroredRepeat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryType {
    Abs,
    Square,
    Cube,
    Sqrt,
    HalfNegative,
    QuarterNegative,
    Reciprocal,
    Negate,
    Squeeze,
    Log,
    Sign,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoundType {
    Floor,
    Round,
    Ceil,
    Truncate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinaryType {
    Add,
    Sub,
    Mul,
    Div,
    Min,
    Max,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SimpleType {
    BlendAlpha,
    BlendOffset,
    Beardifier,
}

#[derive(Debug, Clone, Copy)]
pub struct Gradient {
    pub axis: Axis,
    pub tiling: TilingMode,
    pub from_coordinate: i32,
    pub to_coordinate: i32,
    pub from_value: f32,
    pub to_value: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct BlendedNoise {
    pub xz_scale: f64,
    pub y_scale: f64,
    pub xz_factor: f64,
    pub y_factor: f64,
    pub smear_scale_multiplier: f64,
}

pub const BASE_SCALE: f64 = 684.412;
pub const LIMIT_FACTOR: f64 = 0.999_984_74;

impl BlendedNoise {
    pub fn xz_multiplier(&self) -> f64 {
        BASE_SCALE * self.xz_scale
    }

    pub fn y_multiplier(&self) -> f64 {
        BASE_SCALE * self.y_scale
    }

    fn compute_fbm_range(first_octave: i32, smear_scale_y: f64, value_factor: f64) -> Interval {
        let octaves = -first_octave + 1;
        let mut factor = 1.0;
        let mut value_factor = value_factor / (2.0f64.powf(f64::from(octaves)) - 1.0);
        let mut range = Interval::of_exact(0.0);
        for _ in 0..octaves {
            let layer = Interval::mul(
                SmearedPerlinNoise::range_for(smear_scale_y * factor),
                Interval::of_exact(value_factor as f32),
            );
            range = Interval::add(range, layer);
            factor /= 2.0;
            value_factor *= 2.0;
        }
        range
    }

    pub fn range(&self) -> Interval {
        Self::compute_fbm_range(
            -15,
            self.y_multiplier() * self.smear_scale_multiplier,
            f64::from(LIMIT_FACTOR as f32),
        )
    }
}

#[derive(Debug, Clone)]
pub struct PreparedCache {
    pub id: usize,
    pub range: Interval,
    pub domain_axes: u8,
    pub input: Df,
}

#[derive(Debug)]
pub enum DensityFunction {
    Constant(f32),
    Holder(Holder),
    Noise {
        noise: NoiseHolder,
        xz_scale: f64,
        y_scale: f64,
        shift_x: Df,
        shift_y: Df,
        shift_z: Df,
    },
    ShiftA(NoiseHolder),
    ShiftB(NoiseHolder),
    Shift(NoiseHolder),
    EndOuterIslands,
    DistanceToPoint {
        point: [i32; 3],
        metric: DistanceMetric,
    },
    Gradient(Gradient),
    Unary(UnaryType, Df),
    Round(RoundType, Df, Df),
    Binary(BinaryType, Df, Df),
    Pow(Df, Df),
    Spline(Arc<CubicSpline>),
    Lerp(Df, Df, Df),
    Clamp(Df, f32, f32),
    RangeChoice {
        input: Df,
        min_inclusive: f32,
        max_exclusive: f32,
        when_in_range: Df,
        when_out_of_range: Df,
    },
    IntervalSelect {
        input: Df,
        thresholds: Vec<f32>,
        functions: Vec<Df>,
    },
    Cache(Df),
    BlendDensity(Df),
    Interpolated {
        input: Df,
        cell_size_xz: i32,
        cell_size_y: i32,
    },
    Slice {
        axis: Axis,
        coordinate: i32,
        input: Df,
    },
    FindTopSurface {
        density: Df,
        upper_bound: Df,
        lower_bound: i32,
        cell_height: i32,
    },
    OldBlendedNoise(BlendedNoise),
    Simple(SimpleType),
    PreparedCache(PreparedCache),
}

pub const fn float_eq(a: f32, b: f32) -> bool {
    float_key(a) == float_key(b)
}

pub const fn float_key(v: f32) -> u32 {
    if v.is_nan() { 0x7fc0_0000 } else { v.to_bits() }
}

const fn double_key(v: f64) -> u64 {
    if v.is_nan() {
        0x7ff8_0000_0000_0000
    } else {
        v.to_bits()
    }
}

impl PartialEq for DensityFunction {
    fn eq(&self, other: &Self) -> bool {
        use DensityFunction as D;
        match (self, other) {
            (D::Constant(a), D::Constant(b)) => float_eq(*a, *b),
            (D::Holder(a), D::Holder(b)) => a.key == b.key,
            (
                D::Noise {
                    noise: n1,
                    xz_scale: x1,
                    y_scale: y1,
                    shift_x: a1,
                    shift_y: b1,
                    shift_z: c1,
                },
                D::Noise {
                    noise: n2,
                    xz_scale: x2,
                    y_scale: y2,
                    shift_x: a2,
                    shift_y: b2,
                    shift_z: c2,
                },
            ) => {
                n1.key == n2.key
                    && double_key(*x1) == double_key(*x2)
                    && double_key(*y1) == double_key(*y2)
                    && a1 == a2
                    && b1 == b2
                    && c1 == c2
            }
            (D::ShiftA(a), D::ShiftA(b))
            | (D::ShiftB(a), D::ShiftB(b))
            | (D::Shift(a), D::Shift(b)) => a.key == b.key,
            (D::EndOuterIslands, D::EndOuterIslands) => true,
            (
                D::DistanceToPoint {
                    point: p1,
                    metric: m1,
                },
                D::DistanceToPoint {
                    point: p2,
                    metric: m2,
                },
            ) => p1 == p2 && m1 == m2,
            (D::Gradient(a), D::Gradient(b)) => {
                a.axis == b.axis
                    && a.tiling == b.tiling
                    && a.from_coordinate == b.from_coordinate
                    && a.to_coordinate == b.to_coordinate
                    && float_eq(a.from_value, b.from_value)
                    && float_eq(a.to_value, b.to_value)
            }
            (D::Unary(t1, a1), D::Unary(t2, a2)) => t1 == t2 && a1 == a2,
            (D::Round(t1, a1, b1), D::Round(t2, a2, b2)) => t1 == t2 && a1 == a2 && b1 == b2,
            (D::Binary(t1, a1, b1), D::Binary(t2, a2, b2)) => t1 == t2 && a1 == a2 && b1 == b2,
            (D::Pow(a1, b1), D::Pow(a2, b2)) => a1 == a2 && b1 == b2,
            (D::Spline(a), D::Spline(b)) => a == b,
            (D::Lerp(a1, b1, c1), D::Lerp(a2, b2, c2)) => a1 == a2 && b1 == b2 && c1 == c2,
            (D::Clamp(a1, l1, h1), D::Clamp(a2, l2, h2)) => {
                a1 == a2 && float_eq(*l1, *l2) && float_eq(*h1, *h2)
            }
            (
                D::RangeChoice {
                    input: i1,
                    min_inclusive: l1,
                    max_exclusive: h1,
                    when_in_range: w1,
                    when_out_of_range: o1,
                },
                D::RangeChoice {
                    input: i2,
                    min_inclusive: l2,
                    max_exclusive: h2,
                    when_in_range: w2,
                    when_out_of_range: o2,
                },
            ) => i1 == i2 && float_eq(*l1, *l2) && float_eq(*h1, *h2) && w1 == w2 && o1 == o2,
            (
                D::IntervalSelect {
                    input: i1,
                    thresholds: t1,
                    functions: f1,
                },
                D::IntervalSelect {
                    input: i2,
                    thresholds: t2,
                    functions: f2,
                },
            ) => {
                i1 == i2
                    && t1.len() == t2.len()
                    && t1.iter().zip(t2).all(|(a, b)| float_eq(*a, *b))
                    && f1 == f2
            }
            (D::Cache(a), D::Cache(b)) | (D::BlendDensity(a), D::BlendDensity(b)) => a == b,
            (
                D::Interpolated {
                    input: a,
                    cell_size_xz: x1,
                    cell_size_y: y1,
                },
                D::Interpolated {
                    input: b,
                    cell_size_xz: x2,
                    cell_size_y: y2,
                },
            ) => a == b && x1 == x2 && y1 == y2,
            (
                D::Slice {
                    axis: a1,
                    coordinate: c1,
                    input: i1,
                },
                D::Slice {
                    axis: a2,
                    coordinate: c2,
                    input: i2,
                },
            ) => a1 == a2 && c1 == c2 && i1 == i2,
            (
                D::FindTopSurface {
                    density: d1,
                    upper_bound: u1,
                    lower_bound: l1,
                    cell_height: c1,
                },
                D::FindTopSurface {
                    density: d2,
                    upper_bound: u2,
                    lower_bound: l2,
                    cell_height: c2,
                },
            ) => d1 == d2 && u1 == u2 && l1 == l2 && c1 == c2,
            (D::OldBlendedNoise(a), D::OldBlendedNoise(b)) => {
                double_key(a.xz_scale) == double_key(b.xz_scale)
                    && double_key(a.y_scale) == double_key(b.y_scale)
                    && double_key(a.xz_factor) == double_key(b.xz_factor)
                    && double_key(a.y_factor) == double_key(b.y_factor)
                    && double_key(a.smear_scale_multiplier) == double_key(b.smear_scale_multiplier)
            }
            (D::Simple(a), D::Simple(b)) => a == b,
            (D::PreparedCache(a), D::PreparedCache(b)) => a.id == b.id,
            _ => false,
        }
    }
}

impl Eq for DensityFunction {}

impl Hash for DensityFunction {
    fn hash<H: Hasher>(&self, state: &mut H) {
        use DensityFunction as D;
        mem::discriminant(self).hash(state);
        match self {
            D::Constant(v) => float_key(*v).hash(state),
            D::Holder(h) => h.key.hash(state),
            D::Noise {
                noise,
                xz_scale,
                y_scale,
                shift_x,
                shift_y,
                shift_z,
            } => {
                noise.key.hash(state);
                double_key(*xz_scale).hash(state);
                double_key(*y_scale).hash(state);
                shift_x.hash(state);
                shift_y.hash(state);
                shift_z.hash(state);
            }
            D::ShiftA(n) | D::ShiftB(n) | D::Shift(n) => n.key.hash(state),
            D::EndOuterIslands => {}
            D::DistanceToPoint { point, metric } => {
                point.hash(state);
                metric.hash(state);
            }
            D::Gradient(g) => {
                (g.axis as u8).hash(state);
                g.tiling.hash(state);
                g.from_coordinate.hash(state);
                g.to_coordinate.hash(state);
                float_key(g.from_value).hash(state);
                float_key(g.to_value).hash(state);
            }
            D::Unary(t, a) => {
                t.hash(state);
                a.hash(state);
            }
            D::Round(t, a, b) => {
                t.hash(state);
                a.hash(state);
                b.hash(state);
            }
            D::Binary(t, a, b) => {
                t.hash(state);
                a.hash(state);
                b.hash(state);
            }
            D::Pow(a, b) => {
                a.hash(state);
                b.hash(state);
            }
            D::Spline(s) => s.hash(state),
            D::Lerp(a, b, c) => {
                a.hash(state);
                b.hash(state);
                c.hash(state);
            }
            D::Clamp(a, l, h) => {
                a.hash(state);
                float_key(*l).hash(state);
                float_key(*h).hash(state);
            }
            D::RangeChoice {
                input,
                min_inclusive,
                max_exclusive,
                when_in_range,
                when_out_of_range,
            } => {
                input.hash(state);
                float_key(*min_inclusive).hash(state);
                float_key(*max_exclusive).hash(state);
                when_in_range.hash(state);
                when_out_of_range.hash(state);
            }
            D::IntervalSelect {
                input,
                thresholds,
                functions,
            } => {
                input.hash(state);
                for t in thresholds {
                    float_key(*t).hash(state);
                }
                functions.hash(state);
            }
            D::Cache(a) | D::BlendDensity(a) => a.hash(state),
            D::Interpolated {
                input,
                cell_size_xz,
                cell_size_y,
            } => {
                input.hash(state);
                cell_size_xz.hash(state);
                cell_size_y.hash(state);
            }
            D::Slice {
                axis,
                coordinate,
                input,
            } => {
                (*axis as u8).hash(state);
                coordinate.hash(state);
                input.hash(state);
            }
            D::FindTopSurface {
                density,
                upper_bound,
                lower_bound,
                cell_height,
            } => {
                density.hash(state);
                upper_bound.hash(state);
                lower_bound.hash(state);
                cell_height.hash(state);
            }
            D::OldBlendedNoise(b) => {
                double_key(b.xz_scale).hash(state);
                double_key(b.y_scale).hash(state);
            }
            D::Simple(s) => s.hash(state),
            D::PreparedCache(p) => p.id.hash(state),
        }
    }
}

fn leaky_relu(negative_factor: f32, input: f32) -> f32 {
    if input > 0.0 {
        input
    } else {
        input * negative_factor
    }
}

pub fn squeeze(input: f32) -> f32 {
    let clamped = clamp(input, -1.0, 1.0);
    clamped / 2.0 - clamped * clamped * clamped / 24.0
}

pub fn clamp(value: f32, min: f32, max: f32) -> f32 {
    if value < min {
        min
    } else {
        interval::java_min(value, max)
    }
}

pub fn round_to_integer(input: f32, kind: RoundType) -> f32 {
    match kind {
        RoundType::Floor => f64::from(input).floor() as f32,
        RoundType::Round => java_round(input) as f32,
        RoundType::Ceil => f64::from(input).ceil() as f32,
        RoundType::Truncate => {
            if input > 0.0 {
                f64::from(input).floor() as f32
            } else {
                f64::from(input).ceil() as f32
            }
        }
    }
}

pub const fn java_round(a: f32) -> i32 {
    let int_bits = a.to_bits() as i32;
    let biased_exp = (int_bits & 0x7F80_0000) >> 23;
    let shift = (22 + 127) - biased_exp;
    if (shift & -32) == 0 {
        let mut r = (int_bits & 0x007F_FFFF) | 0x0080_0000;
        if int_bits < 0 {
            r = -r;
        }
        ((r >> shift) + 1) >> 1
    } else {
        a as i32
    }
}

impl DensityFunction {
    pub fn constant(value: f32) -> Df {
        Arc::new(Self::Constant(value))
    }

    pub const fn is_zero_constant(&self) -> bool {
        matches!(self, Self::Constant(v) if float_eq(*v, 0.0))
    }

    pub fn range(&self) -> Interval {
        use DensityFunction as D;
        match self {
            D::Constant(v) => Interval::of_exact(*v),
            D::Holder(h) => h.value().range(),
            D::Noise { noise, .. } => noise.noise.range(),
            D::ShiftA(n) | D::ShiftB(n) | D::Shift(n) => {
                Interval::mul(n.noise.range(), Interval::of_exact(4.0))
            }
            D::EndOuterIslands => Interval::of(-0.843_75, 0.5625),
            D::DistanceToPoint { .. } => Interval::of(0.0, f32::INFINITY),
            D::Gradient(g) => Interval::encapsulating(g.from_value, g.to_value),
            D::Unary(kind, input) => {
                let input = input.range();
                match kind {
                    UnaryType::Abs => Interval::abs(input),
                    UnaryType::Square => Interval::square(input),
                    UnaryType::Cube => Interval::map_monotonic(input, |v| v * v * v),
                    UnaryType::Sqrt => Interval::pow(input, Interval::of_exact(0.5)),
                    UnaryType::HalfNegative => {
                        Interval::map_monotonic(input, |v| leaky_relu(0.5, v))
                    }
                    UnaryType::QuarterNegative => {
                        Interval::map_monotonic(input, |v| leaky_relu(0.25, v))
                    }
                    UnaryType::Reciprocal => Interval::reciprocal(input),
                    UnaryType::Negate => Interval::sub(Interval::of_exact(0.0), input),
                    UnaryType::Squeeze => Interval::map_monotonic(input, squeeze),
                    UnaryType::Log => Interval::log(input),
                    UnaryType::Sign => Interval::sign(input),
                }
            }
            D::Round(kind, input, multiple) => {
                let multiple_range = multiple.range();
                let kind = *kind;
                Interval::mul(
                    Interval::map_monotonic(Interval::div(input.range(), multiple_range), |v| {
                        round_to_integer(v, kind)
                    }),
                    multiple_range,
                )
            }
            D::Binary(kind, left, right) => {
                let left = left.range();
                let right = right.range();
                match kind {
                    BinaryType::Add => Interval::add(left, right),
                    BinaryType::Sub => Interval::sub(left, right),
                    BinaryType::Mul => Interval::mul(left, right),
                    BinaryType::Div => Interval::div(left, right),
                    BinaryType::Min => Interval::min_of(left, right),
                    BinaryType::Max => Interval::max_of(left, right),
                }
            }
            D::Pow(base, exponent) => Interval::pow(base.range(), exponent.range()),
            D::Spline(s) => s.range(),
            D::Lerp(alpha, first, second) => {
                Interval::lerp(alpha.range(), first.range(), second.range())
            }
            D::Clamp(input, min, max) => Interval::clamp(input.range(), *min, *max),
            D::RangeChoice {
                when_in_range,
                when_out_of_range,
                ..
            } => Interval::encapsulating_all([when_in_range.range(), when_out_of_range.range()]),
            D::IntervalSelect { functions, .. } => {
                Interval::encapsulating_all(functions.iter().map(|f| f.range()))
            }
            D::Cache(input)
            | D::BlendDensity(input)
            | D::Interpolated { input, .. }
            | D::Slice { input, .. } => input.range(),
            D::FindTopSurface {
                upper_bound,
                lower_bound,
                ..
            } => Interval::of(
                *lower_bound as f32,
                interval::java_max(*lower_bound as f32, upper_bound.range().max()),
            ),
            D::OldBlendedNoise(b) => b.range(),
            D::Simple(SimpleType::BlendAlpha) => Interval::of(0.0, 1.0),
            D::Simple(SimpleType::BlendOffset | SimpleType::Beardifier) => Interval::INFINITE,
            D::PreparedCache(p) => p.range,
        }
    }

    pub fn domain_axes(&self) -> u8 {
        use DensityFunction as D;
        match self {
            D::Constant(_) => NO_AXES,
            D::Holder(h) => h.value().domain_axes(),
            D::Noise {
                xz_scale,
                y_scale,
                shift_x,
                shift_y,
                shift_z,
                ..
            } => {
                let mut axes = ALL_AXES;
                if *y_scale == 0.0 {
                    axes &= !AXIS_Y;
                }
                if *xz_scale == 0.0 {
                    axes &= !(AXIS_X | AXIS_Z);
                }
                axes | shift_x.domain_axes() | shift_y.domain_axes() | shift_z.domain_axes()
            }
            D::Shift(_) => ALL_AXES,
            D::ShiftA(_) | D::ShiftB(_) | D::EndOuterIslands => AXIS_X | AXIS_Z,
            D::DistanceToPoint { .. } => ALL_AXES,
            D::Gradient(g) => axes_from(g.axis),
            D::Unary(_, input)
            | D::Clamp(input, _, _)
            | D::Cache(input)
            | D::BlendDensity(input)
            | D::Interpolated { input, .. } => input.domain_axes(),
            D::Round(_, a, b) | D::Binary(_, a, b) | D::Pow(a, b) => {
                a.domain_axes() | b.domain_axes()
            }
            D::Spline(s) => {
                let mut axes = NO_AXES;
                s.for_each_coordinate(&mut |c| axes |= c.domain_axes());
                axes
            }
            D::Lerp(a, b, c) => a.domain_axes() | b.domain_axes() | c.domain_axes(),
            D::RangeChoice {
                input,
                when_in_range,
                when_out_of_range,
                ..
            } => {
                input.domain_axes() | when_in_range.domain_axes() | when_out_of_range.domain_axes()
            }
            D::IntervalSelect {
                input, functions, ..
            } => functions
                .iter()
                .fold(input.domain_axes(), |acc, f| acc | f.domain_axes()),
            D::Slice { axis, input, .. } => input.domain_axes() & !axes_from(*axis),
            D::FindTopSurface {
                density,
                upper_bound,
                ..
            } => (density.domain_axes() | upper_bound.domain_axes()) & !AXIS_Y,
            D::OldBlendedNoise(_) => ALL_AXES,
            D::Simple(SimpleType::BlendAlpha | SimpleType::BlendOffset) => AXIS_X | AXIS_Z,
            D::Simple(SimpleType::Beardifier) => ALL_AXES,
            D::PreparedCache(p) => p.domain_axes,
        }
    }
}

#[cfg(test)]
mod java_round_tests {
    use super::java_round;

    #[test]
    fn rounds_half_up_like_java() {
        for (input, expected) in [
            (0.0, 0),
            (0.49999997, 0),
            (0.5, 1),
            (1.5, 2),
            (2.5, 3),
            (-0.5, 0),
            (-1.5, -1),
            (-2.5, -2),
            (-0.7, -1),
            (3.4, 3),
            (8_388_609.0, 8_388_609),
            (1e10, i32::MAX),
            (-1e10, i32::MIN),
            (f32::NAN, 0),
        ] {
            assert_eq!(java_round(input), expected, "{input}");
        }
    }
}
