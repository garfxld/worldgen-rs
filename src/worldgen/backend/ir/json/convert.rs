use std::collections::BTreeMap;
use std::sync::Arc;

use serde::Deserialize;

use super::{NoiseRouterJson, SplineJson, SplinePointJson};
use crate::worldgen::backend::ir::types::{
    BlendAlpha, BlendDensity, BlendOffset, BlendedNoise, Clamp, Constant, CubicSpline,
    DensityFunction, FindTopSurface, IntervalSelect, Mapped, MappedType, Marker, MarkerType,
    Noise, RangeChoice, Reference, Shift, ShiftA, ShiftB, ShiftedNoise, Spline, SplinePoint,
    SplineValue, TwoArgType, TwoArgumentSimple, WeirdScaledSampler, YClampedGradient,
    normalize_id,
};

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct UnaryArg {
    #[serde(alias = "argument")]
    input: Arc<DensityFunction>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BinaryArgs {
    argument1: Arc<DensityFunction>,
    argument2: Arc<DensityFunction>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum DensityFunctionWire {
    Number(f64),
    Reference(String),
    Node(Box<DensityFunctionJson>),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub(crate) enum DensityFunctionJson {
    #[serde(rename = "minecraft:constant")]
    Constant(Constant),
    #[serde(rename = "minecraft:y_clamped_gradient")]
    YClampedGradient(YClampedGradient),
    #[serde(rename = "minecraft:noise")]
    Noise(Noise),
    #[serde(rename = "minecraft:shifted_noise")]
    ShiftedNoise(ShiftedNoise),
    #[serde(rename = "minecraft:shift_a")]
    ShiftA(ShiftA),
    #[serde(rename = "minecraft:shift_b")]
    ShiftB(ShiftB),
    #[serde(rename = "minecraft:shift")]
    Shift(Shift),
    #[serde(rename = "minecraft:clamp")]
    Clamp(Clamp),
    #[serde(rename = "minecraft:abs")]
    Abs(UnaryArg),
    #[serde(rename = "minecraft:square")]
    Square(UnaryArg),
    #[serde(rename = "minecraft:cube")]
    Cube(UnaryArg),
    #[serde(rename = "minecraft:half_negative")]
    HalfNegative(UnaryArg),
    #[serde(rename = "minecraft:quarter_negative")]
    QuarterNegative(UnaryArg),
    #[serde(rename = "minecraft:invert")]
    Invert(UnaryArg),
    #[serde(rename = "minecraft:squeeze")]
    Squeeze(UnaryArg),
    #[serde(rename = "minecraft:add")]
    Add(BinaryArgs),
    #[serde(rename = "minecraft:mul")]
    Mul(BinaryArgs),
    #[serde(rename = "minecraft:min")]
    Min(BinaryArgs),
    #[serde(rename = "minecraft:max")]
    Max(BinaryArgs),
    #[serde(rename = "minecraft:spline")]
    Spline { spline: SplineJson },
    #[serde(rename = "minecraft:range_choice")]
    RangeChoice(RangeChoice),
    #[serde(rename = "minecraft:interval_select")]
    IntervalSelect(IntervalSelect),
    #[serde(rename = "minecraft:interpolated")]
    Interpolated(UnaryArg),
    #[serde(rename = "minecraft:flat_cache")]
    FlatCache(UnaryArg),
    #[serde(rename = "minecraft:cache_once")]
    CacheOnce(UnaryArg),
    #[serde(rename = "minecraft:cache_2d")]
    Cache2d(UnaryArg),
    #[serde(rename = "minecraft:cache_all_in_cell")]
    CacheAllInCell(UnaryArg),
    #[serde(rename = "minecraft:blend_offset")]
    BlendOffset(BlendOffset),
    #[serde(rename = "minecraft:blend_alpha")]
    BlendAlpha(BlendAlpha),
    #[serde(rename = "minecraft:blend_density")]
    BlendDensity(BlendDensity),
    #[serde(rename = "minecraft:beardifier")]
    Beardifier {},
    #[serde(rename = "minecraft:end_islands")]
    EndIslands {},
    #[serde(rename = "minecraft:weird_scaled_sampler")]
    WeirdScaledSampler(WeirdScaledSampler),
    #[serde(rename = "minecraft:old_blended_noise")]
    OldBlendedNoise(BlendedNoise),
    #[serde(rename = "minecraft:find_top_surface")]
    FindTopSurface(FindTopSurface),
}

impl From<DensityFunctionWire> for DensityFunction {
    fn from(wire: DensityFunctionWire) -> Self {
        match wire {
            DensityFunctionWire::Number(value) => Self::Constant(Constant { value }),
            DensityFunctionWire::Reference(id) => Self::Reference(Reference {
                id: normalize_id(&id),
            }),
            DensityFunctionWire::Node(node) => (*node).into(),
        }
    }
}

impl From<DensityFunctionJson> for DensityFunction {
    fn from(node: DensityFunctionJson) -> Self {
        match node {
            DensityFunctionJson::Constant(v) => Self::Constant(v),
            DensityFunctionJson::YClampedGradient(v) => Self::YClampedGradient(v),
            DensityFunctionJson::Noise(v) => Self::Noise(v),
            DensityFunctionJson::ShiftedNoise(v) => Self::ShiftedNoise(v),
            DensityFunctionJson::ShiftA(v) => Self::ShiftA(v),
            DensityFunctionJson::ShiftB(v) => Self::ShiftB(v),
            DensityFunctionJson::Shift(v) => Self::Shift(v),
            DensityFunctionJson::Clamp(v) => Self::Clamp(v),
            DensityFunctionJson::Abs(a) => mapped(MappedType::Abs, a),
            DensityFunctionJson::Square(a) => mapped(MappedType::Square, a),
            DensityFunctionJson::Cube(a) => mapped(MappedType::Cube, a),
            DensityFunctionJson::HalfNegative(a) => mapped(MappedType::HalfNegative, a),
            DensityFunctionJson::QuarterNegative(a) => mapped(MappedType::QuarterNegative, a),
            DensityFunctionJson::Invert(a) => mapped(MappedType::Invert, a),
            DensityFunctionJson::Squeeze(a) => mapped(MappedType::Squeeze, a),
            DensityFunctionJson::Add(a) => two_arg(TwoArgType::Add, a),
            DensityFunctionJson::Mul(a) => two_arg(TwoArgType::Mul, a),
            DensityFunctionJson::Min(a) => two_arg(TwoArgType::Min, a),
            DensityFunctionJson::Max(a) => two_arg(TwoArgType::Max, a),
            DensityFunctionJson::Spline { spline } => Self::Spline(Spline {
                spline: Arc::new(json_spline_to_cubic(&spline)),
            }),
            DensityFunctionJson::RangeChoice(v) => Self::RangeChoice(v),
            DensityFunctionJson::IntervalSelect(v) => Self::IntervalSelect(v),
            DensityFunctionJson::Interpolated(a) => marker(MarkerType::Interpolated, a),
            DensityFunctionJson::FlatCache(a) => marker(MarkerType::FlatCache, a),
            DensityFunctionJson::CacheOnce(a) => marker(MarkerType::CacheOnce, a),
            DensityFunctionJson::Cache2d(a) => marker(MarkerType::Cache2D, a),
            DensityFunctionJson::CacheAllInCell(a) => marker(MarkerType::CacheAllInCell, a),
            DensityFunctionJson::BlendOffset(v) => Self::BlendOffset(v),
            DensityFunctionJson::BlendAlpha(v) => Self::BlendAlpha(v),
            DensityFunctionJson::BlendDensity(v) => Self::BlendDensity(v),
            DensityFunctionJson::Beardifier {} => Self::Constant(Constant { value: 0.0 }),
            DensityFunctionJson::EndIslands {} => Self::EndIslands,
            DensityFunctionJson::WeirdScaledSampler(v) => Self::WeirdScaledSampler(v),
            DensityFunctionJson::OldBlendedNoise(v) => Self::BlendedNoise(v),
            DensityFunctionJson::FindTopSurface(v) => Self::FindTopSurface(v),
        }
    }
}

fn mapped(op: MappedType, arg: UnaryArg) -> DensityFunction {
    DensityFunction::Mapped(Mapped {
        op,
        input: arg.input,
    })
}

fn two_arg(op: TwoArgType, args: BinaryArgs) -> DensityFunction {
    DensityFunction::TwoArgumentSimple(TwoArgumentSimple {
        op,
        argument1: args.argument1,
        argument2: args.argument2,
    })
}

fn marker(kind: MarkerType, arg: UnaryArg) -> DensityFunction {
    DensityFunction::Marker(Marker {
        kind,
        wrapped: arg.input,
    })
}

fn json_spline_to_cubic(spline: &SplineJson) -> CubicSpline {
    match spline {
        SplineJson::Constant(v) => CubicSpline::new(
            Arc::new(DensityFunction::constant(0.0)),
            vec![SplinePoint {
                location: 0.0,
                value: SplineValue::Constant(*v),
                derivative: 0.0,
            }],
        ),
        SplineJson::Multipoint { coordinate, points } => CubicSpline::new(
            Arc::new(DensityFunction::Reference(Reference {
                id: normalize_id(coordinate),
            })),
            points.iter().map(json_spline_point).collect(),
        ),
    }
}

fn json_spline_point(p: &SplinePointJson) -> SplinePoint {
    SplinePoint {
        location: p.location,
        value: match &p.value {
            SplineJson::Constant(v) => SplineValue::Constant(*v),
            multi @ SplineJson::Multipoint { .. } => {
                SplineValue::Spline(Arc::new(json_spline_to_cubic(multi)))
            }
        },
        derivative: p.derivative,
    }
}

#[must_use]
#[rustfmt::skip]
pub fn router_to_entries(router: &NoiseRouterJson) -> BTreeMap<String, DensityFunction> {
    let mut entries = BTreeMap::new();
    entries.insert("barrier".to_string(), router.barrier.clone());
    entries.insert("fluid_level_floodedness".to_string(), router.fluid_level_floodedness.clone());
    entries.insert("fluid_level_spread".to_string(), router.fluid_level_spread.clone());
    entries.insert("lava".to_string(), router.lava.clone());
    entries.insert("temperature".to_string(), router.temperature.clone());
    entries.insert("vegetation".to_string(), router.vegetation.clone());
    entries.insert("continentalness".to_string(), router.continents.clone());
    entries.insert("erosion".to_string(), router.erosion.clone());
    entries.insert("depth".to_string(), router.depth.clone());
    entries.insert("ridges".to_string(), router.ridges.clone());
    entries.insert("final_density".to_string(), router.final_density.clone());
    entries.insert("vein_toggle".to_string(), router.vein_toggle.clone());
    entries.insert("vein_ridged".to_string(), router.vein_ridged.clone());
    entries.insert("vein_gap".to_string(), router.vein_gap.clone());
    if let Some(ref psl) = router.preliminary_surface_level {
        entries.insert("preliminary_surface_level".to_string(), psl.clone());
    }
    entries
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test-only JSON literals are known valid at compile time"
)]
#[expect(
    clippy::float_cmp,
    reason = "values are exact literals round-tripped through JSON, not computed"
)]
mod tests {
    use super::*;
    use crate::worldgen::backend::ir::types::RarityValueMapper;

