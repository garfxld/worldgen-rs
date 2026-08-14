use cranelift_codegen::ir::condcodes::FloatCC;
use cranelift_codegen::ir::{InstBuilder, MemFlagsData, Value, types};
use cranelift_frontend::FunctionBuilder;

use crate::util::noise::{BlendedNoiseSnapshot, ImprovedNoise, PerlinNoise};

use super::math;
use super::noise::{Corner, emit_sample_and_lerp};

const SHIFT_UP_EPSILON: f64 = 1.0e-7_f32 as f64;

const MAIN_OCTAVES: usize = 8;
const LIMIT_OCTAVES: usize = 16;

struct FudgeOctave {
    xo: f64,
    yo: f64,
    zo: f64,
    table: Value,
}

impl FudgeOctave {
    const fn new(noise: &ImprovedNoise, table: Value) -> Self {
        Self {
            xo: noise.xo,
            yo: noise.yo,
            zo: noise.zo,
            table,
        }
    }
}

pub(crate) struct BlendedNoisePlan {
    main: Vec<Option<FudgeOctave>>,
    min_limit: Vec<Option<FudgeOctave>>,
    max_limit: Vec<Option<FudgeOctave>>,
    xz_multiplier: f64,
    y_multiplier: f64,
    xz_factor: f64,
    y_factor: f64,
    smear_scale_multiplier: f64,
}

impl BlendedNoisePlan {
    pub(crate) fn new<E>(
        blended: BlendedNoiseSnapshot<'_>,
        intern: &mut impl FnMut(&[u8; 256]) -> Result<Value, E>,
    ) -> Result<Self, E> {
        fn octaves<E>(
            perlin: &PerlinNoise,
            count: usize,
            intern: &mut impl FnMut(&[u8; 256]) -> Result<Value, E>,
        ) -> Result<Vec<Option<FudgeOctave>>, E> {
            (0..count)
                .map(|i| {
                    perlin.get_octave_noise(i).map_or_else(
                        || Ok(None),
                        |noise| {
                            let table = intern(noise.permutations())?;
                            Ok(Some(FudgeOctave::new(noise, table)))
                        },
                    )
                })
                .collect()
        }

        Ok(Self {
            main: octaves(blended.main_noise, MAIN_OCTAVES, intern)?,
            min_limit: octaves(blended.min_limit_noise, LIMIT_OCTAVES, intern)?,
            max_limit: octaves(blended.max_limit_noise, LIMIT_OCTAVES, intern)?,
            xz_multiplier: blended.xz_multiplier,
            y_multiplier: blended.y_multiplier,
            xz_factor: blended.xz_factor,
            y_factor: blended.y_factor,
            smear_scale_multiplier: blended.smear_scale_multiplier,
        })
    }

