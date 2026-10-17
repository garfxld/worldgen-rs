use cranelift_codegen::ir::condcodes::FloatCC;
use cranelift_codegen::ir::{BlockArg, InstBuilder, Value, types};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_module::Module;
use shulkr::util::Axis;

use crate::world::worldgen::densityfunction::function::{DistanceMetric, RoundType, SimpleType};
use crate::world::worldgen::densityfunction::sampler::{GradientTiling, Sampler, SamplerNode};

use super::ValueFn;
use super::emit::Emitter;
use super::module::Jit;
use super::runtime;
use crate::world::worldgen::densityfunction::sampler::S;
use std::mem;
use std::ptr;
use std::sync;

pub const fn is_boundary(sampler: &Sampler) -> bool {
    matches!(
        sampler,
        Sampler::Caching { .. }
            | Sampler::Interpolated { .. }
            | Sampler::FindTopSurface { .. }
            | Sampler::Spline { .. }
            | Sampler::ContextBound {
                key: SimpleType::Beardifier,
                ..
            }
    )
}

pub fn compile(jit: &mut Jit, node: &SamplerNode) -> Option<ValueFn> {
    if is_boundary(&node.sampler) {
        return None;
    }
    let ptr = jit.pointer_type();
    jit.context.func.signature =
        jit.signature(&[ptr, types::I32, types::I32, types::I32], &[types::F32]);
    let frontend_config = jit.module.target_config();
    let mut builder_context = FunctionBuilderContext::new();
    {
        let mut b = FunctionBuilder::new(&mut jit.context.func, &mut builder_context);
        let entry = b.create_block();
        b.append_block_params_for_function_params(entry);
        b.switch_to_block(entry);
        let params = b.block_params(entry).to_vec();
        let mut emitter = ValueEmitter {
            e: Emitter {
                b: &mut b,
                ptr,
                call_conv: jit.call_conv,
            },
            ctx: params[0],
        };
        let result = emitter.emit(node, params[1], params[2], params[3]);
        b.ins().return_(&[result]);
        b.seal_all_blocks();
        b.finalize(frontend_config);
    }
    let code = jit.finish()?;
    // SAFETY: `code` was just compiled from a signature matching `ValueFn`.
    Some(unsafe { mem::transmute::<*const u8, ValueFn>(code) })
}

struct ValueEmitter<'a, 'f> {
    e: Emitter<'a, 'f>,
    ctx: Value,
}

impl ValueEmitter<'_, '_> {
    fn boundary(&mut self, node: &SamplerNode, x: Value, y: Value, z: Value) -> Value {
        let ptr = self.e.ptr;
        let node_ptr = self.e.address(ptr::from_ref(node));
        self.e
            .call(
                runtime::value_boundary as *const u8,
                &[ptr, ptr, types::I32, types::I32, types::I32],
                &[types::F32],
                &[node_ptr, self.ctx, x, y, z],
            )
            .expect("boundary returns f32")
    }

    fn branch(
        &mut self,
        condition: Value,
        then: impl FnOnce(&mut Self) -> Value,
        otherwise: impl FnOnce(&mut Self) -> Value,
    ) -> Value {
        let then_block = self.e.b.create_block();
        let else_block = self.e.b.create_block();
        let join = self.e.b.create_block();
        self.e.b.append_block_param(join, types::F32);
        self.e
            .b
            .ins()
            .brif(condition, then_block, &[], else_block, &[]);
        self.e.b.switch_to_block(then_block);
        let a = then(self);
        self.e.b.ins().jump(join, &[BlockArg::Value(a)]);
        self.e.b.switch_to_block(else_block);
        let c = otherwise(self);
        self.e.b.ins().jump(join, &[BlockArg::Value(c)]);
        self.e.b.switch_to_block(join);
        self.e.b.block_params(join)[0]
    }

