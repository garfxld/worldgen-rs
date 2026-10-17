use crate::world::worldgen::densityfunction::context::SamplerContext;
use crate::world::worldgen::densityfunction::eval;
use crate::world::worldgen::densityfunction::eval::{end_island_height, interp_value};
use crate::world::worldgen::densityfunction::function::java_round;
use crate::world::worldgen::densityfunction::sampler;
use crate::world::worldgen::densityfunction::sampler::SamplerNode;
use crate::world::worldgen::synth::gradient_noise::GRADIENT;
use crate::world::worldgen::synth::{Noise, SimplexNoise};

pub static GRADIENT_F32: [[f32; 4]; 16] = {
    let mut table = [[0.0f32; 4]; 16];
    let mut i = 0;
    while i < 16 {
        table[i] = [
            GRADIENT[i].x as f32,
            GRADIENT[i].y as f32,
            GRADIENT[i].z as f32,
            0.0,
        ];
        i += 1;
    }
    table
};

pub unsafe extern "C" fn value_boundary(
    node: *const SamplerNode,
    ctx: *mut u8,
    x: i32,
    y: i32,
    z: i32,
) -> f32 {
    // SAFETY: `ctx` is the `SamplerContext` that `sample_value` passed to the
    // compiled function, exclusively borrowed for the duration of the call.
    let ctx = unsafe { &mut *ctx.cast::<SamplerContext<'static>>() };
    // SAFETY: only called from JIT code, which embeds the address of a node in
    // the tree that owns the compiled code, so the node outlives the call.
    interp_value(unsafe { &*node }, ctx, x, y, z)
}

pub unsafe extern "C" fn noise_get(noise: *const Noise, x: f64, y: f64, z: f64) -> f32 {
    // SAFETY: only called from JIT code, which embeds the address of a noise
    // owned by the sampler tree that owns the compiled code.
    unsafe { &*noise }.get(x, y, z)
}

pub unsafe extern "C" fn end_island(noise: *const SimplexNoise, x: i32, z: i32) -> f32 {
    // SAFETY: only called from JIT code, which embeds the address of a noise
    // owned by the sampler tree that owns the compiled code.
    let noise = unsafe { &*noise };
    (end_island_height(noise, x / 8, z / 8) - 8.0) / 128.0
}

pub extern "C" fn pow(base: f64, exponent: f64) -> f64 {
    base.powf(exponent)
}

pub extern "C" fn ln(value: f64) -> f64 {
    value.ln()
}

pub const extern "C" fn round(value: f32) -> f32 {
    java_round(value) as f32
}

pub unsafe extern "C" fn gradient(node: *const SamplerNode, coordinate: i32) -> f32 {
    let sampler::Sampler::Gradient {
        tiling,
        from_coordinate,
        from_value,
        coordinate_factor,
        ..
    } = &{
        // SAFETY: only called from JIT code, which embeds the address of a node in
        // the tree that owns the compiled code, so the node outlives the call.
        unsafe { &*node }
    }
    .sampler
    else {
        return f32::NAN;
    };
    eval::gradient_compute(
        *tiling,
        *from_coordinate,
        *from_value,
        *coordinate_factor,
        coordinate,
    )
}
