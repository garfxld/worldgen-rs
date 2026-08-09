use cranelift_codegen::ir::condcodes::FloatCC;
use cranelift_codegen::ir::{FuncRef, InstBuilder, MemFlagsData, Value, types};
use cranelift_frontend::FunctionBuilder;
use rustc_hash::FxHashMap;

use crate::worldgen::backend::ir::analysis::fingerprint::fingerprint;
use crate::worldgen::backend::ir::analysis::graph::unwrap_markers;
use crate::worldgen::backend::ir::{
    AnalysisContext, DensityFunction, DensityProgram, MappedType, MarkerType, RarityValueMapper,
    TwoArgType,
};

use super::super::inlining::InlinePolicy;
use super::super::layout::CacheLayout;

use super::blended::BlendedNoisePlan;
use super::math;
use super::noise::{NormalNoisePlan, Sample};
use super::spline::emit_spline;

#[derive(Debug, thiserror::Error)]
#[error("density function node not supported by the JIT backend: {0}")]
pub struct Unsupported(pub &'static str);

pub struct EmitCtx<'a> {
    pub(crate) flags: MemFlagsData,
    pub(crate) gradient: Value,
    pub(crate) noises: &'a FxHashMap<String, NormalNoisePlan>,
    pub(crate) blended: Option<&'a BlendedNoisePlan>,
    pub(crate) program: &'a DensityProgram,
    pub(crate) analysis: &'a AnalysisContext,
    pub(crate) layout: &'a CacheLayout,
    pub(crate) inlining: &'a InlinePolicy,
    pub(crate) named_refs: &'a FxHashMap<String, FuncRef>,
    pub(crate) cache: Value,
    pub(crate) pos: Position,
    pub(crate) is_flat: bool,
    pub(crate) fill: Option<Value>,
    pub(crate) interpolated: Option<Value>,
    pub(crate) end_islands: Option<EndIslandsCall>,
    pub(crate) interp_index: usize,
}

#[derive(Clone, Copy)]
pub struct EndIslandsCall {
    pub(crate) func: FuncRef,
    pub(crate) instance: Value,
}

#[derive(Clone, Copy)]
pub struct Position {
    pub(crate) x: Value,
    pub(crate) y: Value,
    pub(crate) z: Value,
}

impl<'a> EmitCtx<'a> {
    fn load_slot(&self, bld: &mut FunctionBuilder<'_>, offset: i32) -> Value {
        bld.ins().load(types::F64, self.flags, self.cache, offset)
    }

