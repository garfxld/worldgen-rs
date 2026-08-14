use std::sync::Arc;

use serde::{Deserialize, Deserializer, de::Error as _};

pub(crate) fn normalize_id(id: &str) -> String {
    if id.contains(':') {
        id.to_owned()
    } else {
        format!("minecraft:{id}")
    }
}

fn deserialize_normalized_id<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let raw = String::deserialize(d)?;
    Ok(normalize_id(&raw))
}

#[derive(Debug, Clone, Deserialize)]
pub struct Constant {
    #[serde(alias = "argument")]
    pub value: f64,
}

#[derive(Debug, Clone)]
pub struct Reference {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct YClampedGradient {
    pub from_y: i32,
    pub to_y: i32,
    pub from_value: f64,
    pub to_value: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Noise {
    #[serde(rename = "noise", deserialize_with = "deserialize_normalized_id")]
    pub noise_id: String,
    pub xz_scale: f64,
    pub y_scale: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ShiftedNoise {
    pub shift_x: Arc<DensityFunction>,
    pub shift_y: Arc<DensityFunction>,
    pub shift_z: Arc<DensityFunction>,
    pub xz_scale: f64,
    pub y_scale: f64,
    #[serde(rename = "noise", deserialize_with = "deserialize_normalized_id")]
    pub noise_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ShiftA {
    #[serde(rename = "argument", deserialize_with = "deserialize_normalized_id")]
    pub noise_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ShiftB {
    #[serde(rename = "argument", deserialize_with = "deserialize_normalized_id")]
    pub noise_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Shift {
    #[serde(rename = "argument", deserialize_with = "deserialize_normalized_id")]
    pub noise_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TwoArgType {
    Add,
    Mul,
    Min,
    Max,
}

#[derive(Debug, Clone)]
pub struct TwoArgumentSimple {
    pub op: TwoArgType,
    pub argument1: Arc<DensityFunction>,
    pub argument2: Arc<DensityFunction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappedType {
    Abs,
    Square,
    Cube,
    HalfNegative,
    QuarterNegative,
    Invert,
    Squeeze,
}

#[derive(Debug, Clone)]
pub struct Mapped {
    pub op: MappedType,
    pub input: Arc<DensityFunction>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Clamp {
    pub input: Arc<DensityFunction>,
    pub min: f64,
    pub max: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RangeChoice {
    pub input: Arc<DensityFunction>,
    pub min_inclusive: f64,
    pub max_exclusive: f64,
    pub when_in_range: Arc<DensityFunction>,
    pub when_out_of_range: Arc<DensityFunction>,
}

#[derive(Debug, Clone)]
pub struct IntervalSelect {
    pub input: Arc<DensityFunction>,
    pub thresholds: Vec<f64>,
    pub functions: Vec<Arc<DensityFunction>>,
}

impl<'de> Deserialize<'de> for IntervalSelect {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            input: Arc<DensityFunction>,
            thresholds: Vec<f64>,
            functions: Vec<Arc<DensityFunction>>,
        }
        let raw = Raw::deserialize(d)?;

        if raw.functions.len() < 2 {
            return Err(D::Error::custom(format!(
                "minecraft:interval_select requires at least two functions, got {}",
                raw.functions.len()
            )));
        }
        if raw.thresholds.len() != raw.functions.len() - 1 {
            return Err(D::Error::custom(format!(
                "minecraft:interval_select requires exactly one more function than thresholds, got {} thresholds and {} functions",
                raw.thresholds.len(),
                raw.functions.len()
            )));
        }
        if !raw.thresholds.windows(2).all(|pair| pair[0] <= pair[1]) {
            return Err(D::Error::custom(
                "minecraft:interval_select thresholds must be ordered from smallest to largest",
            ));
        }

        Ok(Self {
            input: raw.input,
            thresholds: raw.thresholds,
            functions: raw.functions,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct BlendedNoise {
    pub xz_scale: f64,
    pub y_scale: f64,
    pub xz_factor: f64,
    pub y_factor: f64,
    pub smear_scale_multiplier: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RarityValueMapper {
    Tunnels,
    Caves,
}

impl RarityValueMapper {
    pub fn get_values(self, rarity: f64) -> f64 {
        match self {
            Self::Tunnels => {
                if rarity < -0.5 {
                    0.75
                } else if rarity < 0.0 {
                    1.0
                } else if rarity < 0.5 {
                    1.5
                } else {
                    2.0
                }
            }
            Self::Caves => {
                if rarity < -0.75 {
                    0.5
                } else if rarity < -0.5 {
                    0.75
                } else if rarity < 0.5 {
                    1.0
                } else if rarity < 0.75 {
                    2.0
                } else {
                    3.0
                }
            }
        }
    }
}

fn deserialize_rarity_value_mapper<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<RarityValueMapper, D::Error> {
    let raw = String::deserialize(d)?;
    Ok(match raw.as_str() {
        "type_1" => RarityValueMapper::Tunnels,
        _ => RarityValueMapper::Caves,
    })
}

#[derive(Debug, Clone, Deserialize)]
pub struct WeirdScaledSampler {
    pub input: Arc<DensityFunction>,
    #[serde(rename = "noise", deserialize_with = "deserialize_normalized_id")]
    pub noise_id: String,
    #[serde(deserialize_with = "deserialize_rarity_value_mapper")]
    pub rarity_value_mapper: RarityValueMapper,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BlendDensity {
    #[serde(rename = "argument")]
    pub input: Arc<DensityFunction>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FindTopSurface {
    pub density: Arc<DensityFunction>,
    pub upper_bound: Arc<DensityFunction>,
    pub lower_bound: i32,
    pub cell_height: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerType {
    Interpolated,
    FlatCache,
    Cache2D,
    CacheOnce,
    CacheAllInCell,
}

#[derive(Debug, Clone)]
pub struct Marker {
    pub kind: MarkerType,
    pub wrapped: Arc<DensityFunction>,
}

#[derive(Debug, Clone)]
pub struct Spline {
    pub spline: Arc<CubicSpline>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct BlendAlpha {}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct BlendOffset {}

#[derive(Debug, Clone, Deserialize)]
#[serde(from = "crate::worldgen::backend::ir::json::DensityFunctionWire")]
pub enum DensityFunction {
    Constant(Constant),
    Reference(Reference),
    YClampedGradient(YClampedGradient),
    Noise(Noise),
    ShiftedNoise(ShiftedNoise),
    ShiftA(ShiftA),
    ShiftB(ShiftB),
    Shift(Shift),
    TwoArgumentSimple(TwoArgumentSimple),
    Mapped(Mapped),
    Clamp(Clamp),
    RangeChoice(RangeChoice),
    IntervalSelect(IntervalSelect),
    Spline(Spline),
    BlendedNoise(BlendedNoise),
    WeirdScaledSampler(WeirdScaledSampler),
    EndIslands,
    BlendAlpha(BlendAlpha),
    BlendOffset(BlendOffset),
    BlendDensity(BlendDensity),
    Marker(Marker),
    FindTopSurface(FindTopSurface),
}

impl DensityFunction {
    pub const fn constant(value: f64) -> Self {
        Self::Constant(Constant { value })
    }

    pub const fn reference(id: String) -> Self {
        Self::Reference(Reference { id })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct NoiseParameters {
    #[serde(rename = "firstOctave")]
    pub first_octave: i32,
    pub amplitudes: Vec<f64>,
}

impl NoiseParameters {
    pub const fn new(first_octave: i32, amplitudes: Vec<f64>) -> Self {
        Self {
            first_octave,
            amplitudes,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CubicSpline {
    pub coordinate: Arc<DensityFunction>,
    pub points: Vec<SplinePoint>,
    pub locations: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct SplinePoint {
    pub location: f32,
    pub value: SplineValue,
    pub derivative: f32,
}

#[derive(Debug, Clone)]
pub enum SplineValue {
    Constant(f32),
    Spline(Arc<CubicSpline>),
}

impl CubicSpline {
    pub fn new(coordinate: Arc<DensityFunction>, points: Vec<SplinePoint>) -> Self {
        let locations = points.iter().map(|p| p.location).collect();
        Self {
            coordinate,
            points,
            locations,
        }
    }
}
