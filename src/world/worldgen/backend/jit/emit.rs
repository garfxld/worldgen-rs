use cranelift_codegen::ir::AbiParam;
use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{InstBuilder, MemFlagsData, Signature, Value, types};
use cranelift_codegen::isa::CallConv;
use cranelift_frontend::FunctionBuilder;

use crate::world::worldgen::synth::Noise;
use crate::world::worldgen::synth::gradient_noise::GradientNoise;
use crate::world::worldgen::synth::perlin_noise::PerlinNoise;
use crate::world::worldgen::synth::smeared_perlin_noise::SmearedPerlinNoise;

use super::runtime;
use std::ptr;

const ROUND_OFF: f64 = 33_554_432.0;
const HALF_ROUND_OFF: f64 = f64::from_bits(16_777_216.0_f64.to_bits() - 1);
const SHIFT_UP_EPSILON: f32 = 1.0E-7;

pub struct Emitter<'a, 'f> {
    pub b: &'a mut FunctionBuilder<'f>,
    pub ptr: types::Type,
    pub call_conv: CallConv,
}

impl Emitter<'_, '_> {
    pub fn f32(&mut self, value: f32) -> Value {
        self.b.ins().f32const(value)
    }

    pub fn f64(&mut self, value: f64) -> Value {
        self.b.ins().f64const(value)
    }

    pub fn i32(&mut self, value: i32) -> Value {
        self.b.ins().iconst(types::I32, i64::from(value))
    }

    pub fn address<T>(&mut self, pointer: *const T) -> Value {
        self.b.ins().iconst(self.ptr, pointer as i64)
    }

    pub fn call(
        &mut self,
        function: *const u8,
        params: &[types::Type],
        returns: &[types::Type],
        args: &[Value],
    ) -> Option<Value> {
        let mut signature = Signature::new(self.call_conv);
        signature
            .params
            .extend(params.iter().map(|ty| AbiParam::new(*ty)));
        signature
            .returns
            .extend(returns.iter().map(|ty| AbiParam::new(*ty)));
        let signature = self.b.import_signature(signature);
        let callee = self.b.ins().iconst(self.ptr, function as i64);
        let call = self.b.ins().call_indirect(signature, callee, args);
        self.b.inst_results(call).first().copied()
    }

    pub fn promote(&mut self, value: Value) -> Value {
        self.b.ins().fpromote(types::F64, value)
    }

    pub fn demote(&mut self, value: Value) -> Value {
        self.b.ins().fdemote(types::F32, value)
    }

    pub fn int_to_f64(&mut self, value: Value) -> Value {
        self.b.ins().fcvt_from_sint(types::F64, value)
    }

    pub fn int_to_f32(&mut self, value: Value) -> Value {
        self.b.ins().fcvt_from_sint(types::F32, value)
    }

    pub fn floor_to_int(&mut self, value: Value) -> Value {
        let floored = self.b.ins().floor(value);
        self.b.ins().fcvt_to_sint_sat(types::I32, floored)
    }

    pub fn lerp(&mut self, alpha: Value, p0: Value, p1: Value) -> Value {
        let delta = self.b.ins().fsub(p1, p0);
        let scaled = self.b.ins().fmul(alpha, delta);
        self.b.ins().fadd(p0, scaled)
    }

    pub fn smoothstep(&mut self, x: Value) -> Value {
        let six = self.f32(6.0);
        let fifteen = self.f32(15.0);
        let ten = self.f32(10.0);
        let x2 = self.b.ins().fmul(x, x);
        let x3 = self.b.ins().fmul(x2, x);
        let a = self.b.ins().fmul(x, six);
        let a = self.b.ins().fsub(a, fifteen);
        let a = self.b.ins().fmul(x, a);
        let a = self.b.ins().fadd(a, ten);
        self.b.ins().fmul(x3, a)
    }