    fn noise_plan(&self, id: &str) -> Result<&'a NormalNoisePlan, Unsupported> {
        self.noises
            .get(id)
            .ok_or(Unsupported("noise ID not bound for this dimension"))
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "one arm per density function variant reads better than split dispatch"
)]
pub fn emit(
    builder: &mut FunctionBuilder<'_>,
    ctx: &mut EmitCtx<'_>,
    df: &DensityFunction,
) -> Result<Value, Unsupported> {
    match df {
        DensityFunction::Constant(c) => Ok(math::f64_const(builder, c.value)),

        DensityFunction::YClampedGradient(g) => {
            let from_y = f64::from(g.from_y);
            let to_y = f64::from(g.to_y);
            Ok(map_clamped(
                builder,
                ctx.pos.y,
                from_y,
                to_y,
                g.from_value,
                g.to_value,
            ))
        }

        DensityFunction::Noise(n) => {
            if !ctx.is_flat
                && n.y_scale == 0.0
                && let Some(offset) = ctx.layout.inline_noise_offset(fingerprint(df))
            {
                return Ok(ctx.load_slot(builder, offset));
            }

            let sample = scaled_sample(builder, ctx.pos, n.xz_scale, n.y_scale);
            let plan = ctx.noise_plan(&n.noise_id)?;
            Ok(plan.emit(builder, ctx.flags, ctx.gradient, sample))
        }

        DensityFunction::ShiftedNoise(sn) => {
            let plan = ctx.noise_plan(&sn.noise_id)?;
            let shift_x = emit(builder, ctx, &sn.shift_x)?;
            let shift_y = emit(builder, ctx, &sn.shift_y)?;
            let shift_z = emit(builder, ctx, &sn.shift_z)?;

            let xz = math::f64_const(builder, sn.xz_scale);
            let ys = math::f64_const(builder, sn.y_scale);
            let x = builder.ins().fmul(ctx.pos.x, xz);
            let x = builder.ins().fadd(x, shift_x);
            let y = builder.ins().fmul(ctx.pos.y, ys);
            let y = builder.ins().fadd(y, shift_y);
            let z = builder.ins().fmul(ctx.pos.z, xz);
            let z = builder.ins().fadd(z, shift_z);

            Ok(plan.emit(builder, ctx.flags, ctx.gradient, Sample::Xyz { x, y, z }))
        }

        DensityFunction::Shift(s) => {
            let plan = ctx.noise_plan(&s.noise_id)?;
            let x = quarter(builder, ctx.pos.x);
            let z = quarter(builder, ctx.pos.z);
            let sample = if ctx.is_flat {
                Sample::Xz { x, z }
            } else {
                Sample::Xyz {
                    x,
                    y: quarter(builder, ctx.pos.y),
                    z,
                }
            };
            let value = plan.emit(builder, ctx.flags, ctx.gradient, sample);
            Ok(times_four(builder, value))
        }
        DensityFunction::ShiftA(s) => {
            let plan = ctx.noise_plan(&s.noise_id)?;
            let sample = Sample::Xz {
                x: quarter(builder, ctx.pos.x),
                z: quarter(builder, ctx.pos.z),
            };
            let value = plan.emit(builder, ctx.flags, ctx.gradient, sample);
            Ok(times_four(builder, value))
        }
        DensityFunction::ShiftB(s) => {
            let plan = ctx.noise_plan(&s.noise_id)?;
            let sample = Sample::Xy {
                x: quarter(builder, ctx.pos.z),
                y: quarter(builder, ctx.pos.x),
            };
            let value = plan.emit(builder, ctx.flags, ctx.gradient, sample);
            Ok(times_four(builder, value))
        }

        DensityFunction::Clamp(c) => {
            let v = emit(builder, ctx, &c.input)?;
            Ok(math::clamp_const(builder, v, c.min, c.max))
        }

        DensityFunction::Mapped(m) => {
            let v = emit(builder, ctx, &m.input)?;
            Ok(mapped(builder, m.op, v))
        }

        DensityFunction::TwoArgumentSimple(t) => {
            let a = emit(builder, ctx, &t.argument1)?;
            let b = emit(builder, ctx, &t.argument2)?;
            Ok(match t.op {
                TwoArgType::Add => builder.ins().fadd(a, b),
                TwoArgType::Mul => builder.ins().fmul(a, b),
                TwoArgType::Min => {
                    let cmp = builder.ins().fcmp(FloatCC::LessThan, b, a);
                    builder.ins().select(cmp, b, a)
                }
                TwoArgType::Max => {
                    let cmp = builder.ins().fcmp(FloatCC::GreaterThan, b, a);
                    builder.ins().select(cmp, b, a)
                }
            })
        }

        DensityFunction::RangeChoice(rc) => {
            let input = emit(builder, ctx, &rc.input)?;
            let in_range = emit(builder, ctx, &rc.when_in_range)?;
            let out_range = emit(builder, ctx, &rc.when_out_of_range)?;

            let min = math::f64_const(builder, rc.min_inclusive);
            let max = math::f64_const(builder, rc.max_exclusive);
            let ge_min = builder.ins().fcmp(FloatCC::GreaterThanOrEqual, input, min);
            let lt_max = builder.ins().fcmp(FloatCC::LessThan, input, max);
            let inside = builder.ins().band(ge_min, lt_max);
            Ok(builder.ins().select(inside, in_range, out_range))
        }

        DensityFunction::IntervalSelect(interval) => {
            let input = emit(builder, ctx, &interval.input)?;
            let branches = interval
                .functions
                .iter()
                .map(|f| emit(builder, ctx, f))
                .collect::<Result<Vec<_>, _>>()?;

            let mut selected = *branches
                .last()
                .ok_or(Unsupported("interval_select with no branches"))?;
            for (threshold, branch) in interval
                .thresholds
                .iter()
                .zip(&branches)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
            {
                let t = math::f64_const(builder, *threshold);
                let below = builder.ins().fcmp(FloatCC::LessThan, input, t);
                selected = builder.ins().select(below, *branch, selected);
            }
            Ok(selected)
        }

        DensityFunction::Marker(m) => match m.kind {
            MarkerType::Interpolated => match ctx.interpolated {
                Some(channels) => {
                    let index = ctx.interp_index;
                    ctx.interp_index += 1;
                    let offset = i32::try_from(index * size_of::<f64>())
                        .map_err(|_| Unsupported("too many interpolated channels"))?;
                    Ok(builder.ins().load(types::F64, ctx.flags, channels, offset))
                }
                None => emit(builder, ctx, &m.wrapped),
            },
            MarkerType::FlatCache
            | MarkerType::Cache2D
            | MarkerType::CacheOnce
            | MarkerType::CacheAllInCell => emit(builder, ctx, &m.wrapped),
        },

        DensityFunction::Reference(r) => {
            let must_inline = (ctx.interpolated.is_some()
                && ctx.analysis.interpolated_refs.contains(&r.id))
                || (ctx.fill.is_some() && ctx.analysis.blended_noise_refs.contains(&r.id));

            if must_inline {
                let target = ctx
                    .program
                    .registry
                    .get(&r.id)
                    .ok_or(Unsupported("unresolved density function reference"))?;
                return emit(builder, ctx, target);
            }

            if let Some(offset) = ctx.layout.named_offset(&r.id) {
                return Ok(ctx.load_slot(builder, offset));
            }

            if ctx.inlining.should_inline(&r.id) {
                let target = ctx
                    .program
                    .registry
                    .get(&r.id)
                    .ok_or(Unsupported("unresolved density function reference"))?;
                let inner = unwrap_markers(target);
                return emit(builder, ctx, inner);
            }

            let func = *ctx
                .named_refs
                .get(&r.id)
                .ok_or(Unsupported("reference to an uncompiled density function"))?;
            let args = [ctx.cache, ctx.pos.x, ctx.pos.y, ctx.pos.z];
            let call = builder.ins().call(func, &args);
            Ok(builder.inst_results(call)[0])
        }

        DensityFunction::Spline(s) => emit_spline(builder, ctx, &s.spline),

        DensityFunction::WeirdScaledSampler(ws) => {
            let rarity = emit(builder, ctx, &ws.input)?;
            let scale = rarity_scale(builder, ws.rarity_value_mapper, rarity);

            let plan = ctx.noise_plan(&ws.noise_id)?;
            let x = builder.ins().fdiv(ctx.pos.x, scale);
            let y = builder.ins().fdiv(ctx.pos.y, scale);
            let z = builder.ins().fdiv(ctx.pos.z, scale);

            let sampled = plan.emit(builder, ctx.flags, ctx.gradient, Sample::Xyz { x, y, z });
            let magnitude = builder.ins().fabs(sampled);
            Ok(builder.ins().fmul(scale, magnitude))
        }

        DensityFunction::BlendAlpha(_) => Ok(math::f64_const(builder, 1.0)),
        DensityFunction::BlendOffset(_) => Ok(math::f64_const(builder, 0.0)),
        DensityFunction::BlendDensity(bd) => emit(builder, ctx, &bd.input),

        DensityFunction::BlendedNoise(_) => {
            if let Some(value) = ctx.fill {
                return Ok(value);
            }
            let plan = ctx.blended.ok_or(Unsupported(
                "old_blended_noise with no blended sampler bound",
            ))?;
            Ok(plan.emit(
                builder,
                ctx.flags,
                ctx.gradient,
                ctx.pos.x,
                ctx.pos.y,
                ctx.pos.z,
            ))
        }
        DensityFunction::FindTopSurface(fts) => {
            let upper = emit(builder, ctx, &fts.upper_bound)?;
            let cell_height = f64::from(fts.cell_height);
            let lower_bound = f64::from(fts.lower_bound);

            let cell = math::f64_const(builder, cell_height);
            let lower = math::f64_const(builder, lower_bound);

            let scaled = builder.ins().fdiv(upper, cell);
            let floored = builder.ins().floor(scaled);
            let top_y = builder.ins().fmul(floored, cell);

            let header = builder.create_block();
            builder.append_block_param(header, types::F64);
            let body = builder.create_block();
            let next = builder.create_block();
            let done = builder.create_block();
            builder.append_block_param(done, types::F64);

            let empty = builder.ins().fcmp(FloatCC::LessThanOrEqual, top_y, lower);
            builder
                .ins()
                .brif(empty, done, &[lower.into()], header, &[top_y.into()]);

            builder.switch_to_block(header);
            let y = builder.block_params(header)[0];
            let in_range = builder.ins().fcmp(FloatCC::GreaterThanOrEqual, y, lower);
            builder
                .ins()
                .brif(in_range, body, &[], done, &[lower.into()]);

            builder.switch_to_block(body);
            builder.seal_block(body);
            let outer_y = ctx.pos.y;
            let outer_flat = ctx.is_flat;
            ctx.pos.y = y;
            ctx.is_flat = false;
            let density = emit(builder, ctx, &fts.density);
            ctx.pos.y = outer_y;
            ctx.is_flat = outer_flat;
            let density = density?;
            let zero = math::f64_const(builder, 0.0);
            let solid = builder.ins().fcmp(FloatCC::GreaterThan, density, zero);
            builder.ins().brif(solid, done, &[y.into()], next, &[]);

            builder.switch_to_block(next);
            builder.seal_block(next);
            let stepped = builder.ins().fsub(y, cell);
            builder.ins().jump(header, &[stepped.into()]);
            builder.seal_block(header);

            builder.switch_to_block(done);
            builder.seal_block(done);
            Ok(builder.block_params(done)[0])
        }

        DensityFunction::EndIslands => {
            let call = ctx
                .end_islands
                .ok_or(Unsupported("end_islands with no sampler bound"))?;
            let y = if ctx.is_flat {
                math::f64_const(builder, 0.0)
            } else {
                ctx.pos.y
            };
            let inst = builder
                .ins()
                .call(call.func, &[call.instance, ctx.pos.x, y, ctx.pos.z]);
            Ok(builder.inst_results(inst)[0])
        }
    }
}