    fn emit(&mut self, node: &SamplerNode, x: Value, y: Value, z: Value) -> Value {
        let sampler = &node.sampler;
        if is_boundary(sampler) {
            return self.boundary(node, x, y, z);
        }
        match sampler {
            Sampler::Constant(v) => self.e.f32(*v),
            Sampler::Noise {
                noise,
                xz_scale,
                y_scale,
            } => {
                let nx = self.scaled(x, *xz_scale);
                let ny = self.scaled(y, *y_scale);
                let nz = self.scaled(z, *xz_scale);
                self.e.noise_get(noise, nx, ny, nz)
            }
            Sampler::ShiftedXz {
                shift_x,
                shift_z,
                noise,
                xz_scale,
                y_scale,
            } => {
                let sx = self.emit(shift_x, x, y, z);
                let nx = self.shifted(x, *xz_scale, sx);
                let ny = self.scaled(y, *y_scale);
                let sz = self.emit(shift_z, x, y, z);
                let nz = self.shifted(z, *xz_scale, sz);
                self.e.noise_get(noise, nx, ny, nz)
            }
            Sampler::ShiftedXyz {
                shift_x,
                shift_y,
                shift_z,
                noise,
                xz_scale,
                y_scale,
            } => {
                let sx = self.emit(shift_x, x, y, z);
                let nx = self.shifted(x, *xz_scale, sx);
                let sy = self.emit(shift_y, x, y, z);
                let ny = self.shifted(y, *y_scale, sy);
                let sz = self.emit(shift_z, x, y, z);
                let nz = self.shifted(z, *xz_scale, sz);
                self.e.noise_get(noise, nx, ny, nz)
            }
            Sampler::ShiftB(noise) => {
                let nx = self.scaled(z, 0.25);
                let ny = self.scaled(x, 0.25);
                let nz = self.e.f64(0.0);
                let value = self.e.noise_get(noise, nx, ny, nz);
                let four = self.e.f32(4.0);
                self.e.b.ins().fmul(value, four)
            }
            Sampler::EndIsland(noise) => {
                let ptr = self.e.ptr;
                let noise_ptr = self.e.address(sync::Arc::as_ptr(noise));
                self.e
                    .call(
                        runtime::end_island as *const u8,
                        &[ptr, types::I32, types::I32],
                        &[types::F32],
                        &[noise_ptr, x, z],
                    )
                    .expect("end_island returns f32")
            }
            Sampler::DistanceToPoint { point, metric } => {
                let dx = self.delta(point[0], x);
                let dy = self.delta(point[1], y);
                let dz = self.delta(point[2], z);
                self.distance(*metric, dx, dy, dz)
            }
            Sampler::Gradient {
                axis,
                tiling,
                from_coordinate,
                from_value,
                coordinate_factor,
            } => {
                let coordinate = match axis {
                    Axis::X => x,
                    Axis::Y => y,
                    Axis::Z => z,
                };
                match tiling {
                    GradientTiling::Clamped { min, max } => {
                        let min = self.e.i32(*min);
                        let max = self.e.i32(*max);
                        let clamped = self.e.b.ins().smax(coordinate, min);
                        let clamped = self.e.b.ins().smin(clamped, max);
                        let relative = self
                            .e
                            .b
                            .ins()
                            .iadd_imm_s(clamped, -i64::from(*from_coordinate));
                        let relative = self.e.int_to_f32(relative);
                        let factor = self.e.f32(*coordinate_factor);
                        let scaled = self.e.b.ins().fmul(relative, factor);
                        let from = self.e.f32(*from_value);
                        self.e.b.ins().fadd(from, scaled)
                    }
                    GradientTiling::Repeat { .. } | GradientTiling::MirroredRepeat { .. } => {
                        let ptr = self.e.ptr;
                        let node_ptr = self.e.address(ptr::from_ref(node));
                        self.e
                            .call(
                                runtime::gradient as *const u8,
                                &[ptr, types::I32],
                                &[types::F32],
                                &[node_ptr, coordinate],
                            )
                            .expect("gradient returns f32")
                    }
                }
            }
            Sampler::Abs(input) => {
                let v = self.emit(input, x, y, z);
                self.e.b.ins().fabs(v)
            }
            Sampler::Square(input) => {
                let v = self.emit(input, x, y, z);
                self.e.b.ins().fmul(v, v)
            }
            Sampler::Cube(input) => {
                let v = self.emit(input, x, y, z);
                let square = self.e.b.ins().fmul(v, v);
                self.e.b.ins().fmul(square, v)
            }
            Sampler::Sqrt(input) => {
                let v = self.emit(input, x, y, z);
                self.e.sqrt_f32(v)
            }
            Sampler::LeakyRelu(input, factor) => {
                let v = self.emit(input, x, y, z);
                self.leaky_relu(v, *factor)
            }
            Sampler::Reciprocal(input) => {
                let v = self.emit(input, x, y, z);
                let one = self.e.f32(1.0);
                self.e.b.ins().fdiv(one, v)
            }
            Sampler::Negate(input) => {
                let v = self.emit(input, x, y, z);
                self.e.b.ins().fneg(v)
            }
            Sampler::Squeeze(input) => {
                let v = self.emit(input, x, y, z);
                self.squeeze(v)
            }
            Sampler::Log(input) => {
                let v = self.emit(input, x, y, z);
                let wide = self.e.promote(v);
                let result = self
                    .e
                    .call(
                        runtime::ln as *const u8,
                        &[types::F64],
                        &[types::F64],
                        &[wide],
                    )
                    .expect("ln returns f64");
                self.e.demote(result)
            }
            Sampler::Sign(input) => {
                let v = self.emit(input, x, y, z);
                self.signum(v)
            }
            Sampler::RoundInteger(kind, input) => {
                let v = self.emit(input, x, y, z);
                self.round_to_integer(*kind, v)
            }
            Sampler::Round(kind, input, multiple) => {
                let v = self.emit(input, x, y, z);
                let m = self.emit(multiple, x, y, z);
                let quotient = self.e.b.ins().fdiv(v, m);
                let rounded = self.round_to_integer(*kind, quotient);
                let product = self.e.b.ins().fmul(rounded, m);
                let zero = self.e.f32(0.0);
                let is_zero = self.e.b.ins().fcmp(FloatCC::Equal, m, zero);
                self.e.b.ins().select(is_zero, v, product)
            }
            Sampler::Add(l, r) => {
                let a = self.emit(l, x, y, z);
                let c = self.emit(r, x, y, z);
                self.e.b.ins().fadd(a, c)
            }
            Sampler::ConstAdd(l, c) => {
                let a = self.emit(l, x, y, z);
                let c = self.e.f32(*c);
                self.e.b.ins().fadd(a, c)
            }
            Sampler::Sub(l, r) => {
                let a = self.emit(l, x, y, z);
                let c = self.emit(r, x, y, z);
                self.e.b.ins().fsub(a, c)
            }
            Sampler::ConstSub(c, r) => {
                let v = self.emit(r, x, y, z);
                let c = self.e.f32(*c);
                self.e.b.ins().fsub(c, v)
            }
            Sampler::Mul(l, r) | Sampler::Div(l, r) => {
                let divide = matches!(sampler, Sampler::Div(..));
                let left = self.emit(l, x, y, z);
                let zero = self.e.f32(0.0);
                let is_zero = self.e.b.ins().fcmp(FloatCC::Equal, left, zero);
                self.branch(
                    is_zero,
                    |s| s.e.f32(0.0),
                    |s| {
                        let right = s.emit(r, x, y, z);
                        if divide {
                            s.e.b.ins().fdiv(left, right)
                        } else {
                            s.e.b.ins().fmul(left, right)
                        }
                    },
                )
            }
            Sampler::ConstMul(l, c) => {
                let a = self.emit(l, x, y, z);
                let c = self.e.f32(*c);
                self.e.b.ins().fmul(a, c)
            }
            Sampler::ConstDiv(c, r) => {
                let v = self.emit(r, x, y, z);
                let c = self.e.f32(*c);
                self.e.b.ins().fdiv(c, v)
            }
            Sampler::Min(l, r, bound) => {
                let left = self.emit(l, x, y, z);
                let bound = self.e.f32(*bound);
                let short = self.e.b.ins().fcmp(FloatCC::LessThanOrEqual, left, bound);
                self.branch(
                    short,
                    |_| left,
                    |s| {
                        let right = s.emit(r, x, y, z);
                        s.e.java_min(left, right)
                    },
                )
            }
            Sampler::Max(l, r, bound) => {
                let left = self.emit(l, x, y, z);
                let bound = self.e.f32(*bound);
                let short = self
                    .e
                    .b
                    .ins()
                    .fcmp(FloatCC::GreaterThanOrEqual, left, bound);
                self.branch(
                    short,
                    |_| left,
                    |s| {
                        let right = s.emit(r, x, y, z);
                        s.e.java_max(left, right)
                    },
                )
            }
            Sampler::ConstMin(l, c) => {
                let v = self.emit(l, x, y, z);
                let c = self.e.f32(*c);
                self.e.java_min(v, c)
            }
            Sampler::ConstMax(l, c) => {
                let v = self.emit(l, x, y, z);
                let c = self.e.f32(*c);
                self.e.java_max(v, c)
            }
            Sampler::PowConstBase(base, exponent) => {
                let v = self.emit(exponent, x, y, z);
                let base = self.e.f64(*base);
                let exponent = self.e.promote(v);
                self.pow(base, exponent)
            }
            Sampler::PowConstExponent(base, exponent) => {
                let v = self.emit(base, x, y, z);
                let base = self.e.promote(v);
                let exponent = self.e.f64(*exponent);
                self.pow(base, exponent)
            }
            Sampler::Pow(base, exponent) => {
                let b = self.emit(base, x, y, z);
                let e = self.emit(exponent, x, y, z);
                let b = self.e.promote(b);
                let e = self.e.promote(e);
                self.pow(b, e)
            }
            Sampler::LerpConstFirst(alpha, first, second) => {
                let a = self.emit(alpha, x, y, z);
                let first = *first;
                let zero = self.e.f32(0.0);
                let is_zero = self.e.b.ins().fcmp(FloatCC::Equal, a, zero);
                self.branch(
                    is_zero,
                    |s| s.e.f32(first),
                    |s| {
                        let second = s.emit(second, x, y, z);
                        let one = s.e.f32(1.0);
                        let is_one = s.e.b.ins().fcmp(FloatCC::Equal, a, one);
                        let first = s.e.f32(first);
                        let mixed = s.e.lerp(a, first, second);
                        s.e.b.ins().select(is_one, second, mixed)
                    },
                )
            }
            Sampler::LerpConstSecond(alpha, first, second) => {
                let a = self.emit(alpha, x, y, z);
                let second = *second;
                let one = self.e.f32(1.0);
                let is_one = self.e.b.ins().fcmp(FloatCC::Equal, a, one);
                let zero = self.e.f32(0.0);
                let is_zero = self.e.b.ins().fcmp(FloatCC::Equal, a, zero);
                let not_zero = self.e.b.ins().bxor_imm_u(is_zero, 1);
                let constant = self.e.b.ins().band(is_one, not_zero);
                self.branch(
                    constant,
                    |s| s.e.f32(second),
                    |s| {
                        let first = s.emit(first, x, y, z);
                        let second = s.e.f32(second);
                        let mixed = s.e.lerp(a, first, second);
                        s.e.b.ins().select(is_zero, first, mixed)
                    },
                )
            }
            Sampler::Lerp(alpha, first, second) => {
                let a = self.emit(alpha, x, y, z);
                let zero = self.e.f32(0.0);
                let is_zero = self.e.b.ins().fcmp(FloatCC::Equal, a, zero);
                self.branch(
                    is_zero,
                    |s| s.emit(first, x, y, z),
                    |s| {
                        let one = s.e.f32(1.0);
                        let is_one = s.e.b.ins().fcmp(FloatCC::Equal, a, one);
                        s.branch(
                            is_one,
                            |s| s.emit(second, x, y, z),
                            |s| {
                                let f = s.emit(first, x, y, z);
                                let sv = s.emit(second, x, y, z);
                                s.e.lerp(a, f, sv)
                            },
                        )
                    },
                )
            }
            Sampler::Clamp(input, min, max) => {
                let v = self.emit(input, x, y, z);
                self.e.clamp(v, *min, *max)
            }
            Sampler::RangeChoiceConst {
                input,
                min_inclusive,
                max_exclusive,
                when_in_range,
                when_out_of_range,
            } => {
                let v = self.emit(input, x, y, z);
                let in_range = self.in_range(v, *min_inclusive, *max_exclusive);
                let a = self.e.f32(*when_in_range);
                let c = self.e.f32(*when_out_of_range);
                self.e.b.ins().select(in_range, a, c)
            }
            Sampler::RangeChoice {
                input,
                min_inclusive,
                max_exclusive,
                when_in_range,
                when_out_of_range,
            } => {
                let v = self.emit(input, x, y, z);
                let in_range = self.in_range(v, *min_inclusive, *max_exclusive);
                self.branch(
                    in_range,
                    |s| s.emit(when_in_range, x, y, z),
                    |s| s.emit(when_out_of_range, x, y, z),
                )
            }
            Sampler::IntervalSelect {
                input,
                thresholds,
                samplers,
            } => {
                let v = self.emit(input, x, y, z);
                self.interval_select(v, thresholds, samplers, x, y, z)
            }
            Sampler::IntervalSelectSingle {
                input,
                threshold,
                if_below,
                if_above,
            } => {
                let v = self.emit(input, x, y, z);
                let threshold = self.e.f32(*threshold);
                let below = self.e.b.ins().fcmp(FloatCC::LessThan, v, threshold);
                self.branch(
                    below,
                    |s| s.emit(if_below, x, y, z),
                    |s| s.emit(if_above, x, y, z),
                )
            }
            Sampler::ContextBound { fallback, .. } => self.e.f32(*fallback),
            Sampler::BlendDensity(input) => self.emit(input, x, y, z),
            Sampler::SliceX(input, sx) => {
                let sx = self.e.i32(*sx);
                self.emit(input, sx, y, z)
            }
            Sampler::SliceY(input, sy) => {
                let sy = self.e.i32(*sy);
                self.emit(input, x, sy, z)
            }
            Sampler::SliceZ(input, sz) => {
                let sz = self.e.i32(*sz);
                self.emit(input, x, y, sz)
            }
            Sampler::SliceXz(input, sx, sz) => {
                let sx = self.e.i32(*sx);
                let sz = self.e.i32(*sz);
                self.emit(input, sx, y, sz)
            }
            Sampler::Caching { .. }
            | Sampler::Interpolated { .. }
            | Sampler::FindTopSurface { .. }
            | Sampler::Spline { .. } => self.boundary(node, x, y, z),
        }
    }

