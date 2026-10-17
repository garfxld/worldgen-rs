use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{BlockArg, InstBuilder, MemFlagsData, Value, types};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_module::Module;
use rustc_hash::FxHashMap;
use shulkr::util::Axis;

use crate::world::worldgen::densityfunction::function::{DistanceMetric, RoundType, SimpleType};
use crate::world::worldgen::densityfunction::sampler::{GradientTiling, S, Sampler, SamplerNode};
use crate::world::worldgen::synth::Noise;

use super::emit::Emitter;
use super::module::Jit;
use super::runtime;
use crate::world::worldgen::synth::gradient_noise::GradientNoise;
use std::iter;
use std::mem;
use std::ptr;
use std::sync;

pub type KernelFn =
    unsafe extern "C" fn(*mut f32, *const *const f32, i32, i32, i32, i32, i32, i32, i32, i32, i32);

pub struct VolumeKernel {
    pub code: KernelFn,
    pub boundaries: Vec<S>,
}

pub fn is_boundary(sampler: &Sampler) -> bool {
    match sampler {
        Sampler::Caching { .. }
        | Sampler::Interpolated { .. }
        | Sampler::SliceX(..)
        | Sampler::SliceY(..)
        | Sampler::SliceZ(..)
        | Sampler::SliceXz(..)
        | Sampler::FindTopSurface { .. }
        | Sampler::EndIsland(_)
        | Sampler::Spline { .. } => true,
        Sampler::ContextBound { key, .. } => *key == SimpleType::Beardifier,
        _ => false,
    }
}

fn children_in_volume_order(sampler: &Sampler) -> Vec<&S> {
    match sampler {
        Sampler::ShiftedXz {
            shift_x, shift_z, ..
        } => vec![shift_x, shift_z],
        Sampler::ShiftedXyz {
            shift_x,
            shift_y,
            shift_z,
            ..
        } => vec![shift_x, shift_y, shift_z],
        Sampler::Abs(i)
        | Sampler::Square(i)
        | Sampler::Cube(i)
        | Sampler::Sqrt(i)
        | Sampler::LeakyRelu(i, _)
        | Sampler::Reciprocal(i)
        | Sampler::Negate(i)
        | Sampler::Squeeze(i)
        | Sampler::Log(i)
        | Sampler::Sign(i)
        | Sampler::RoundInteger(_, i)
        | Sampler::ConstAdd(i, _)
        | Sampler::ConstSub(_, i)
        | Sampler::ConstMul(i, _)
        | Sampler::ConstDiv(_, i)
        | Sampler::ConstMin(i, _)
        | Sampler::ConstMax(i, _)
        | Sampler::PowConstBase(_, i)
        | Sampler::PowConstExponent(i, _)
        | Sampler::Clamp(i, _, _)
        | Sampler::RangeChoiceConst { input: i, .. }
        | Sampler::BlendDensity(i) => vec![i],
        Sampler::Round(_, i, m) => vec![i, m],
        Sampler::Add(l, r)
        | Sampler::Sub(l, r)
        | Sampler::Mul(l, r)
        | Sampler::Div(l, r)
        | Sampler::Min(l, r, _)
        | Sampler::Max(l, r, _)
        | Sampler::Pow(l, r) => vec![l, r],
        Sampler::LerpConstFirst(alpha, _, second) => vec![alpha, second],
        Sampler::LerpConstSecond(alpha, first, _) => vec![alpha, first],
        Sampler::Lerp(alpha, first, second) => vec![alpha, first, second],
        Sampler::RangeChoice {
            input,
            when_in_range,
            when_out_of_range,
            ..
        } => vec![when_in_range, input, when_out_of_range],
        Sampler::IntervalSelect {
            input, samplers, ..
        } => iter::once(input).chain(samplers.iter()).collect(),
        Sampler::IntervalSelectSingle {
            input,
            if_below,
            if_above,
            ..
        } => vec![input, if_below, if_above],
        _ => Vec::new(),
    }
}

fn collect_boundaries(node: &S, out: &mut Vec<S>) {
    if is_boundary(&node.sampler) {
        out.push(node.clone());
        return;
    }
    for child in children_in_volume_order(&node.sampler) {
        collect_boundaries(child, out);
    }
}

