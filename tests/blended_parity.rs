use std::collections::BTreeMap;

use rustc_hash::FxHashMap;
use worldgen_rs::util::noise::BlendedNoise;
use worldgen_rs::util::rng::RandomSource;
use worldgen_rs::util::rng::xoroshiro::XoroshiroRandomSource;
use worldgen_rs::worldgen::backend::ir::{
    BlendedNoise as BlendedNoiseIr, DensityFunction, DensityProgram,
};
use worldgen_rs::worldgen::backend::jit::{CompiledExpr, JitModule};

fn empty_program() -> DensityProgram {
    DensityProgram {
        registry: BTreeMap::new(),
        router_entries: BTreeMap::new(),
        prefix: "Test".to_string(),
        cell_width: 4,
        legacy_random_source: false,
    }
}

const XZ_SCALE: f64 = 0.25;
const Y_SCALE: f64 = 0.125;
const XZ_FACTOR: f64 = 80.0;
const Y_FACTOR: f64 = 160.0;
const SMEAR: f64 = 8.0;

fn build_blended(seed: u64) -> BlendedNoise {
    let mut random = RandomSource::Xoroshiro(XoroshiroRandomSource::from_seed(seed));
    BlendedNoise::new(&mut random, XZ_SCALE, Y_SCALE, XZ_FACTOR, Y_FACTOR, SMEAR)
}

fn compile(blended: &BlendedNoise) -> CompiledExpr {
    let df = DensityFunction::BlendedNoise(BlendedNoiseIr {
        xz_scale: XZ_SCALE,
        y_scale: Y_SCALE,
        xz_factor: XZ_FACTOR,
        y_factor: Y_FACTOR,
        smear_scale_multiplier: SMEAR,
    });
    let module = JitModule::new().expect("JIT unavailable on this host");
    CompiledExpr::compile(
        module,
        &df,
        &empty_program(),
        &FxHashMap::default(),
        Some(blended),
        false,
    )
    .expect("blended noise should compile")
}

fn probe_points() -> Vec<(f64, f64, f64)> {
    let horizontal = [0.0, 1.0, -1.0, 7.0, 16.0, -16.0, 233.0, -1024.0, 30_000.0];
    let vertical = [-64.0, -32.0, 0.0, 1.0, 63.0, 64.0, 128.0, 319.0, 384.0];

    let mut points = Vec::new();
    for x in horizontal {
        for y in vertical {
            for z in horizontal {
                points.push((x, y, z));
            }
        }
    }
    points
}

#[test]
fn blended_noise_matches_compute_bit_exactly() {
    for seed in [42_u64, 1, 987_654_321] {
        let blended = build_blended(seed);
        let compiled = compile(&blended);

        for (x, y, z) in probe_points() {
            let expected = blended.compute(x, y, z);
            let actual = compiled.eval(x, y, z);
            assert_eq!(
                actual.to_bits(),
                expected.to_bits(),
                "blended noise mismatch (seed {seed}) at ({x}, {y}, {z}): {actual} != {expected}"
            );
        }
    }
}
