use std::collections::BTreeMap;
use std::sync::Arc;

use rustc_hash::FxHashMap;
use worldgen_rs::worldgen::backend::ir::{
    CubicSpline, DensityFunction, DensityProgram, Spline, SplinePoint, SplineValue,
    YClampedGradient,
};
use worldgen_rs::worldgen::backend::jit::{CompiledExpr, JitModule};
use worldgen_rs::worldgen::density::spline_eval;

fn empty_program() -> DensityProgram {
    DensityProgram {
        registry: BTreeMap::new(),
        router_entries: BTreeMap::new(),
        prefix: "Test".to_string(),
        cell_width: 4,
        legacy_random_source: false,
    }
}

fn coordinate() -> Arc<DensityFunction> {
    Arc::new(DensityFunction::YClampedGradient(YClampedGradient {
        from_y: 0,
        to_y: 1,
        from_value: 0.0,
        to_value: 1.0,
    }))
}

const fn constant_point(location: f32, value: f32, derivative: f32) -> SplinePoint {
    SplinePoint {
        location,
        value: SplineValue::Constant(value),
        derivative,
    }
}

fn compile(spline: CubicSpline) -> CompiledExpr {
    let df = DensityFunction::Spline(Spline {
        spline: Arc::new(spline),
    });
    let module = JitModule::new().expect("JIT unavailable on this host");
    CompiledExpr::compile(
        module,
        &df,
        &empty_program(),
        &FxHashMap::default(),
        None,
        false,
    )
    .expect("spline should compile")
}

fn probe_inputs(locations: &[f32]) -> Vec<f64> {
    let mut inputs = vec![-1000.0_f64, 1000.0];
    for &location in locations {
        let l = f64::from(location);
        inputs.extend([l - 0.25, l, l + 0.25, l + 0.5]);
    }
    inputs
}

fn eval_compiled(compiled: &CompiledExpr, input: f64) -> f64 {
    compiled.eval(0.0, input, 0.0)
}

#[test]
fn multipoint_spline_matches_evaluator_bit_exactly() {
    let points = vec![
        constant_point(-1.0, 0.5, 0.0),
        constant_point(-0.2, -0.3, 1.5),
        constant_point(0.4, 2.0, -0.5),
        constant_point(1.1, 0.25, 0.75),
    ];
    let locations: Vec<f32> = points.iter().map(|p| p.location).collect();
    let derivatives: Vec<f32> = points.iter().map(|p| p.derivative).collect();
    let values: Vec<f32> = points
        .iter()
        .map(|p| match p.value {
            SplineValue::Constant(c) => c,
            SplineValue::Spline(_) => unreachable!("constant points only"),
        })
        .collect();

    let compiled = compile(CubicSpline::new(coordinate(), points.clone()));

    for input in probe_inputs(&locations) {
        let coord = input.clamp(0.0, 1.0);
        let expected =
            spline_eval::evaluate_spline(&locations, &derivatives, coord as f32, |i| values[i]);
        let actual = eval_compiled(&compiled, input);
        assert_eq!(
            actual.to_bits(),
            f64::from(expected).to_bits(),
            "spline mismatch at coord {coord}: {actual} != {expected}"
        );
    }
}

#[test]
fn nested_spline_matches_evaluator_bit_exactly() {
    let inner = Arc::new(CubicSpline::new(
        coordinate(),
        vec![
            constant_point(0.0, 1.0, 0.0),
            constant_point(0.5, -2.0, 0.5),
            constant_point(1.0, 3.0, 0.0),
        ],
    ));

    let outer_points = vec![
        constant_point(-0.5, 0.0, 0.0),
        SplinePoint {
            location: 0.25,
            value: SplineValue::Spline(Arc::clone(&inner)),
            derivative: 0.5,
        },
        constant_point(0.9, 4.0, -1.0),
    ];

    let compiled = compile(CubicSpline::new(coordinate(), outer_points.clone()));

    let inner_locations: Vec<f32> = inner.points.iter().map(|p| p.location).collect();
    let inner_derivatives: Vec<f32> = inner.points.iter().map(|p| p.derivative).collect();
    let inner_values: Vec<f32> = inner
        .points
        .iter()
        .map(|p| match p.value {
            SplineValue::Constant(c) => c,
            SplineValue::Spline(_) => unreachable!("constant points only"),
        })
        .collect();

    let outer_locations: Vec<f32> = outer_points.iter().map(|p| p.location).collect();
    let outer_derivatives: Vec<f32> = outer_points.iter().map(|p| p.derivative).collect();

    for input in probe_inputs(&outer_locations) {
        let coord = (input.clamp(0.0, 1.0)) as f32;
        let expected =
            spline_eval::evaluate_spline(&outer_locations, &outer_derivatives, coord, |i| {
                match &outer_points[i].value {
                    SplineValue::Constant(c) => *c,
                    SplineValue::Spline(_) => spline_eval::evaluate_spline(
                        &inner_locations,
                        &inner_derivatives,
                        coord,
                        |j| inner_values[j],
                    ),
                }
            });
        let actual = eval_compiled(&compiled, input);
        assert_eq!(
            actual.to_bits(),
            f64::from(expected).to_bits(),
            "nested spline mismatch at coord {coord}"
        );
    }
}

#[test]
fn single_point_spline_extrapolates_both_directions() {
    let points = vec![constant_point(0.3, 1.25, 2.0)];
    let locations = [0.3_f32];
    let derivatives = [2.0_f32];

    let compiled = compile(CubicSpline::new(coordinate(), points));

    for input in probe_inputs(&locations) {
        let coord = (input.clamp(0.0, 1.0)) as f32;
        let expected = spline_eval::evaluate_spline(&locations, &derivatives, coord, |_| 1.25_f32);
        let actual = eval_compiled(&compiled, input);
        assert_eq!(
            actual.to_bits(),
            f64::from(expected).to_bits(),
            "single-point spline mismatch at coord {coord}"
        );
    }
}