    pub fn lerp3(&mut self, a1: Value, a2: Value, a3: Value, corners: [Value; 8]) -> Value {
        let [x000, x100, x010, x110, x001, x101, x011, x111] = corners;
        let l0 = self.lerp(a1, x000, x100);
        let l1 = self.lerp(a1, x010, x110);
        let near = self.lerp(a2, l0, l1);
        let l2 = self.lerp(a1, x001, x101);
        let l3 = self.lerp(a1, x011, x111);
        let far = self.lerp(a2, l2, l3);
        self.lerp(a3, near, far)
    }

    pub fn java_min(&mut self, a: Value, b: Value) -> Value {
        let le = self.b.ins().fcmp(FloatCC::LessThanOrEqual, a, b);
        let ordinary = self.b.ins().select(le, a, b);
        let zero = self.f32(0.0);
        let a_zero = self.b.ins().fcmp(FloatCC::Equal, a, zero);
        let bits = self.b.ins().bitcast(types::I32, MemFlagsData::new(), b);
        let b_negative_zero = self.b.ins().icmp_imm_u(IntCC::Equal, bits, 0x8000_0000);
        let both = self.b.ins().band(a_zero, b_negative_zero);
        let zeroed = self.b.ins().select(both, b, ordinary);
        let a_nan = self.b.ins().fcmp(FloatCC::Unordered, a, a);
        self.b.ins().select(a_nan, a, zeroed)
    }

    pub fn java_max(&mut self, a: Value, b: Value) -> Value {
        let ge = self.b.ins().fcmp(FloatCC::GreaterThanOrEqual, a, b);
        let ordinary = self.b.ins().select(ge, a, b);
        let zero = self.f32(0.0);
        let b_zero = self.b.ins().fcmp(FloatCC::Equal, b, zero);
        let bits = self.b.ins().bitcast(types::I32, MemFlagsData::new(), a);
        let a_negative_zero = self.b.ins().icmp_imm_u(IntCC::Equal, bits, 0x8000_0000);
        let both = self.b.ins().band(b_zero, a_negative_zero);
        let zeroed = self.b.ins().select(both, b, ordinary);
        let a_nan = self.b.ins().fcmp(FloatCC::Unordered, a, a);
        self.b.ins().select(a_nan, a, zeroed)
    }

    pub fn clamp(&mut self, value: Value, min: f32, max: f32) -> Value {
        let min_value = self.f32(min);
        let max_value = self.f32(max);
        let upper = self.java_min(value, max_value);
        let below = self.b.ins().fcmp(FloatCC::LessThan, value, min_value);
        self.b.ins().select(below, min_value, upper)
    }

    pub fn sqrt_f32(&mut self, value: Value) -> Value {
        let wide = self.promote(value);
        let root = self.b.ins().sqrt(wide);
        self.demote(root)
    }

    pub fn wrap(&mut self, x: Value) -> Value {
        let low = self.f64(-HALF_ROUND_OFF);
        let high = self.f64(HALF_ROUND_OFF);
        let round_off = self.f64(ROUND_OFF);
        let half = self.f64(0.5);
        let ge = self.b.ins().fcmp(FloatCC::GreaterThanOrEqual, x, low);
        let lt = self.b.ins().fcmp(FloatCC::LessThan, x, high);
        let inside = self.b.ins().band(ge, lt);
        let scaled = self.b.ins().fdiv(x, round_off);
        let scaled = self.b.ins().fadd(scaled, half);
        let floored = self.b.ins().floor(scaled);
        let offset = self.b.ins().fmul(floored, round_off);
        let wrapped = self.b.ins().fsub(x, offset);
        self.b.ins().select(inside, x, wrapped)
    }

    pub fn permute(&mut self, perms: Value, index: Value) -> Value {
        let masked = self.b.ins().band_imm_u(index, 0xFF);
        let offset = self.b.ins().uextend(self.ptr, masked);
        let address = self.b.ins().iadd(perms, offset);
        let byte = self.b.ins().load(
            types::I8,
            MemFlagsData::trusted().with_readonly().with_can_move(),
            address,
            0,
        );
        self.b.ins().uextend(types::I32, byte)
    }