fn scaled_sample(
    builder: &mut FunctionBuilder<'_>,
    pos: Position,
    xz_scale: f64,
    y_scale: f64,
) -> Sample {
    let xz = math::f64_const(builder, xz_scale);
    let x = builder.ins().fmul(pos.x, xz);
    let z = builder.ins().fmul(pos.z, xz);

    if y_scale == 0.0 {
        return Sample::Xz { x, z };
    }

    let ys = math::f64_const(builder, y_scale);
    let y = builder.ins().fmul(pos.y, ys);
    Sample::Xyz { x, y, z }
}

fn rarity_scale(
    builder: &mut FunctionBuilder<'_>,
    mapper: RarityValueMapper,
    rarity: Value,
) -> Value {
    let steps: &[(f64, f64)] = match mapper {
        RarityValueMapper::Tunnels => &[(-0.5, 0.75), (0.0, 1.0), (0.5, 1.5)],
        RarityValueMapper::Caves => &[(-0.75, 0.5), (-0.5, 0.75), (0.5, 1.0), (0.75, 2.0)],
    };
    let fallback = match mapper {
        RarityValueMapper::Tunnels => 2.0,
        RarityValueMapper::Caves => 3.0,
    };

    let mut selected = math::f64_const(builder, fallback);
    for (threshold, value) in steps.iter().rev() {
        let t = math::f64_const(builder, *threshold);
        let v = math::f64_const(builder, *value);
        let below = builder.ins().fcmp(FloatCC::LessThan, rarity, t);
        selected = builder.ins().select(below, v, selected);
    }
    selected
}