    fn interval_select(
        &mut self,
        v: Value,
        thresholds: &[f32],
        samplers: &[S],
        x: Value,
        y: Value,
        z: Value,
    ) -> Value {
        match thresholds.split_first() {
            Some((threshold, rest)) if samplers.len() > 1 => {
                let threshold = self.e.f32(*threshold);
                let below = self.e.b.ins().fcmp(FloatCC::LessThan, v, threshold);
                self.branch(
                    below,
                    |s| s.emit(&samplers[0], x, y, z),
                    |s| s.interval_select(v, rest, &samplers[1..], x, y, z),
                )
            }
            _ => self.emit(&samplers[samplers.len() - 1], x, y, z),
        }
    }

    fn scaled(&mut self, coordinate: Value, scale: f64) -> Value {
        let wide = self.e.int_to_f64(coordinate);
        let scale = self.e.f64(scale);
        self.e.b.ins().fmul(wide, scale)
    }

    fn shifted(&mut self, coordinate: Value, scale: f64, shift: Value) -> Value {
        let scaled = self.scaled(coordinate, scale);
        let shift = self.e.promote(shift);
        self.e.b.ins().fadd(scaled, shift)
    }

    fn delta(&mut self, target: i32, coordinate: Value) -> Value {
        let target = self.e.i32(target);
        let delta = self.e.b.ins().isub(target, coordinate);
        self.e.int_to_f32(delta)
    }

