use std::slice;

use serde::Deserialize;

mod convert;

pub(crate) use convert::DensityFunctionWire;
pub use convert::router_to_entries;

use crate::worldgen::backend::ir::types::DensityFunction;

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum SplineJson {
    Constant(f32),
    Multipoint {
        coordinate: String,
        #[serde(default)]
        points: Vec<SplinePointJson>,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct SplinePointJson {
    pub location: f32,
    pub value: SplineJson,
    pub derivative: f32,
}

#[derive(Deserialize)]
pub struct NoiseRouterJson {
    pub barrier: DensityFunction,
    pub fluid_level_floodedness: DensityFunction,
    pub fluid_level_spread: DensityFunction,
    pub lava: DensityFunction,
    pub temperature: DensityFunction,
    pub vegetation: DensityFunction,
    pub continents: DensityFunction,
    pub erosion: DensityFunction,
    pub depth: DensityFunction,
    pub ridges: DensityFunction,
    pub preliminary_surface_level: Option<DensityFunction>,
    pub final_density: DensityFunction,
    pub vein_toggle: DensityFunction,
    pub vein_ridged: DensityFunction,
    pub vein_gap: DensityFunction,
}

#[derive(Deserialize)]
pub struct NoiseConfigJson {
    pub min_y: i32,
    pub height: i32,
    pub size_horizontal: i32,
    pub size_vertical: i32,
}

#[derive(Deserialize)]
pub struct BlockStateJson {
    #[serde(rename = "Name")]
    pub name: String,
}

#[derive(Deserialize)]
pub struct MultiNoiseBiomeParametersJson {
    pub biomes: Vec<MultiNoiseBiomeEntryJson>,
}

#[derive(Deserialize)]
pub struct MultiNoiseBiomeEntryJson {
    pub biome: String,
    pub t: [i64; 2],
    pub h: [i64; 2],
    pub c: [i64; 2],
    pub e: [i64; 2],
    pub d: [i64; 2],
    pub w: [i64; 2],
    pub o: i64,
}

#[derive(Deserialize)]
pub struct NoiseSettingsJson {
    pub sea_level: i32,
    pub ore_veins_enabled: bool,
    pub aquifers_enabled: bool,
    #[serde(default)]
    pub legacy_random_source: bool,
    pub default_block: BlockStateJson,
    pub default_fluid: BlockStateJson,
    pub noise: NoiseConfigJson,
    pub noise_router: NoiseRouterJson,
    #[serde(default)]
    pub surface_rule: Option<SurfaceRuleJson>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum SurfaceRuleJson {
    #[serde(rename = "minecraft:block")]
    Block { result_state: ResultStateJson },
    #[serde(rename = "minecraft:sequence")]
    Sequence { sequence: Vec<SurfaceRuleJson> },
    #[serde(rename = "minecraft:condition")]
    Condition {
        if_true: SurfaceConditionJson,
        then_run: Box<SurfaceRuleJson>,
    },
    #[serde(rename = "minecraft:bandlands")]
    Bandlands {},
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResultStateJson {
    #[serde(rename = "Name")]
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum SurfaceConditionJson {
    #[serde(rename = "minecraft:stone_depth")]
    StoneDepth {
        offset: i32,
        add_surface_depth: bool,
        secondary_depth_range: i32,
        surface_type: String,
    },
    #[serde(rename = "minecraft:above_preliminary_surface")]
    AbovePreliminarySurface {},
    #[serde(rename = "minecraft:biome")]
    BiomeIs { biome_is: SingleOrList<BiomeIdJson> },
    #[serde(rename = "minecraft:noise_threshold")]
    NoiseThreshold {
        noise: String,
        #[serde(default)]
        is_3d: bool,
        min_threshold: f64,
        max_threshold: f64,
    },
    #[serde(rename = "minecraft:vertical_gradient")]
    VerticalGradient {
        random_name: String,
        true_at_and_below: VerticalAnchorJson,
        false_at_and_above: VerticalAnchorJson,
    },
    #[serde(rename = "minecraft:y_above")]
    YAbove {
        anchor: VerticalAnchorJson,
        surface_depth_multiplier: i32,
        add_stone_depth: bool,
    },
    #[serde(rename = "minecraft:water")]
    Water {
        offset: i32,
        surface_depth_multiplier: i32,
        add_stone_depth: bool,
    },
    #[serde(rename = "minecraft:temperature")]
    Temperature {},
    #[serde(rename = "minecraft:steep")]
    Steep {},
    #[serde(rename = "minecraft:hole")]
    Hole {},
    #[serde(rename = "minecraft:not")]
    Not { invert: Box<SurfaceConditionJson> },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(transparent)]
pub struct BiomeIdJson(String);

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum SingleOrList<T> {
    Single(T),
    List(Vec<T>),
}

impl<T> SingleOrList<T> {
    pub fn as_slice(&self) -> &[T] {
        match self {
            Self::Single(value) => slice::from_ref(value),
            Self::List(values) => values,
        }
    }
}

impl BiomeIdJson {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum VerticalAnchorJson {
    Absolute { absolute: i32 },
    AboveBottom { above_bottom: i32 },
    BelowTop { below_top: i32 },
}