    fn gradient_entry(&mut self, hash: Value) -> Value {
        let table = self.address(runtime::GRADIENT_F32.as_ptr());
        let index = self.b.ins().band_imm_u(hash, 15);
        let index = self.b.ins().uextend(self.ptr, index);
        let offset = self.b.ins().ishl_imm_u(index, 4);
        self.b.ins().iadd(table, offset)
    }

    fn gradient_component(&mut self, entry: Value, component: i32) -> Value {
        self.b.ins().load(
            types::F32,
            MemFlagsData::trusted().with_readonly().with_can_move(),
            entry,
            component * 4,
        )
    }

    pub fn grad_dot(&mut self, hash: Value, x: Value, y: Value, z: Value) -> Value {
        let entry = self.gradient_entry(hash);
        let gx = self.gradient_component(entry, 0);
        let gy = self.gradient_component(entry, 1);
        let gz = self.gradient_component(entry, 2);
        let tx = self.b.ins().fmul(gx, x);
        let ty = self.b.ins().fmul(gy, y);
        let sum = self.b.ins().fadd(tx, ty);
        let tz = self.b.ins().fmul(gz, z);
        self.b.ins().fadd(sum, tz)
    }

    pub fn grad_xz_and_y(&mut self, hash: Value, x: Value, z: Value) -> (Value, Value) {
        let entry = self.gradient_entry(hash);
        let gx = self.gradient_component(entry, 0);
        let gy = self.gradient_component(entry, 1);
        let gz = self.gradient_component(entry, 2);
        let tx = self.b.ins().fmul(gx, x);
        let tz = self.b.ins().fmul(gz, z);
        (self.b.ins().fadd(tx, tz), gy)
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors PerlinNoise.sampleAndLerp"
    )]
    pub fn sample_and_lerp(
        &mut self,
        base: &GradientNoise,
        x: Value,
        y: Value,
        z: Value,
        rx: Value,
        ry: Value,
        rz: Value,
        original_ry: Value,
    ) -> Value {
        let perms = self.address(base.perms.as_ptr());
        let x0 = self.permute(perms, x);
        let x_plus = self.b.ins().iadd_imm_s(x, 1);
        let x1 = self.permute(perms, x_plus);
        let x0y = self.b.ins().iadd(x0, y);
        let x1y = self.b.ins().iadd(x1, y);
        let xy00 = self.permute(perms, x0y);
        let x0y1 = self.b.ins().iadd_imm_s(x0y, 1);
        let xy01 = self.permute(perms, x0y1);
        let xy10 = self.permute(perms, x1y);
        let x1y1 = self.b.ins().iadd_imm_s(x1y, 1);
        let xy11 = self.permute(perms, x1y1);
        let one = self.f32(1.0);
        let rx1 = self.b.ins().fsub(rx, one);
        let ry1 = self.b.ins().fsub(ry, one);
        let rz1 = self.b.ins().fsub(rz, one);
        let z1 = self.b.ins().iadd_imm_s(z, 1);
        let corners = [
            (xy00, z, rx, ry, rz),
            (xy10, z, rx1, ry, rz),
            (xy01, z, rx, ry1, rz),
            (xy11, z, rx1, ry1, rz),
            (xy00, z1, rx, ry, rz1),
            (xy10, z1, rx1, ry, rz1),
            (xy01, z1, rx, ry1, rz1),
            (xy11, z1, rx1, ry1, rz1),
        ];
        let mut dots = [x; 8];
        for (i, (xy, zz, cx, cy, cz)) in corners.into_iter().enumerate() {
            let index = self.b.ins().iadd(xy, zz);
            let hash = self.permute(perms, index);
            dots[i] = self.grad_dot(hash, cx, cy, cz);
        }
        let ax = self.smoothstep(rx);
        let ay = self.smoothstep(original_ry);
        let az = self.smoothstep(rz);
        self.lerp3(ax, ay, az, dots)
    }

    fn perlin_get(&mut self, noise: &PerlinNoise, x: Value, y: Value, z: Value) -> Value {
        let base = &noise.base;
        let (fx, rx, _) = self.offset_axis(x, base.offset_x);
        let (fy, ry, _) = self.offset_axis(y, base.offset_y);
        let (fz, rz, _) = self.offset_axis(z, base.offset_z);
        self.sample_and_lerp(base, fx, fy, fz, rx, ry, rz, ry)
    }

    fn offset_axis(&mut self, coordinate: Value, offset: f64) -> (Value, Value, Value) {
        let wrapped = self.wrap(coordinate);
        let offset = self.f64(offset);
        let shifted = self.b.ins().fadd(wrapped, offset);
        let floor = self.floor_to_int(shifted);
        let floor_f = self.int_to_f64(floor);
        let relative = self.b.ins().fsub(shifted, floor_f);
        let relative_f32 = self.demote(relative);
        (floor, relative_f32, relative)
    }

    pub fn fudge_y(&mut self, fudge_y_scale: f64, original_y: Value, relative_y: Value) -> Value {
        let zero = self.f64(0.0);
        let non_negative = self
            .b
            .ins()
            .fcmp(FloatCC::GreaterThanOrEqual, original_y, zero);
        let below = self.b.ins().fcmp(FloatCC::LessThan, original_y, relative_y);
        let use_original = self.b.ins().band(non_negative, below);
        let limit = self.b.ins().select(use_original, original_y, relative_y);
        let scale = self.f64(fudge_y_scale);
        let epsilon = self.f64(f64::from(SHIFT_UP_EPSILON));
        let divided = self.b.ins().fdiv(limit, scale);
        let shifted = self.b.ins().fadd(divided, epsilon);
        let floored = self.floor_to_int(shifted);
        let floored = self.int_to_f64(floored);
        self.b.ins().fmul(floored, scale)
    }

    fn smeared_get(&mut self, noise: &SmearedPerlinNoise, x: Value, y: Value, z: Value) -> Value {
        let base = &noise.perlin.base;
        let (fx, rx, _) = self.offset_axis(x, base.offset_x);
        let (fy, _, relative_y) = self.offset_axis(y, base.offset_y);
        let (fz, rz, _) = self.offset_axis(z, base.offset_z);
        let fudge = self.fudge_y(noise.fudge_y_scale, y, relative_y);
        let fudged = self.b.ins().fsub(relative_y, fudge);
        let fudged = self.demote(fudged);
        let original = self.demote(relative_y);
        self.sample_and_lerp(base, fx, fy, fz, rx, fudged, rz, original)
    }

    pub fn noise_get(&mut self, noise: &Noise, x: Value, y: Value, z: Value) -> Value {
        match noise {
            Noise::Perlin(perlin) => self.perlin_get(perlin, x, y, z),
            Noise::SmearedPerlin(smeared) => self.smeared_get(smeared, x, y, z),
            Noise::Simplex(_) => {
                let ptr = self.ptr;
                let noise_ptr = self.address(ptr::from_ref(noise));
                self.call(
                    runtime::noise_get as *const u8,
                    &[ptr, types::F64, types::F64, types::F64],
                    &[types::F32],
                    &[noise_ptr, x, y, z],
                )
                .expect("noise_get returns f32")
            }
            Noise::Stack(stack) => {
                let mut value = self.f32(0.0);
                for layer in &stack.layers {
                    let frequency = self.f64(layer.frequency);
                    let lx = self.b.ins().fmul(x, frequency);
                    let ly = self.b.ins().fmul(y, frequency);
                    let lz = self.b.ins().fmul(z, frequency);
                    let sample = self.noise_get(&layer.noise, lx, ly, lz);
                    let amplitude = self.f32(layer.amplitude);
                    let scaled = self.b.ins().fmul(amplitude, sample);
                    value = self.b.ins().fadd(value, scaled);
                }
                value
            }
        }
    }
}