    fn distance(&mut self, metric: DistanceMetric, dx: Value, dy: Value, dz: Value) -> Value {
        match metric {
            DistanceMetric::Euclidean | DistanceMetric::EuclideanSquared => {
                let xx = self.e.b.ins().fmul(dx, dx);
                let yy = self.e.b.ins().fmul(dy, dy);
                let sum = self.e.b.ins().fadd(xx, yy);
                let zz = self.e.b.ins().fmul(dz, dz);
                let sum = self.e.b.ins().fadd(sum, zz);
                if metric == DistanceMetric::Euclidean {
                    self.e.sqrt_f32(sum)
                } else {
                    sum
                }
            }
            DistanceMetric::Manhattan => {
                let ax = self.e.b.ins().fabs(dx);
                let ay = self.e.b.ins().fabs(dy);
                let az = self.e.b.ins().fabs(dz);
                let sum = self.e.b.ins().fadd(ax, ay);
                self.e.b.ins().fadd(sum, az)
            }
            DistanceMetric::Chebyshev => {
                let ax = self.e.b.ins().fabs(dx);
                let ay = self.e.b.ins().fabs(dy);
                let az = self.e.b.ins().fabs(dz);
                let m = self.e.java_max(ax, ay);
                self.e.java_max(m, az)
            }
        }
    }