    #[test]
    fn parses_vanilla_density_function_json_into_ir_tree() {
        let json = r#"
            {
                "type": "minecraft:add",
                "argument1": 1.5,
                "argument2": {
                    "type": "minecraft:noise",
                    "noise": "overworld/continentalness",
                    "xz_scale": 0.25,
                    "y_scale": 0.0
                }
            }
        "#;

        let df: DensityFunction = serde_json::from_str(json).unwrap();

        let DensityFunction::TwoArgumentSimple(two_arg) = df else {
            panic!("expected a TwoArgumentSimple node");
        };
        assert_eq!(two_arg.op, TwoArgType::Add);
        assert!(matches!(*two_arg.argument1, DensityFunction::Constant(Constant { value }) if value == 1.5));

        let DensityFunction::Noise(noise) = &*two_arg.argument2 else {
            panic!("expected a Noise node");
        };
        assert_eq!(noise.noise_id, "minecraft:overworld/continentalness");
        assert_eq!(noise.xz_scale, 0.25);
    }

    #[test]
    fn unnamespaced_reference_defaults_to_minecraft() {
        let df: DensityFunction = serde_json::from_str("\"depth\"").unwrap();

        let DensityFunction::Reference(reference) = df else {
            panic!("expected a Reference node");
        };
        assert_eq!(reference.id, "minecraft:depth");
    }

    #[test]
    fn beardifier_collapses_to_zero_constant() {
        let df: DensityFunction = serde_json::from_str(r#"{"type": "minecraft:beardifier"}"#).unwrap();

        assert!(matches!(df, DensityFunction::Constant(Constant { value }) if value == 0.0));
    }

    #[test]
    fn weird_scaled_sampler_parses_rarity_mapper() {
        let json = r#"
            {
                "type": "minecraft:weird_scaled_sampler",
                "input": 0.0,
                "noise": "cave_layer",
                "rarity_value_mapper": "type_1"
            }
        "#;

        let df: DensityFunction = serde_json::from_str(json).unwrap();

        let DensityFunction::WeirdScaledSampler(sampler) = df else {
            panic!("expected a WeirdScaledSampler node");
        };
        assert_eq!(sampler.rarity_value_mapper, RarityValueMapper::Tunnels);
    }

    #[test]
    fn interval_select_rejects_mismatched_threshold_count() {
        let json = r#"
            {
                "type": "minecraft:interval_select",
                "input": 0.0,
                "thresholds": [0.0, 1.0],
                "functions": [0.0, 1.0]
            }
        "#;

        let result: Result<DensityFunction, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }
}
