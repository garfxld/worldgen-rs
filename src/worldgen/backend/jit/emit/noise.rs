use cranelift_codegen::ir::{InstBuilder, MemFlagsData, Value, types};
use cranelift_frontend::FunctionBuilder;

use crate::util::math::fast_floor as host_fast_floor;
use crate::util::noise::{NormalNoise, OctaveSnapshot, PerlinNoise};

use super::math;

const INPUT_FACTOR: f64 = 1.018_126_888_217_522_7;

pub(crate) struct Octave {
    xo: f64,
    yo: f64,
    zo: f64,
    yo_floor: i32,
    yo_fraction: f64,
    zo_floor: i32,
    zo_fraction: f64,
    input_factor: f64,
    output_factor: f64,
    table: Value,
}

impl Octave {
    pub(crate) fn new(snapshot: &OctaveSnapshot<'_>, table: Value) -> Self {
        let yo = snapshot.noise.yo;
        let zo = snapshot.noise.zo;
        let yo_floor = host_fast_floor(yo);
        let zo_floor = host_fast_floor(zo);
        Self {
            xo: snapshot.noise.xo,
            yo,
            zo,
            yo_floor,
            yo_fraction: yo - f64::from(yo_floor),
            zo_floor,
            zo_fraction: zo - f64::from(zo_floor),
            input_factor: snapshot.input_factor,
            output_factor: snapshot.output_factor,
            table,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Sample {
    Xyz { x: Value, y: Value, z: Value },
    Xz { x: Value, z: Value },
    Xy { x: Value, y: Value },
}

impl Sample {
    fn scaled(self, builder: &mut FunctionBuilder<'_>, factor: f64) -> Self {
        let f = math::f64_const(builder, factor);
        match self {
            Self::Xyz { x, y, z } => Self::Xyz {
                x: builder.ins().fmul(x, f),
                y: builder.ins().fmul(y, f),
                z: builder.ins().fmul(z, f),
            },
            Self::Xz { x, z } => Self::Xz {
                x: builder.ins().fmul(x, f),
                z: builder.ins().fmul(z, f),
            },
            Self::Xy { x, y } => Self::Xy {
                x: builder.ins().fmul(x, f),
                y: builder.ins().fmul(y, f),
            },
        }
    }
}

fn perm_load(
    builder: &mut FunctionBuilder<'_>,
    flags: MemFlagsData,
    table: Value,
    index: Value,
) -> Value {
    let mask = builder.ins().iconst(types::I32, 0xFF);
    let wrapped = builder.ins().band(index, mask);
    let offset = builder.ins().uextend(types::I64, wrapped);
    let addr = builder.ins().iadd(table, offset);
    let byte = builder.ins().load(types::I8, flags, addr, 0);
    builder.ins().uextend(types::I32, byte)
}

#[expect(
    clippy::too_many_arguments,
    reason = "mirrors grad_dot_flat's signature"
)]
fn grad_dot_flat(
    builder: &mut FunctionBuilder<'_>,
    flags: MemFlagsData,
    gradient: Value,
    table: Value,
    px: Value,
    py: Value,
    pz: Value,
    fx: Value,
    fy: Value,
    fz: Value,
) -> Value {
    let a = perm_load(builder, flags, table, px);
    let a_py = builder.ins().iadd(a, py);
    let b = perm_load(builder, flags, table, a_py);
    let b_pz = builder.ins().iadd(b, pz);
    let hash = perm_load(builder, flags, table, b_pz);

    let fifteen = builder.ins().iconst(types::I32, 15);
    let idx = builder.ins().band(hash, fifteen);
    let idx = builder.ins().uextend(types::I64, idx);
    let shift = builder.ins().iconst(
        types::I64,
        super::super::jit_module::JitModule::gradient_stride_shift(),
    );
    let byte_offset = builder.ins().ishl(idx, shift);
    let entry = builder.ins().iadd(gradient, byte_offset);

    let g0 = builder.ins().load(types::F64, flags, entry, 0);
    let g1 = builder.ins().load(types::F64, flags, entry, 8);
    let g2 = builder.ins().load(types::F64, flags, entry, 16);

    let t0 = builder.ins().fmul(g0, fx);
    let t1 = builder.ins().fmul(g1, fy);
    let sum = builder.ins().fadd(t0, t1);
    let t2 = builder.ins().fmul(g2, fz);
    builder.ins().fadd(sum, t2)
}

pub(crate) struct Corner {
    pub(crate) xf: Value,
    pub(crate) yf: Value,
    pub(crate) zf: Value,
    pub(crate) xr: Value,
    pub(crate) yr: Value,
    pub(crate) zr: Value,
    pub(crate) yr_original: Value,
}

pub(crate) fn emit_sample_and_lerp(
    builder: &mut FunctionBuilder<'_>,
    flags: MemFlagsData,
    gradient: Value,
    table: Value,
    corner: Corner,
) -> Value {
    let Corner {
        xf,
        yf,
        zf,
        xr,
        yr,
        zr,
        yr_original,
    } = corner;
    let one_i = builder.ins().iconst(types::I32, 1);
    let px1 = builder.ins().iadd(xf, one_i);
    let py1 = builder.ins().iadd(yf, one_i);
    let pz1 = builder.ins().iadd(zf, one_i);

    let one_f = math::f64_const(builder, 1.0);
    let xr1 = builder.ins().fsub(xr, one_f);
    let yr1 = builder.ins().fsub(yr, one_f);
    let zr1 = builder.ins().fsub(zr, one_f);

    let corners = [
        (xf, yf, zf, xr, yr, zr),
        (px1, yf, zf, xr1, yr, zr),
        (xf, py1, zf, xr, yr1, zr),
        (px1, py1, zf, xr1, yr1, zr),
        (xf, yf, pz1, xr, yr, zr1),
        (px1, yf, pz1, xr1, yr, zr1),
        (xf, py1, pz1, xr, yr1, zr1),
        (px1, py1, pz1, xr1, yr1, zr1),
    ]
    .map(|(cx, cy, cz, fx, fy, fz)| {
        grad_dot_flat(builder, flags, gradient, table, cx, cy, cz, fx, fy, fz)
    });

    let alpha_x = math::smoothstep(builder, xr);
    let alpha_y = math::smoothstep(builder, yr_original);
    let alpha_z = math::smoothstep(builder, zr);
    math::lerp3(builder, alpha_x, alpha_y, alpha_z, corners)
}

fn improved_noise(
    builder: &mut FunctionBuilder<'_>,
    flags: MemFlagsData,
    gradient: Value,
    octave: &Octave,
    sample: Sample,
) -> Value {
    match sample {
        Sample::Xyz { x, y, z } => {
            let xo = math::f64_const(builder, octave.xo);
            let yo = math::f64_const(builder, octave.yo);
            let zo = math::f64_const(builder, octave.zo);
            let x = builder.ins().fadd(x, xo);
            let y = builder.ins().fadd(y, yo);
            let z = builder.ins().fadd(z, zo);

            let xf = math::fast_floor(builder, x);
            let yf = math::fast_floor(builder, y);
            let zf = math::fast_floor(builder, z);

            let xf_f = builder.ins().fcvt_from_sint(types::F64, xf);
            let yf_f = builder.ins().fcvt_from_sint(types::F64, yf);
            let zf_f = builder.ins().fcvt_from_sint(types::F64, zf);
            let xr = builder.ins().fsub(x, xf_f);
            let yr = builder.ins().fsub(y, yf_f);
            let zr = builder.ins().fsub(z, zf_f);

            emit_sample_and_lerp(
                builder,
                flags,
                gradient,
                octave.table,
                Corner {
                    xf,
                    yf,
                    zf,
                    xr,
                    yr,
                    zr,
                    yr_original: yr,
                },
            )
        }
        Sample::Xz { x, z } => {
            let xo = math::f64_const(builder, octave.xo);
            let zo = math::f64_const(builder, octave.zo);
            let x = builder.ins().fadd(x, xo);
            let z = builder.ins().fadd(z, zo);

            let xf = math::fast_floor(builder, x);
            let zf = math::fast_floor(builder, z);

            let xf_f = builder.ins().fcvt_from_sint(types::F64, xf);
            let zf_f = builder.ins().fcvt_from_sint(types::F64, zf);
            let xr = builder.ins().fsub(x, xf_f);
            let zr = builder.ins().fsub(z, zf_f);

            let yf = builder.ins().iconst(types::I32, i64::from(octave.yo_floor));
            let yr = math::f64_const(builder, octave.yo_fraction);

            emit_sample_and_lerp(
                builder,
                flags,
                gradient,
                octave.table,
                Corner {
                    xf,
                    yf,
                    zf,
                    xr,
                    yr,
                    zr,
                    yr_original: yr,
                },
            )
        }
        Sample::Xy { x, y } => {
            let xo = math::f64_const(builder, octave.xo);
            let yo = math::f64_const(builder, octave.yo);
            let x = builder.ins().fadd(x, xo);
            let y = builder.ins().fadd(y, yo);

            let xf = math::fast_floor(builder, x);
            let yf = math::fast_floor(builder, y);

            let xf_f = builder.ins().fcvt_from_sint(types::F64, xf);
            let yf_f = builder.ins().fcvt_from_sint(types::F64, yf);
            let xr = builder.ins().fsub(x, xf_f);
            let yr = builder.ins().fsub(y, yf_f);

            let zf = builder.ins().iconst(types::I32, i64::from(octave.zo_floor));
            let zr = math::f64_const(builder, octave.zo_fraction);

            emit_sample_and_lerp(
                builder,
                flags,
                gradient,
                octave.table,
                Corner {
                    xf,
                    yf,
                    zf,
                    xr,
                    yr,
                    zr,
                    yr_original: yr,
                },
            )
        }
    }
}

fn perlin(
    builder: &mut FunctionBuilder<'_>,
    flags: MemFlagsData,
    gradient: Value,
    octaves: &[Octave],
    sample: Sample,
) -> Value {
    let mut value = math::f64_const(builder, 0.0);
    for octave in octaves {
        let factor = math::f64_const(builder, octave.input_factor);
        let scaled = match sample {
            Sample::Xyz { x, y, z } => Sample::Xyz {
                x: {
                    let v = builder.ins().fmul(x, factor);
                    math::wrap(builder, v)
                },
                y: {
                    let v = builder.ins().fmul(y, factor);
                    math::wrap(builder, v)
                },
                z: {
                    let v = builder.ins().fmul(z, factor);
                    math::wrap(builder, v)
                },
            },
            Sample::Xz { x, z } => Sample::Xz {
                x: {
                    let v = builder.ins().fmul(x, factor);
                    math::wrap(builder, v)
                },
                z: {
                    let v = builder.ins().fmul(z, factor);
                    math::wrap(builder, v)
                },
            },
            Sample::Xy { x, y } => Sample::Xy {
                x: {
                    let v = builder.ins().fmul(x, factor);
                    math::wrap(builder, v)
                },
                y: {
                    let v = builder.ins().fmul(y, factor);
                    math::wrap(builder, v)
                },
            },
        };

        let noise_val = improved_noise(builder, flags, gradient, octave, scaled);
        let out = math::f64_const(builder, octave.output_factor);
        let weighted = builder.ins().fmul(out, noise_val);
        value = builder.ins().fadd(value, weighted);
    }
    value
}

pub(crate) struct NormalNoisePlan {
    first: Vec<Octave>,
    second: Vec<Octave>,
    value_factor: f64,
}

impl NormalNoisePlan {
    pub(crate) fn new<E>(
        noise: &NormalNoise,
        intern: &mut impl FnMut(&[u8; 256]) -> Result<Value, E>,
    ) -> Result<Self, E> {
        fn octaves<E>(
            perlin: &PerlinNoise,
            intern: &mut impl FnMut(&[u8; 256]) -> Result<Value, E>,
        ) -> Result<Vec<Octave>, E> {
            perlin
                .active_octaves()
                .map(|snapshot| {
                    let table = intern(snapshot.noise.permutations())?;
                    Ok(Octave::new(&snapshot, table))
                })
                .collect()
        }

        Ok(Self {
            first: octaves(noise.first(), intern)?,
            second: octaves(noise.second(), intern)?,
            value_factor: noise.value_factor(),
        })
    }

    pub(crate) fn emit(
        &self,
        builder: &mut FunctionBuilder<'_>,
        flags: MemFlagsData,
        gradient: Value,
        sample: Sample,
    ) -> Value {
        let first = perlin(builder, flags, gradient, &self.first, sample);
        let scaled = sample.scaled(builder, INPUT_FACTOR);
        let second = perlin(builder, flags, gradient, &self.second, scaled);

        let sum = builder.ins().fadd(first, second);
        let factor = math::f64_const(builder, self.value_factor);
        builder.ins().fmul(sum, factor)
    }
}