    fn leaky_relu(&mut self, v: Value, factor: f32) -> Value {
        let zero = self.e.f32(0.0);
        let positive = self.e.b.ins().fcmp(FloatCC::GreaterThan, v, zero);
        let factor = self.e.f32(factor);
        let scaled = self.e.b.ins().fmul(v, factor);
        self.e.b.ins().select(positive, v, scaled)
    }

    fn squeeze(&mut self, v: Value) -> Value {
        let c = self.e.clamp(v, -1.0, 1.0);
        let two = self.e.f32(2.0);
        let half = self.e.b.ins().fdiv(c, two);
        let square = self.e.b.ins().fmul(c, c);
        let cube = self.e.b.ins().fmul(square, c);
        let twenty_four = self.e.f32(24.0);
        let part = self.e.b.ins().fdiv(cube, twenty_four);
        self.e.b.ins().fsub(half, part)
    }

    fn signum(&mut self, v: Value) -> Value {
        let zero = self.e.f32(0.0);
        let is_zero = self.e.b.ins().fcmp(FloatCC::Equal, v, zero);
        let is_nan = self.e.b.ins().fcmp(FloatCC::Unordered, v, v);
        let keep = self.e.b.ins().bor(is_zero, is_nan);
        let one = self.e.f32(1.0);
        let signed = self.e.b.ins().fcopysign(one, v);
        self.e.b.ins().select(keep, v, signed)
    }