pub fn compile(jit: &mut Jit, node: &SamplerNode) -> Option<VolumeKernel> {
    if is_boundary(&node.sampler) {
        return None;
    }
    let mut boundaries = Vec::new();
    for child in children_in_volume_order(&node.sampler) {
        collect_boundaries(child, &mut boundaries);
    }
    let mut inputs: FxHashMap<*const SamplerNode, usize> = FxHashMap::default();
    for (index, boundary) in boundaries.iter().enumerate() {
        inputs.entry(sync::Arc::as_ptr(boundary)).or_insert(index);
    }

    let ptr = jit.pointer_type();
    let i32t = types::I32;
    jit.context.func.signature = jit.signature(
        &[
            ptr, ptr, i32t, i32t, i32t, i32t, i32t, i32t, i32t, i32t, i32t,
        ],
        &[],
    );
    let frontend_config = jit.module.target_config();
    let mut builder_context = FunctionBuilderContext::new();
    {
        let mut b = FunctionBuilder::new(&mut jit.context.func, &mut builder_context);
        let entry = b.create_block();
        b.append_block_params_for_function_params(entry);
        b.switch_to_block(entry);
        let params = b.block_params(entry).to_vec();
        let [
            out,
            inputs_ptr,
            size_x,
            size_y,
            size_z,
            min_x,
            min_y,
            min_z,
            step_x,
            step_y,
            step_z,
        ] = params[..]
        else {
            return None;
        };
        let flags = MemFlagsData::trusted().with_readonly();
        let mut input_pointers = Vec::with_capacity(boundaries.len());
        for index in 0..boundaries.len() {
            let pointer = b.ins().load(ptr, flags, inputs_ptr, (index * 8) as i32);
            input_pointers.push(pointer);
        }

        let z_head = b.create_block();
        let z_body = b.create_block();
        let x_head = b.create_block();
        let x_body = b.create_block();
        let y_head = b.create_block();
        let y_body = b.create_block();
        let x_next = b.create_block();
        let z_next = b.create_block();
        let exit = b.create_block();
        b.append_block_param(z_head, i32t);
        b.append_block_param(z_head, ptr);
        b.append_block_param(x_head, i32t);
        b.append_block_param(x_head, ptr);
        b.append_block_param(y_head, i32t);
        b.append_block_param(y_head, ptr);
        b.append_block_param(x_next, ptr);
        b.append_block_param(z_next, ptr);

        let zero = b.ins().iconst(i32t, 0);
        let zero_index = b.ins().iconst(ptr, 0);
        b.ins().jump(
            z_head,
            &[BlockArg::Value(zero), BlockArg::Value(zero_index)],
        );

        b.switch_to_block(z_head);
        let zi = b.block_params(z_head)[0];
        let z_index = b.block_params(z_head)[1];
        let z_more = b.ins().icmp(IntCC::SignedLessThan, zi, size_z);
        b.ins().brif(z_more, z_body, &[], exit, &[]);

        b.switch_to_block(z_body);
        let z_offset = b.ins().imul(zi, step_z);
        let block_z = b.ins().iadd(min_z, z_offset);
        let zero = b.ins().iconst(i32t, 0);
        b.ins()
            .jump(x_head, &[BlockArg::Value(zero), BlockArg::Value(z_index)]);

        b.switch_to_block(x_head);
        let xi = b.block_params(x_head)[0];
        let x_index = b.block_params(x_head)[1];
        let x_more = b.ins().icmp(IntCC::SignedLessThan, xi, size_x);
        b.ins()
            .brif(x_more, x_body, &[], z_next, &[BlockArg::Value(x_index)]);

        b.switch_to_block(x_body);
        let x_offset = b.ins().imul(xi, step_x);
        let block_x = b.ins().iadd(min_x, x_offset);
        let zero = b.ins().iconst(i32t, 0);
        b.ins()
            .jump(y_head, &[BlockArg::Value(zero), BlockArg::Value(x_index)]);

        b.switch_to_block(y_head);
        let yi = b.block_params(y_head)[0];
        let y_index = b.block_params(y_head)[1];
        let y_more = b.ins().icmp(IntCC::SignedLessThan, yi, size_y);
        b.ins()
            .brif(y_more, y_body, &[], x_next, &[BlockArg::Value(y_index)]);

        b.switch_to_block(y_body);
        let y_offset = b.ins().imul(yi, step_y);
        let block_y = b.ins().iadd(min_y, y_offset);
        let byte_offset = b.ins().ishl_imm_u(y_index, 2);
        let value = {
            let mut emitter = VolumeEmitter {
                e: Emitter {
                    b: &mut b,
                    ptr,
                    call_conv: jit.call_conv,
                },
                inputs: &inputs,
                input_pointers: &input_pointers,
                byte_offset,
            };
            emitter.emit(node, block_x, block_y, block_z)
        };
        let address = b.ins().iadd(out, byte_offset);
        b.ins().store(MemFlagsData::trusted(), value, address, 0);
        let next_yi = b.ins().iadd_imm_s(yi, 1);
        let next_index = b.ins().iadd_imm_s(y_index, 1);
        b.ins().jump(
            y_head,
            &[BlockArg::Value(next_yi), BlockArg::Value(next_index)],
        );

        b.switch_to_block(x_next);
        let index_after_y = b.block_params(x_next)[0];
        let next_xi = b.ins().iadd_imm_s(xi, 1);
        b.ins().jump(
            x_head,
            &[BlockArg::Value(next_xi), BlockArg::Value(index_after_y)],
        );

        b.switch_to_block(z_next);
        let index_after_x = b.block_params(z_next)[0];
        let next_zi = b.ins().iadd_imm_s(zi, 1);
        b.ins().jump(
            z_head,
            &[BlockArg::Value(next_zi), BlockArg::Value(index_after_x)],
        );

        b.switch_to_block(exit);
        b.ins().return_(&[]);
        b.seal_all_blocks();
        b.finalize(frontend_config);
    }
    let code = jit.finish()?;
    Some(VolumeKernel {
        // SAFETY: `code` was just compiled from a signature matching `KernelFn`.
        code: unsafe { mem::transmute::<*const u8, KernelFn>(code) },
        boundaries,
    })
}

