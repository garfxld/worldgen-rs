use cranelift_codegen::ir::condcodes::FloatCC;
use cranelift_codegen::ir::{InstBuilder, Value, types};
use cranelift_frontend::FunctionBuilder;

const ROUND_OFF: f64 = 33_554_432.0;

pub(crate) fn f64_const(builder: &mut FunctionBuilder<'_>, v: f64) -> Value {
    builder.ins().f64const(v)
}

pub(crate) fn lerp(builder: &mut FunctionBuilder<'_>, alpha: Value, a: Value, b: Value) -> Value {
    let diff = builder.ins().fsub(b, a);
    let scaled = builder.ins().fmul(alpha, diff);
    builder.ins().fadd(a, scaled)
}

pub(crate) fn lerp2(
    builder: &mut FunctionBuilder<'_>,
    a1: Value,
    a2: Value,
    x00: Value,
    x10: Value,
    x01: Value,
    x11: Value,
) -> Value {
    let lo = lerp(builder, a1, x00, x10);
    let hi = lerp(builder, a1, x01, x11);
    lerp(builder, a2, lo, hi)
}

pub(crate) fn lerp3(
    builder: &mut FunctionBuilder<'_>,
    a1: Value,
    a2: Value,
    a3: Value,
    corners: [Value; 8],
) -> Value {
    let [x000, x100, x010, x110, x001, x101, x011, x111] = corners;
    let lo = lerp2(builder, a1, a2, x000, x100, x010, x110);
    let hi = lerp2(builder, a1, a2, x001, x101, x011, x111);
    lerp(builder, a3, lo, hi)
}

pub(crate) fn smoothstep(builder: &mut FunctionBuilder<'_>, x: Value) -> Value {
    let six = f64_const(builder, 6.0);
    let fifteen = f64_const(builder, 15.0);
    let ten = f64_const(builder, 10.0);

    let inner = builder.ins().fmul(x, six);
    let inner = builder.ins().fsub(inner, fifteen);
    let inner = builder.ins().fmul(x, inner);
    let inner = builder.ins().fadd(inner, ten);

    let x2 = builder.ins().fmul(x, x);
    let x3 = builder.ins().fmul(x2, x);
    builder.ins().fmul(x3, inner)
}

pub(crate) fn fast_floor(builder: &mut FunctionBuilder<'_>, v: Value) -> Value {
    let truncated = builder.ins().fcvt_to_sint_sat(types::I32, v);
    let as_f64 = builder.ins().fcvt_from_sint(types::F64, truncated);
    let below = builder.ins().fcmp(FloatCC::LessThan, v, as_f64);
    let one = builder.ins().iconst(types::I32, 1);
    let decremented = builder.ins().isub(truncated, one);
    builder.ins().select(below, decremented, truncated)
}

pub(crate) fn wrap(builder: &mut FunctionBuilder<'_>, x: Value) -> Value {
    let round_off = f64_const(builder, ROUND_OFF);
    let half = f64_const(builder, 0.5);

    let scaled = builder.ins().fdiv(x, round_off);
    let biased = builder.ins().fadd(scaled, half);
    let floored = builder.ins().floor(biased);
    let correction = builder.ins().fmul(floored, round_off);
    builder.ins().fsub(x, correction)
}

pub(crate) fn clamp_const(
    builder: &mut FunctionBuilder<'_>,
    value: Value,
    min: f64,
    max: f64,
) -> Value {
    let min_v = f64_const(builder, min);
    let max_v = f64_const(builder, max);
    let below = builder.ins().fcmp(FloatCC::LessThan, value, min_v);
    let above = builder.ins().fcmp(FloatCC::GreaterThan, value, max_v);
    let upper = builder.ins().select(above, max_v, value);
    builder.ins().select(below, min_v, upper)
}
