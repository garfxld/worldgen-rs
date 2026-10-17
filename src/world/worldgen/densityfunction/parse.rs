use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use serde_json::Value;
use shulkr::util::Axis;

use crate::world::worldgen::synth::normal_noise::NormalNoise;

use super::function::{
    BinaryType, BlendedNoise, DensityFunction, Df, DistanceMetric, Gradient, Holder, NoiseHolder,
    RoundType, SimpleType, TilingMode, UnaryType,
};
use super::spline::CubicSpline;

pub struct ParseContext<'a> {
    pub holders: &'a BTreeMap<String, Arc<OnceLock<Df>>>,
    pub noises: &'a BTreeMap<String, Arc<NormalNoise>>,
}

pub fn normalize_id(id: &str) -> String {
    if id.contains(':') {
        id.to_owned()
    } else {
        format!("minecraft:{id}")
    }
}

fn field<'v>(v: &'v Value, name: &str) -> Result<&'v Value, String> {
    v.get(name)
        .ok_or_else(|| format!("missing field `{name}` in {v}"))
}

fn f32_field(v: &Value, name: &str) -> Result<f32, String> {
    field(v, name)?
        .as_f64()
        .map(|d| d as f32)
        .ok_or_else(|| format!("field `{name}` is not a number"))
}

fn f64_field(v: &Value, name: &str) -> Result<f64, String> {
    field(v, name)?
        .as_f64()
        .ok_or_else(|| format!("field `{name}` is not a number"))
}

fn i32_field(v: &Value, name: &str) -> Result<i32, String> {
    let raw = field(v, name)?;
    raw.as_i64()
        .or_else(|| raw.as_f64().map(|d| d as i64))
        .map(|i| i as i32)
        .ok_or_else(|| format!("field `{name}` is not an integer"))
}

fn str_field<'v>(v: &'v Value, name: &str) -> Result<&'v str, String> {
    field(v, name)?
        .as_str()
        .ok_or_else(|| format!("field `{name}` is not a string"))
}

fn axis(raw: &str) -> Result<Axis, String> {
    match raw {
        "x" => Ok(Axis::X),
        "y" => Ok(Axis::Y),
        "z" => Ok(Axis::Z),
        other => Err(format!("unknown axis {other}")),
    }
}