struct VolumeEmitter<'a, 'f> {
    e: Emitter<'a, 'f>,
    inputs: &'a FxHashMap<*const SamplerNode, usize>,
    input_pointers: &'a [Value],
    byte_offset: Value,
}

impl VolumeEmitter<'_, '_> {
    fn input(&mut self, node: &SamplerNode) -> Value {
        let index = self.inputs[&ptr::from_ref(node)];
        let base = self.input_pointers[index];
        let address = self.e.b.ins().iadd(base, self.byte_offset);
        self.e.b.ins().load(
            types::F32,
            MemFlagsData::trusted().with_readonly(),
            address,
            0,
        )
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
            return self.input(node);
        }
        match sampler {
            Sampler::Constant(v) => self.e.f32(*v),
            Sampler::Noise {
                noise,
                xz_scale,
                y_scale,
            } => {
                let mut value = self.e.f32(0.0);
                self.add_noise(&mut value, noise, x, y, z, *xz_scale, *y_scale, 1.0);
                value
            }
            Sampler::ShiftedXz {
                shift_x,
                shift_z,
                noise,
                xz_scale,
                y_scale,
            } => {
                let sx = self.emit(shift_x, x, y, z);
                let sz = self.emit(shift_z, x, y, z);
                let nx = self.shifted(x, *xz_scale, sx);
                let ny = self.scaled(y, *y_scale);
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
                let sy = self.emit(shift_y, x, y, z);
                let sz = self.emit(shift_z, x, y, z);
                let nx = self.shifted(x, *xz_scale, sx);
                let ny = self.shifted(y, *y_scale, sy);
                let nz = self.shifted(z, *xz_scale, sz);
                self.e.noise_get(noise, nx, ny, nz)
            }
            Sampler::ShiftB(noise) => {
                let mut value = self.e.f32(0.0);
                let zero = self.e.i32(0);
                self.add_noise(&mut value, noise, z, x, zero, 0.25, 0.25, 4.0);
                value
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
                let zero = self.e.f32(0.0);
                let positive = self.e.b.ins().fcmp(FloatCC::GreaterThan, v, zero);
                let factor = self.e.f32(*factor);
                let scaled = self.e.b.ins().fmul(v, factor);
                self.e.b.ins().select(positive, v, scaled)
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
                let c = self.e.clamp(v, -1.0, 1.0);
                let two = self.e.f32(2.0);
                let half = self.e.b.ins().fdiv(c, two);
                let square = self.e.b.ins().fmul(c, c);
                let cube = self.e.b.ins().fmul(square, c);
                let twenty_four = self.e.f32(24.0);
                let part = self.e.b.ins().fdiv(cube, twenty_four);
                self.e.b.ins().fsub(half, part)
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
                let zero = self.e.f32(0.0);
                let is_zero = self.e.b.ins().fcmp(FloatCC::Equal, v, zero);
                let is_nan = self.e.b.ins().fcmp(FloatCC::Unordered, v, v);
                let keep = self.e.b.ins().bor(is_zero, is_nan);
                let one = self.e.f32(1.0);
                let signed = self.e.b.ins().fcopysign(one, v);
                self.e.b.ins().select(keep, v, signed)
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
            Sampler::Sub(l, r) => {
                let a = self.emit(l, x, y, z);
                let c = self.emit(r, x, y, z);
                let negated = self.e.b.ins().fneg(c);
                self.e.b.ins().fadd(a, negated)
            }
            Sampler::Mul(l, r) => {
                let a = self.emit(l, x, y, z);
                let c = self.emit(r, x, y, z);
                self.e.b.ins().fmul(a, c)
            }
            Sampler::Div(l, r) => {
                let a = self.emit(l, x, y, z);
                let c = self.emit(r, x, y, z);
                self.e.b.ins().fdiv(a, c)
            }
            Sampler::ConstAdd(l, c) => {
                let a = self.emit(l, x, y, z);
                let c = self.e.f32(*c);
                self.e.b.ins().fadd(a, c)
            }
            Sampler::ConstSub(c, r) => {
                let v = self.emit(r, x, y, z);
                let c = self.e.f32(*c);
                self.e.b.ins().fsub(c, v)
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
            Sampler::Min(l, r, _) => {
                let a = self.emit(l, x, y, z);
                let c = self.emit(r, x, y, z);
                let less = self.e.b.ins().fcmp(FloatCC::LessThan, c, a);
                self.e.b.ins().select(less, c, a)
            }
            Sampler::Max(l, r, _) => {
                let a = self.emit(l, x, y, z);
                let c = self.emit(r, x, y, z);
                let greater = self.e.b.ins().fcmp(FloatCC::GreaterThan, c, a);
                self.e.b.ins().select(greater, c, a)
            }
            Sampler::ConstMin(l, c) => {
                let v = self.emit(l, x, y, z);
                let c = self.e.f32(*c);
                let less = self.e.b.ins().fcmp(FloatCC::LessThan, c, v);
                self.e.b.ins().select(less, c, v)
            }
            Sampler::ConstMax(l, c) => {
                let v = self.emit(l, x, y, z);
                let c = self.e.f32(*c);
                let greater = self.e.b.ins().fcmp(FloatCC::GreaterThan, c, v);
                self.e.b.ins().select(greater, c, v)
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
            Sampler::Caching { .. }
            | Sampler::Interpolated { .. }
            | Sampler::SliceX(..)
            | Sampler::SliceY(..)
            | Sampler::SliceZ(..)
            | Sampler::SliceXz(..)
            | Sampler::FindTopSurface { .. }
            | Sampler::EndIsland(_)
            | Sampler::Spline { .. } => self.input(node),
        }
    }

    #[expect(clippy::too_many_arguments, reason = "mirrors Noise.addToVolume")]
    fn add_noise(
        &mut self,
        accumulator: &mut Value,
        noise: &Noise,
        x: Value,
        y: Value,
        z: Value,
        xz_scale: f64,
        y_scale: f64,
        amplitude: f32,
    ) {
        match noise {
            Noise::Stack(stack) => {
                for layer in &stack.layers {
                    self.add_noise(
                        accumulator,
                        &layer.noise,
                        x,
                        y,
                        z,
                        xz_scale * layer.frequency,
                        y_scale * layer.frequency,
                        amplitude * layer.amplitude,
                    );
                }
            }
            Noise::Perlin(perlin) => {
                let value = self.perlin_volume(&perlin.base, None, x, y, z, xz_scale, y_scale);
                self.accumulate(accumulator, amplitude, value);
            }
            Noise::SmearedPerlin(smeared) => {
                let value = self.perlin_volume(
                    &smeared.perlin.base,
                    Some(smeared.fudge_y_scale),
                    x,
                    y,
                    z,
                    xz_scale,
                    y_scale,
                );
                self.accumulate(accumulator, amplitude, value);
            }
            Noise::Simplex(_) => {
                let nx = self.scaled(x, xz_scale);
                let ny = self.scaled(y, y_scale);
                let nz = self.scaled(z, xz_scale);
                let value = self.e.noise_get(noise, nx, ny, nz);
                self.accumulate(accumulator, amplitude, value);
            }
        }
    }

    fn accumulate(&mut self, accumulator: &mut Value, amplitude: f32, value: Value) {
        let amplitude = self.e.f32(amplitude);
        let scaled = self.e.b.ins().fmul(amplitude, value);
        *accumulator = self.e.b.ins().fadd(*accumulator, scaled);
    }

    #[expect(clippy::too_many_arguments, reason = "mirrors PerlinNoise.addToVolume")]
    fn perlin_volume(
        &mut self,
        base: &GradientNoise,
        fudge_y_scale: Option<f64>,
        x: Value,
        y: Value,
        z: Value,
        xz_scale: f64,
        y_scale: f64,
    ) -> Value {
        let perms = self.e.address(base.perms.as_ptr());
        let (floor_z, relative_z) = self.volume_axis(z, xz_scale, base.offset_z);
        let alpha_z = self.e.smoothstep(relative_z);
        let (floor_x, relative_x) = self.volume_axis(x, xz_scale, base.offset_x);
        let x0 = self.e.permute(perms, floor_x);
        let x_plus = self.e.b.ins().iadd_imm_s(floor_x, 1);
        let x1 = self.e.permute(perms, x_plus);
        let alpha_x = self.e.smoothstep(relative_x);

        let original_y = self.scaled(y, y_scale);
        let wrapped_y = self.e.wrap(original_y);
        let offset_y = self.e.f64(base.offset_y);
        let shifted_y = self.e.b.ins().fadd(wrapped_y, offset_y);
        let floor_y = self.e.floor_to_int(shifted_y);
        let floor_y_f = self.e.int_to_f64(floor_y);
        let relative_y_d = self.e.b.ins().fsub(shifted_y, floor_y_f);
        let relative_y_f = self.e.demote(relative_y_d);
        let alpha_y = self.e.smoothstep(relative_y_f);

        let x0y = self.e.b.ins().iadd(x0, floor_y);
        let x1y = self.e.b.ins().iadd(x1, floor_y);
        let xy00 = self.e.permute(perms, x0y);
        let x0y1 = self.e.b.ins().iadd_imm_s(x0y, 1);
        let xy01 = self.e.permute(perms, x0y1);
        let xy10 = self.e.permute(perms, x1y);
        let x1y1 = self.e.b.ins().iadd_imm_s(x1y, 1);
        let xy11 = self.e.permute(perms, x1y1);
        let one = self.e.f32(1.0);
        let relative_x1 = self.e.b.ins().fsub(relative_x, one);
        let relative_z1 = self.e.b.ins().fsub(relative_z, one);
        let floor_z1 = self.e.b.ins().iadd_imm_s(floor_z, 1);
        let corners = [
            (xy00, floor_z, relative_x, relative_z),
            (xy10, floor_z, relative_x1, relative_z),
            (xy01, floor_z, relative_x, relative_z),
            (xy11, floor_z, relative_x1, relative_z),
            (xy00, floor_z1, relative_x, relative_z1),
            (xy10, floor_z1, relative_x1, relative_z1),
            (xy01, floor_z1, relative_x, relative_z1),
            (xy11, floor_z1, relative_x1, relative_z1),
        ];

        let ry = match fudge_y_scale {
            None => relative_y_f,
            Some(scale) => {
                let fudge = self.e.fudge_y(scale, original_y, relative_y_d);
                let fudged = self.e.b.ins().fsub(relative_y_d, fudge);
                self.e.demote(fudged)
            }
        };
        let ry1 = self.e.b.ins().fsub(ry, one);
        let mut values = [ry; 8];
        for (i, (xy, zz, cx, cz)) in corners.into_iter().enumerate() {
            let index = self.e.b.ins().iadd(xy, zz);
            let hash = self.e.permute(perms, index);
            let (d, g) = self.e.grad_xz_and_y(hash, cx, cz);
            let y_term = if i % 4 >= 2 { ry1 } else { ry };
            let scaled = self.e.b.ins().fmul(g, y_term);
            values[i] = self.e.b.ins().fadd(d, scaled);
        }
        self.e.lerp3(alpha_x, alpha_y, alpha_z, values)
    }

    fn volume_axis(&mut self, coordinate: Value, scale: f64, offset: f64) -> (Value, Value) {
        let scaled = self.scaled(coordinate, scale);
        let wrapped = self.e.wrap(scaled);
        let offset = self.e.f64(offset);
        let shifted = self.e.b.ins().fadd(wrapped, offset);
        let floor = self.e.floor_to_int(shifted);
        let floor_f = self.e.int_to_f64(floor);
        let relative = self.e.b.ins().fsub(shifted, floor_f);
        (floor, self.e.demote(relative))
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
