use cranelift_codegen::ir::condcodes::FloatCC;
use cranelift_codegen::ir::{Block, InstBuilder, Value, types};
use cranelift_frontend::FunctionBuilder;

use crate::worldgen::backend::ir::{CubicSpline, SplineValue};

use super::expr::{EmitCtx, Unsupported, emit};

pub(crate) fn emit_spline(
    builder: &mut FunctionBuilder<'_>,
    ctx: &mut EmitCtx<'_>,
    spline: &CubicSpline,
) -> Result<Value, Unsupported> {
    let value = emit_spline_f32(builder, ctx, spline)?;
    Ok(builder.ins().fpromote(types::F64, value))
}

fn emit_spline_f32(
    builder: &mut FunctionBuilder<'_>,
    ctx: &mut EmitCtx<'_>,
    spline: &CubicSpline,
) -> Result<Value, Unsupported> {
    let coord_f64 = emit(builder, ctx, &spline.coordinate)?;
    let coord = builder.ins().fdemote(types::F32, coord_f64);

    let Some(first) = spline.points.first() else {
        return Ok(builder.ins().f32const(0.0));
    };
    let last_index = spline.points.len() - 1;

    if last_index == 0 {
        let value = point_value(builder, ctx, &first.value)?;
        return Ok(extrapolate(
            builder,
            value,
            first.derivative,
            first.location,
            coord,
        ));
    }

    let merge = builder.create_block();
    builder.append_block_param(merge, types::F32);

    let mut fallthrough = branch_if_below(builder, coord, first.location);
    {
        let value = point_value(builder, ctx, &first.value)?;
        let result = extrapolate(builder, value, first.derivative, first.location, coord);
        builder.ins().jump(merge, &[result.into()]);
    }

    for i in 0..last_index {
        let lo = &spline.points[i];
        let hi = &spline.points[i + 1];

        builder.switch_to_block(fallthrough);
        builder.seal_block(fallthrough);
        fallthrough = branch_if_below(builder, coord, hi.location);

        let y1 = point_value(builder, ctx, &lo.value)?;
        let y2 = point_value(builder, ctx, &hi.value)?;
        let result = hermite(
            builder,
            HermiteSegment {
                x1: lo.location,
                x2: hi.location,
                d1: lo.derivative,
                d2: hi.derivative,
            },
            y1,
            y2,
            coord,
        );
        builder.ins().jump(merge, &[result.into()]);
    }

    builder.switch_to_block(fallthrough);
    builder.seal_block(fallthrough);
    {
        let point = &spline.points[last_index];
        let value = point_value(builder, ctx, &point.value)?;
        let result = extrapolate(builder, value, point.derivative, point.location, coord);
        builder.ins().jump(merge, &[result.into()]);
    }

    builder.switch_to_block(merge);
    builder.seal_block(merge);
    Ok(builder.block_params(merge)[0])
}

fn branch_if_below(builder: &mut FunctionBuilder<'_>, coord: Value, threshold: f32) -> Block {
    let taken = builder.create_block();
    let fallthrough = builder.create_block();
    let limit = builder.ins().f32const(threshold);
    let below = builder.ins().fcmp(FloatCC::LessThan, coord, limit);
    builder.ins().brif(below, taken, &[], fallthrough, &[]);
    builder.switch_to_block(taken);
    builder.seal_block(taken);
    fallthrough
}

fn point_value(
    builder: &mut FunctionBuilder<'_>,
    ctx: &mut EmitCtx<'_>,
    value: &SplineValue,
) -> Result<Value, Unsupported> {
    match value {
        SplineValue::Constant(c) => Ok(builder.ins().f32const(*c)),
        SplineValue::Spline(nested) => emit_spline_f32(builder, ctx, nested),
    }
}

fn extrapolate(
    builder: &mut FunctionBuilder<'_>,
    value: Value,
    derivative: f32,
    location: f32,
    coord: Value,
) -> Value {
    let loc = builder.ins().f32const(location);
    let der = builder.ins().f32const(derivative);
    let delta = builder.ins().fsub(coord, loc);
    let scaled = builder.ins().fmul(der, delta);
    builder.ins().fadd(value, scaled)
}

struct HermiteSegment {
    x1: f32,
    x2: f32,
    d1: f32,
    d2: f32,
}

fn hermite(
    builder: &mut FunctionBuilder<'_>,
    seg: HermiteSegment,
    y1: Value,
    y2: Value,
    coord: Value,
) -> Value {
    let x1 = builder.ins().f32const(seg.x1);
    let h = builder.ins().f32const(seg.x2 - seg.x1);
    let d1 = builder.ins().f32const(seg.d1);
    let d2 = builder.ins().f32const(seg.d2);
    let one = builder.ins().f32const(1.0);

    let t = {
        let numerator = builder.ins().fsub(coord, x1);
        builder.ins().fdiv(numerator, h)
    };

    let dy = builder.ins().fsub(y2, y1);

    let a = {
        let scaled = builder.ins().fmul(d1, h);
        builder.ins().fsub(scaled, dy)
    };
    let b = {
        let neg = builder.ins().fneg(d2);
        let scaled = builder.ins().fmul(neg, h);
        builder.ins().fadd(scaled, dy)
    };

    let lerp_y = {
        let scaled = builder.ins().fmul(t, dy);
        builder.ins().fadd(y1, scaled)
    };
    let lerp_ab = {
        let diff = builder.ins().fsub(b, a);
        let scaled = builder.ins().fmul(t, diff);
        builder.ins().fadd(a, scaled)
    };

    let one_minus_t = builder.ins().fsub(one, t);
    let weight = builder.ins().fmul(t, one_minus_t);
    let correction = builder.ins().fmul(weight, lerp_ab);
    builder.ins().fadd(lerp_y, correction)
}
