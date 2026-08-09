use std::collections::BTreeMap;

use rustc_hash::FxHashMap;
use worldgen_rs::util::noise::NormalNoise;
use worldgen_rs::util::rng::xoroshiro::XoroshiroRandomSource;
use worldgen_rs::util::rng::{RandomFactory, RandomSource};
use worldgen_rs::worldgen::backend::ir::{
    DensityFunction, DensityProgram, Noise, Shift, ShiftA, ShiftB,
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

const NOISE_ID: &str = "test:noise";

fn splitter(seed: u64) -> RandomFactory {
    let mut rng = RandomSource::Xoroshiro(XoroshiroRandomSource::from_seed(seed));
    rng.next_positional()
}

fn bind(noise: NormalNoise) -> FxHashMap<String, NormalNoise> {
    let mut map = FxHashMap::default();
    map.insert(NOISE_ID.to_string(), noise);
    map
}

fn compile(
    df: &DensityFunction,
    noises: &FxHashMap<String, NormalNoise>,
    is_flat: bool,
) -> CompiledExpr {
    let module = JitModule::new().expect("JIT unavailable on this host");
    CompiledExpr::compile(module, df, &empty_program(), noises, None, is_flat)
        .expect("density function should compile")
}

fn probe_points() -> Vec<(f64, f64, f64)> {
    let axis = [
        0.0,
        1.0,
        -1.0,
        0.5,
        -0.5,
        17.25,
        -17.25,
        255.0,
        256.0,
        -256.0,
        4096.0,
        -4096.0,
        33_554_432.0,
        -33_554_432.0,
        50_000_000.0,
    ];
    let mut points = Vec::new();
    for x in axis {
        for y in axis {
            for z in axis {
                points.push((x, y, z));
            }
        }
    }
    points
}

#[test]
fn noise_3d_matches_sampler_bit_exactly() {
    let noise = NormalNoise::create(&splitter(42), NOISE_ID, -7, &[1.0, 1.0, 2.0, 0.0, 1.0]);
    let xz_scale = 0.25;
    let y_scale = 0.125;

    let df = DensityFunction::Noise(Noise {
        noise_id: NOISE_ID.to_string(),
        xz_scale,
        y_scale,
    });

    let noises = bind(noise);
    let compiled = compile(&df, &noises, false);
    let reference = &noises[NOISE_ID];

    for (x, y, z) in probe_points() {
        let expected = reference.get_value(x * xz_scale, y * y_scale, z * xz_scale);
        let actual = compiled.eval(x, y, z);
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "3D noise mismatch at ({x}, {y}, {z}): {actual} != {expected}"
        );
    }
}

#[test]
fn y_independent_noise_takes_the_xz_path_bit_exactly() {
    let noise = NormalNoise::create(&splitter(7), NOISE_ID, -5, &[1.0, 1.0, 1.0]);
    let xz_scale = 0.75;

    let df = DensityFunction::Noise(Noise {
        noise_id: NOISE_ID.to_string(),
        xz_scale,
        y_scale: 0.0,
    });

    let noises = bind(noise);
    let compiled = compile(&df, &noises, false);
    let reference = &noises[NOISE_ID];

    for (x, y, z) in probe_points() {
        let expected = reference.get_value_xz(x * xz_scale, z * xz_scale);
        let actual = compiled.eval(x, y, z);
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "XZ noise mismatch at ({x}, {y}, {z})"
        );

        let at_y_zero = reference.get_value(x * xz_scale, 0.0, z * xz_scale);
        assert_eq!(
            expected.to_bits(),
            at_y_zero.to_bits(),
            "get_value_xz stopped matching get_value at y = 0 ({x}, {z})"
        );
    }
}

#[test]
fn shift_family_matches_sampler_bit_exactly() {
    let noise = NormalNoise::create(&splitter(1234), NOISE_ID, -3, &[1.0, 1.0]);
    let noises = bind(noise);
    let reference = &noises[NOISE_ID];

    let shift = compile(
        &DensityFunction::Shift(Shift {
            noise_id: NOISE_ID.to_string(),
        }),
        &noises,
        false,
    );
    let shift_flat = compile(
        &DensityFunction::Shift(Shift {
            noise_id: NOISE_ID.to_string(),
        }),
        &noises,
        true,
    );
    let shift_a = compile(
        &DensityFunction::ShiftA(ShiftA {
            noise_id: NOISE_ID.to_string(),
        }),
        &noises,
        false,
    );
    let shift_b = compile(
        &DensityFunction::ShiftB(ShiftB {
            noise_id: NOISE_ID.to_string(),
        }),
        &noises,
        false,
    );

    for (x, y, z) in probe_points() {
        let expected_shift = reference.get_value(x * 0.25, y * 0.25, z * 0.25) * 4.0;
        assert_eq!(
            shift.eval(x, y, z).to_bits(),
            expected_shift.to_bits(),
            "shift mismatch at ({x}, {y}, {z})"
        );

        let expected_flat = reference.get_value_xz(x * 0.25, z * 0.25) * 4.0;
        assert_eq!(
            shift_flat.eval(x, y, z).to_bits(),
            expected_flat.to_bits(),
            "flat shift mismatch at ({x}, {z})"
        );
        assert_eq!(
            shift_a.eval(x, y, z).to_bits(),
            expected_flat.to_bits(),
            "shift_a mismatch at ({x}, {z})"
        );

        let expected_b = reference.get_value_xy(z * 0.25, x * 0.25) * 4.0;
        assert_eq!(
            shift_b.eval(x, y, z).to_bits(),
            expected_b.to_bits(),
            "shift_b mismatch at ({x}, {z})"
        );
    }
}
