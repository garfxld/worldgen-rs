use std::sync::Arc;

use shulkr::util::Axis;

use crate::world::worldgen::backend::jit::Slots;

use crate::world::worldgen::synth::{Noise, SimplexNoise};

use super::function::{DistanceMetric, RoundType, SimpleType};
use std::fmt;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::ops::Deref;

pub type S = Arc<SamplerNode>;

pub struct SamplerNode {
    pub sampler: Sampler,
    pub jit: Slots,
}

impl Deref for SamplerNode {
    type Target = Sampler;

    fn deref(&self) -> &Sampler {
        &self.sampler
    }
}

impl Debug for SamplerNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.sampler.fmt(f)
    }
}

pub fn node(sampler: Sampler) -> S {
    Arc::new(SamplerNode {
        sampler,
        jit: Slots::default(),
    })
}

#[derive(Debug, Clone, Copy)]
pub enum GradientTiling {
    Clamped { min: i32, max: i32 },
    Repeat { range: i32 },
    MirroredRepeat { range: i32 },
}

#[derive(Debug)]
pub enum SplineNode {
    Constant(f32),
    Multipoint {
        coordinate: usize,
        locations: Vec<f32>,
        values: Vec<SplineNode>,
        derivatives: Vec<f32>,
    },
}

#[derive(Debug)]
pub enum Sampler {
    Constant(f32),
    Noise {
        noise: Arc<Noise>,
        xz_scale: f64,
        y_scale: f64,
    },
    ShiftedXz {
        shift_x: S,
        shift_z: S,
        noise: Arc<Noise>,
        xz_scale: f64,
        y_scale: f64,
    },
    ShiftedXyz {
        shift_x: S,
        shift_y: S,
        shift_z: S,
        noise: Arc<Noise>,
        xz_scale: f64,
        y_scale: f64,
    },
    ShiftB(Arc<Noise>),
    EndIsland(Arc<SimplexNoise>),
    DistanceToPoint {
        point: [i32; 3],
        metric: DistanceMetric,
    },
    Gradient {
        axis: Axis,
        tiling: GradientTiling,
        from_coordinate: i32,
        from_value: f32,
        coordinate_factor: f32,
    },
    Abs(S),
    Square(S),
    Cube(S),
    Sqrt(S),
    LeakyRelu(S, f32),
    Reciprocal(S),
    Negate(S),
    Squeeze(S),
    Log(S),
    Sign(S),
    RoundInteger(RoundType, S),
    Round(RoundType, S, S),
    Add(S, S),
    ConstAdd(S, f32),
    Sub(S, S),
    ConstSub(f32, S),
    Mul(S, S),
    ConstMul(S, f32),
    Div(S, S),
    ConstDiv(f32, S),
    Min(S, S, f32),
    ConstMin(S, f32),
    Max(S, S, f32),
    ConstMax(S, f32),
    PowConstBase(f64, S),
    PowConstExponent(S, f64),
    Pow(S, S),
    Spline {
        root: SplineNode,
        coordinates: Vec<S>,
    },
    LerpConstFirst(S, f32, S),
    LerpConstSecond(S, S, f32),
    Lerp(S, S, S),
    Clamp(S, f32, f32),
    RangeChoiceConst {
        input: S,
        min_inclusive: f32,
        max_exclusive: f32,
        when_in_range: f32,
        when_out_of_range: f32,
    },
    RangeChoice {
        input: S,
        min_inclusive: f32,
        max_exclusive: f32,
        when_in_range: S,
        when_out_of_range: S,
    },
    IntervalSelect {
        input: S,
        thresholds: Vec<f32>,
        samplers: Vec<S>,
    },
    IntervalSelectSingle {
        input: S,
        threshold: f32,
        if_below: S,
        if_above: S,
    },
    Caching {
        id: usize,
        input: S,
    },
    ContextBound {
        key: SimpleType,
        fallback: f32,
    },
    BlendDensity(S),
    Interpolated {
        input: S,
        cell_size_xz: i32,
        cell_size_y: i32,
        cell_size_xz_inv: f32,
        cell_size_y_inv: f32,
    },
    SliceX(S, i32),
    SliceY(S, i32),
    SliceZ(S, i32),
    SliceXz(S, i32, i32),
    FindTopSurface {
        density: S,
        upper_bound: S,
        lower_bound: i32,
        cell_height: i32,
    },
}