fn quarter(builder: &mut FunctionBuilder<'_>, v: Value) -> Value {
    let q = math::f64_const(builder, 0.25);
    builder.ins().fmul(v, q)
}

fn times_four(builder: &mut FunctionBuilder<'_>, v: Value) -> Value {
    let four = math::f64_const(builder, 4.0);
    builder.ins().fmul(v, four)
}

fn mapped(builder: &mut FunctionBuilder<'_>, op: MappedType, v: Value) -> Value {
    match op {
        MappedType::Abs => builder.ins().fabs(v),
        MappedType::Square => builder.ins().fmul(v, v),
        MappedType::Cube => {
            let sq = builder.ins().fmul(v, v);
            builder.ins().fmul(sq, v)
        }
        MappedType::HalfNegative => scale_if_negative(builder, v, 0.5),
        MappedType::QuarterNegative => scale_if_negative(builder, v, 0.25),
        MappedType::Invert => {
            let one = math::f64_const(builder, 1.0);
            builder.ins().fdiv(one, v)
        }
        MappedType::Squeeze => {
            let c = math::clamp_const(builder, v, -1.0, 1.0);
            let two = math::f64_const(builder, 2.0);
            let twenty_four = math::f64_const(builder, 24.0);
            let half = builder.ins().fdiv(c, two);
            let c2 = builder.ins().fmul(c, c);
            let c3 = builder.ins().fmul(c2, c);
            let cubic = builder.ins().fdiv(c3, twenty_four);
            builder.ins().fsub(half, cubic)
        }
    }
}

fn scale_if_negative(builder: &mut FunctionBuilder<'_>, v: Value, factor: f64) -> Value {
    let zero = math::f64_const(builder, 0.0);
    let f = math::f64_const(builder, factor);
    let scaled = builder.ins().fmul(v, f);
    let positive = builder.ins().fcmp(FloatCC::GreaterThan, v, zero);
    builder.ins().select(positive, v, scaled)
}

fn map_clamped(
    builder: &mut FunctionBuilder<'_>,
    value: Value,
    from_min: f64,
    from_max: f64,
    to_min: f64,
    to_max: f64,
) -> Value {
    let from_min_v = math::f64_const(builder, from_min);
    let span = math::f64_const(builder, from_max - from_min);
    let numerator = builder.ins().fsub(value, from_min_v);
    let t = builder.ins().fdiv(numerator, span);

    let to_min_v = math::f64_const(builder, to_min);
    let to_max_v = math::f64_const(builder, to_max);
    let zero = math::f64_const(builder, 0.0);
    let one = math::f64_const(builder, 1.0);

    let lerped = math::lerp(builder, t, to_min_v, to_max_v);
    let below = builder.ins().fcmp(FloatCC::LessThan, t, zero);
    let above = builder.ins().fcmp(FloatCC::GreaterThan, t, one);
    let upper = builder.ins().select(above, to_max_v, lerped);
    builder.ins().select(below, to_min_v, upper)
}