    fn round_to_integer(&mut self, kind: RoundType, v: Value) -> Value {
        match kind {
            RoundType::Floor => {
                let wide = self.e.promote(v);
                let r = self.e.b.ins().floor(wide);
                self.e.demote(r)
            }
            RoundType::Ceil => {
                let wide = self.e.promote(v);
                let r = self.e.b.ins().ceil(wide);
                self.e.demote(r)
            }
            RoundType::Truncate => {
                let wide = self.e.promote(v);
                let floor = self.e.b.ins().floor(wide);
                let ceil = self.e.b.ins().ceil(wide);
                let zero = self.e.f32(0.0);
                let positive = self.e.b.ins().fcmp(FloatCC::GreaterThan, v, zero);
                let r = self.e.b.ins().select(positive, floor, ceil);
                self.e.demote(r)
            }
            RoundType::Round => self
                .e
                .call(
                    runtime::round as *const u8,
                    &[types::F32],
                    &[types::F32],
                    &[v],
                )
                .expect("round returns f32"),
        }
    }

    fn pow(&mut self, base: Value, exponent: Value) -> Value {
        let result = self
            .e
            .call(
                runtime::pow as *const u8,
                &[types::F64, types::F64],
                &[types::F64],
                &[base, exponent],
            )
            .expect("pow returns f64");
        self.e.demote(result)
    }

    fn in_range(&mut self, v: Value, min_inclusive: f32, max_exclusive: f32) -> Value {
        let min = self.e.f32(min_inclusive);
        let max = self.e.f32(max_exclusive);
        let ge = self.e.b.ins().fcmp(FloatCC::GreaterThanOrEqual, v, min);
        let lt = self.e.b.ins().fcmp(FloatCC::LessThan, v, max);
        self.e.b.ins().band(ge, lt)
    }
}