impl ParseContext<'_> {
    fn noise(&self, v: &Value, name: &str) -> Result<NoiseHolder, String> {
        let key = normalize_id(
            field(v, name)?
                .as_str()
                .ok_or("inline noise parameters cannot be instantiated by RandomState")?,
        );
        let noise = self
            .noises
            .get(&key)
            .ok_or_else(|| format!("unknown noise {key}"))?
            .clone();
        Ok(NoiseHolder { key, noise })
    }

    fn sub(&self, v: &Value, name: &str) -> Result<Df, String> {
        self.parse(field(v, name)?)
    }

    fn optional_sub(&self, v: &Value, name: &str, default: f32) -> Result<Df, String> {
        match v.get(name) {
            Some(inner) => self.parse(inner),
            None => Ok(DensityFunction::constant(default)),
        }
    }

    pub fn holder(&self, key: &str) -> Result<Df, String> {
        let key = normalize_id(key);
        let value = self
            .holders
            .get(&key)
            .ok_or_else(|| format!("unknown density function {key}"))?
            .clone();
        Ok(Arc::new(DensityFunction::Holder(Holder { key, value })))
    }

    pub fn parse(&self, v: &Value) -> Result<Df, String> {
        if let Some(number) = v.as_f64() {
            return Ok(DensityFunction::constant(number as f32));
        }
        if let Some(key) = v.as_str() {
            return self.holder(key);
        }
        let kind = normalize_id(str_field(v, "type")?);
        let kind = kind.strip_prefix("minecraft:").unwrap_or(&kind);
        let unary = |t: UnaryType| -> Result<Df, String> {
            Ok(Arc::new(DensityFunction::Unary(t, self.sub(v, "input")?)))
        };
        let binary = |t: BinaryType| -> Result<Df, String> {
            Ok(Arc::new(DensityFunction::Binary(
                t,
                self.sub(v, "left")?,
                self.sub(v, "right")?,
            )))
        };
        let round = |t: RoundType| -> Result<Df, String> {
            Ok(Arc::new(DensityFunction::Round(
                t,
                self.sub(v, "input")?,
                self.optional_sub(v, "multiple", 1.0)?,
            )))
        };
        let df = match kind {
            "constant" => DensityFunction::Constant(f32_field(v, "value")?),
            "blend_alpha" => DensityFunction::Simple(SimpleType::BlendAlpha),
            "blend_offset" => DensityFunction::Simple(SimpleType::BlendOffset),
            "beardifier" => DensityFunction::Simple(SimpleType::Beardifier),
            "noise" => DensityFunction::Noise {
                noise: self.noise(v, "noise")?,
                xz_scale: f64_field(v, "xz_scale")?,
                y_scale: f64_field(v, "y_scale")?,
                shift_x: self.optional_sub(v, "shift_x", 0.0)?,
                shift_y: self.optional_sub(v, "shift_y", 0.0)?,
                shift_z: self.optional_sub(v, "shift_z", 0.0)?,
            },
            "end_outer_islands" => DensityFunction::EndOuterIslands,
            "distance_to_point" => {
                let point = field(v, "point")?
                    .as_array()
                    .filter(|a| a.len() == 3)
                    .ok_or("point must be a list of three integers")?;
                let p = |i: usize| point[i].as_i64().unwrap_or(0) as i32;
                DensityFunction::DistanceToPoint {
                    point: [p(0), p(1), p(2)],
                    metric: match str_field(v, "metric")? {
                        "euclidean" => DistanceMetric::Euclidean,
                        "euclidean_squared" => DistanceMetric::EuclideanSquared,
                        "manhattan" => DistanceMetric::Manhattan,
                        "chebyshev" => DistanceMetric::Chebyshev,
                        other => return Err(format!("unknown distance metric {other}")),
                    },
                }
            }
            "gradient" => {
                let gradient = Gradient {
                    axis: axis(str_field(v, "axis")?)?,
                    tiling: match v.get("tiling").and_then(Value::as_str) {
                        None | Some("clamp_to_edge") => TilingMode::ClampToEdge,
                        Some("repeat") => TilingMode::Repeat,
                        Some("mirrored_repeat") => TilingMode::MirroredRepeat,
                        Some(other) => return Err(format!("unknown tiling mode {other}")),
                    },
                    from_coordinate: i32_field(v, "from_coordinate")?,
                    to_coordinate: i32_field(v, "to_coordinate")?,
                    from_value: f32_field(v, "from_value")?,
                    to_value: f32_field(v, "to_value")?,
                };
                if gradient.from_coordinate == gradient.to_coordinate {
                    return Err("from_coordinate cannot be equal to to_coordinate".to_owned());
                }
                DensityFunction::Gradient(gradient)
            }
            "shift_a" => DensityFunction::ShiftA(self.noise(v, "noise")?),
            "shift_b" => DensityFunction::ShiftB(self.noise(v, "noise")?),
            "shift" => DensityFunction::Shift(self.noise(v, "noise")?),
            "abs" => return unary(UnaryType::Abs),
            "square" => return unary(UnaryType::Square),
            "cube" => return unary(UnaryType::Cube),
            "sqrt" => return unary(UnaryType::Sqrt),
            "half_negative" => return unary(UnaryType::HalfNegative),
            "quarter_negative" => return unary(UnaryType::QuarterNegative),
            "reciprocal" => return unary(UnaryType::Reciprocal),
            "negate" => return unary(UnaryType::Negate),
            "squeeze" => return unary(UnaryType::Squeeze),
            "log" => return unary(UnaryType::Log),
            "sign" => return unary(UnaryType::Sign),
            "floor" => return round(RoundType::Floor),
            "round" => return round(RoundType::Round),
            "ceil" => return round(RoundType::Ceil),
            "truncate" => return round(RoundType::Truncate),
            "add" => return binary(BinaryType::Add),
            "sub" => return binary(BinaryType::Sub),
            "mul" => return binary(BinaryType::Mul),
            "div" => return binary(BinaryType::Div),
            "min" => return binary(BinaryType::Min),
            "max" => return binary(BinaryType::Max),
            "pow" => DensityFunction::Pow(self.sub(v, "base")?, self.sub(v, "exponent")?),
            "spline" => DensityFunction::Spline(self.spline(field(v, "spline")?)?),
            "lerp" => DensityFunction::Lerp(
                self.sub(v, "alpha")?,
                self.sub(v, "first")?,
                self.sub(v, "second")?,
            ),
            "clamp" => {
                let min = f32_field(v, "min")?;
                let max = f32_field(v, "max")?;
                if max < min {
                    return Err(format!(
                        "min ({min}) must be less than or equal to max ({max})"
                    ));
                }
                DensityFunction::Clamp(self.sub(v, "input")?, min, max)
            }
            "range_choice" => DensityFunction::RangeChoice {
                input: self.sub(v, "input")?,
                min_inclusive: f32_field(v, "min_inclusive")?,
                max_exclusive: f32_field(v, "max_exclusive")?,
                when_in_range: self.sub(v, "when_in_range")?,
                when_out_of_range: self.sub(v, "when_out_of_range")?,
            },
            "interval_select" => {
                let thresholds: Vec<f32> = field(v, "thresholds")?
                    .as_array()
                    .ok_or("thresholds must be a list")?
                    .iter()
                    .map(|t| {
                        t.as_f64()
                            .map(|d| d as f32)
                            .ok_or("threshold must be a number")
                    })
                    .collect::<Result<_, _>>()?;
                let functions: Vec<Df> = field(v, "functions")?
                    .as_array()
                    .ok_or("functions must be a list")?
                    .iter()
                    .map(|f| self.parse(f))
                    .collect::<Result<_, _>>()?;
                if functions.len() < 2 {
                    return Err("interval_select needs at least two functions".to_owned());
                }
                if thresholds.len() != functions.len() - 1 {
                    return Err(format!(
                        "Expected {} thresholds for {} functions, but got {}",
                        functions.len() - 1,
                        functions.len(),
                        thresholds.len()
                    ));
                }
                if !thresholds.windows(2).all(|w| w[0] <= w[1]) {
                    return Err(
                        "Threshold values must be ordered from smallest to largest".to_owned()
                    );
                }
                DensityFunction::IntervalSelect {
                    input: self.sub(v, "input")?,
                    thresholds,
                    functions,
                }
            }
            "cache" => DensityFunction::Cache(self.sub(v, "input")?),
            "blend_density" => DensityFunction::BlendDensity(self.sub(v, "input")?),
            "interpolated" => DensityFunction::Interpolated {
                input: self.sub(v, "input")?,
                cell_size_xz: i32_field(v, "cell_size_xz")?,
                cell_size_y: i32_field(v, "cell_size_y")?,
            },
            "slice" => DensityFunction::Slice {
                axis: axis(str_field(v, "axis")?)?,
                coordinate: i32_field(v, "coordinate")?,
                input: self.sub(v, "input")?,
            },
            "find_top_surface" => DensityFunction::FindTopSurface {
                density: self.sub(v, "density")?,
                upper_bound: self.sub(v, "upper_bound")?,
                lower_bound: i32_field(v, "lower_bound")?,
                cell_height: i32_field(v, "cell_height")?,
            },
            "old_blended_noise" => DensityFunction::OldBlendedNoise(BlendedNoise {
                xz_scale: f64_field(v, "xz_scale")?,
                y_scale: f64_field(v, "y_scale")?,
                xz_factor: f64_field(v, "xz_factor")?,
                y_factor: f64_field(v, "y_factor")?,
                smear_scale_multiplier: f64_field(v, "smear_scale_multiplier")?,
            }),
            other => return Err(format!("unknown density function type minecraft:{other}")),
        };
        Ok(Arc::new(df))
    }

    pub fn spline(&self, v: &Value) -> Result<Arc<CubicSpline>, String> {
        if let Some(number) = v.as_f64() {
            return Ok(Arc::new(CubicSpline::Constant(number as f32)));
        }
        let coordinate = self.sub(v, "coordinate")?;
        let points = field(v, "points")?
            .as_array()
            .filter(|p| !p.is_empty())
            .ok_or("spline points must be a non-empty list")?;
        let mut locations = Vec::with_capacity(points.len());
        let mut values = Vec::with_capacity(points.len());
        let mut derivatives = Vec::with_capacity(points.len());
        for point in points {
            locations.push(f32_field(point, "location")?);
            values.push(self.spline(field(point, "value")?)?);
            derivatives.push(f32_field(point, "derivative")?);
        }
        Ok(Arc::new(CubicSpline::Multipoint {
            coordinate,
            locations,
            values,
            derivatives,
        }))
    }
}