    pub(crate) fn emit(
        &self,
        builder: &mut FunctionBuilder<'_>,
        flags: MemFlagsData,
        gradient: Value,
        x: Value,
        y: Value,
        z: Value,
    ) -> Value {
        let xz_mul = math::f64_const(builder, self.xz_multiplier);
        let y_mul = math::f64_const(builder, self.y_multiplier);
        let limit_x = builder.ins().fmul(x, xz_mul);
        let limit_y = builder.ins().fmul(y, y_mul);
        let limit_z = builder.ins().fmul(z, xz_mul);

        let xz_factor = math::f64_const(builder, self.xz_factor);
        let y_factor = math::f64_const(builder, self.y_factor);
        let main_x = builder.ins().fdiv(limit_x, xz_factor);
        let main_y = builder.ins().fdiv(limit_y, y_factor);
        let main_z = builder.ins().fdiv(limit_z, xz_factor);

        let limit_smear = self.y_multiplier * self.smear_scale_multiplier;
        let main_smear = limit_smear / self.y_factor;

        let main_value = self.emit_octave_sum(
            builder,
            flags,
            gradient,
            &self.main,
            Coords {
                x: main_x,
                y: main_y,
                z: main_z,
            },
            main_smear,
        );

        let ten = math::f64_const(builder, 10.0);
        let one = math::f64_const(builder, 1.0);
        let two = math::f64_const(builder, 2.0);
        let scaled_main = builder.ins().fdiv(main_value, ten);
        let factor = {
            let sum = builder.ins().fadd(scaled_main, one);
            builder.ins().fdiv(sum, two)
        };

        let limit_coords = Coords {
            x: limit_x,
            y: limit_y,
            z: limit_z,
        };

        let is_max = builder.ins().fcmp(FloatCC::GreaterThanOrEqual, factor, one);
        let zero = math::f64_const(builder, 0.0);
        let is_min = builder.ins().fcmp(FloatCC::LessThanOrEqual, factor, zero);

        let blend_min = self.emit_skippable_sum(
            builder,
            flags,
            gradient,
            &self.min_limit,
            limit_coords,
            limit_smear,
            is_max,
        );
        let blend_max = self.emit_skippable_sum(
            builder,
            flags,
            gradient,
            &self.max_limit,
            limit_coords,
            limit_smear,
            is_min,
        );

        let five_twelve = math::f64_const(builder, 512.0);
        let min_scaled = builder.ins().fdiv(blend_min, five_twelve);
        let max_scaled = builder.ins().fdiv(blend_max, five_twelve);

        let lerped = math::lerp(builder, factor, min_scaled, max_scaled);
        let below = builder.ins().fcmp(FloatCC::LessThan, factor, zero);
        let above = builder.ins().fcmp(FloatCC::GreaterThan, factor, one);
        let upper = builder.ins().select(above, max_scaled, lerped);
        let clamped = builder.ins().select(below, min_scaled, upper);

        let one_twenty_eight = math::f64_const(builder, 128.0);
        builder.ins().fdiv(clamped, one_twenty_eight)
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "emission plumbing, not a public API"
    )]
    fn emit_skippable_sum(
        &self,
        builder: &mut FunctionBuilder<'_>,
        flags: MemFlagsData,
        gradient: Value,
        octaves: &[Option<FudgeOctave>],
        coords: Coords,
        smear: f64,
        skip: Value,
    ) -> Value {
        let compute = builder.create_block();
        let merge = builder.create_block();
        builder.append_block_param(merge, types::F64);

        let zero = math::f64_const(builder, 0.0);
        builder
            .ins()
            .brif(skip, merge, &[zero.into()], compute, &[]);

        builder.switch_to_block(compute);
        builder.seal_block(compute);
        let sum = self.emit_octave_sum(builder, flags, gradient, octaves, coords, smear);
        builder.ins().jump(merge, &[sum.into()]);

        builder.switch_to_block(merge);
        builder.seal_block(merge);
        builder.block_params(merge)[0]
    }

    fn emit_octave_sum(
        &self,
        builder: &mut FunctionBuilder<'_>,
        flags: MemFlagsData,
        gradient: Value,
        octaves: &[Option<FudgeOctave>],
        coords: Coords,
        smear: f64,
    ) -> Value {
        let mut sum = math::f64_const(builder, 0.0);
        let mut pow = 1.0_f64;

        for octave in octaves {
            if let Some(octave) = octave {
                let pow_v = math::f64_const(builder, pow);
                let sx = {
                    let v = builder.ins().fmul(coords.x, pow_v);
                    math::wrap(builder, v)
                };
                let sy = {
                    let v = builder.ins().fmul(coords.y, pow_v);
                    math::wrap(builder, v)
                };
                let sz = {
                    let v = builder.ins().fmul(coords.z, pow_v);
                    math::wrap(builder, v)
                };
                let y_fudge = builder.ins().fmul(coords.y, pow_v);

                let sampled = emit_noise_with_y_scale(
                    builder,
                    flags,
                    gradient,
                    octave,
                    sx,
                    sy,
                    sz,
                    smear * pow,
                    y_fudge,
                );
                let contribution = builder.ins().fdiv(sampled, pow_v);
                sum = builder.ins().fadd(sum, contribution);
            }
            pow /= 2.0;
        }

        sum
    }
}

#[derive(Clone, Copy)]
struct Coords {
    x: Value,
    y: Value,
    z: Value,
}

#[expect(clippy::too_many_arguments, reason = "mirrors noise_with_y_scale")]
fn emit_noise_with_y_scale(
    builder: &mut FunctionBuilder<'_>,
    flags: MemFlagsData,
    gradient: Value,
    octave: &FudgeOctave,
    x: Value,
    y: Value,
    z: Value,
    y_scale: f64,
    y_fudge: Value,
) -> Value {
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

    let shifted_yr = if y_scale == 0.0 {
        yr
    } else {
        let zero = math::f64_const(builder, 0.0);
        let scale = math::f64_const(builder, y_scale);
        let epsilon = math::f64_const(builder, SHIFT_UP_EPSILON);

        let non_negative = builder
            .ins()
            .fcmp(FloatCC::GreaterThanOrEqual, y_fudge, zero);
        let below_yr = builder.ins().fcmp(FloatCC::LessThan, y_fudge, yr);
        let use_fudge = builder.ins().band(non_negative, below_yr);
        let fudge_limit = builder.ins().select(use_fudge, y_fudge, yr);

        let ratio = builder.ins().fdiv(fudge_limit, scale);
        let biased = builder.ins().fadd(ratio, epsilon);
        let floored = builder.ins().floor(biased);
        let yr_fudge = builder.ins().fmul(floored, scale);
        builder.ins().fsub(yr, yr_fudge)
    };

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
            yr: shifted_yr,
            zr,
            yr_original: yr,
        },
    )
}
