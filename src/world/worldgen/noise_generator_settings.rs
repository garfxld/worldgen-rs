use std::io::{Error as IoError, ErrorKind};

use serde::de::Error as DeError;
use serde_json::Value;
use shulkr::util::Key;
use shulkr::world::block::BlockState;

use crate::datapack::{Datapack, DatapackError, read};
use crate::world::worldgen::aquifer;
use crate::world::worldgen::biome::climate::Parameter;
use crate::world::worldgen::densityfunction::function::Df;
use crate::world::worldgen::densityfunction::registry::DensityRegistry;
use crate::world::worldgen::material::registry::MaterialRegistry;
use crate::world::worldgen::material::rule::MaterialRule;

const DIMENSION_Y_SIZE: i32 = 4064;
const DIMENSION_MAX_Y: i32 = (DIMENSION_Y_SIZE >> 1) - 1;
const DIMENSION_MIN_Y: i32 = DIMENSION_MAX_Y - DIMENSION_Y_SIZE + 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoiseSettings {
    pub min_y: i32,
    pub height: i32,
}

impl NoiseSettings {
    pub const fn clamp_to_height_accessor(self, min_y: i32, max_y: i32) -> Self {
        let new_min_y = self.min_y.max(min_y);
        let new_height = (self.min_y + self.height).min(max_y + 1) - new_min_y;
        Self {
            min_y: new_min_y,
            height: new_height,
        }
    }
}

#[derive(Clone)]
pub struct NoiseRouter {
    pub temperature: Df,
    pub vegetation: Df,
    pub continents: Df,
    pub erosion: Df,
    pub depth: Df,
    pub ridges: Df,
    pub chunk_surface_level: Df,
    pub final_density: Df,
}

#[derive(Clone)]
pub struct SpawnTargetPoint {
    pub parameters: Vec<(Df, Parameter)>,
}

#[derive(Clone)]
pub struct NoiseGeneratorSettings {
    pub noise_settings: NoiseSettings,
    pub default_block: BlockState,
    pub default_fluid: BlockState,
    pub noise_router: NoiseRouter,
    pub material_rule: MaterialRule,
    pub spawn_target: Vec<SpawnTargetPoint>,
    pub sea_level: i32,
    pub disable_mob_generation: bool,
    pub aquifers: Option<aquifer::Config>,
    pub use_legacy_random_source: bool,
}

fn field<'a>(v: &'a Value, name: &str) -> Result<&'a Value, String> {
    v.get(name).ok_or_else(|| format!("missing field {name}"))
}

fn int(v: &Value, name: &str) -> Result<i32, String> {
    field(v, name)?
        .as_i64()
        .map(|n| n as i32)
        .ok_or_else(|| format!("{name} is not an integer"))
}

fn boolean(v: &Value, name: &str) -> Result<bool, String> {
    field(v, name)?
        .as_bool()
        .ok_or_else(|| format!("{name} is not a boolean"))
}

fn block_state(v: &Value, name: &str) -> Result<BlockState, String> {
    serde_json::from_value(field(v, name)?.clone()).map_err(|e| format!("{name}: {e}"))
}

fn float(v: &Value) -> Result<f32, String> {
    v.as_f64()
        .map(|n| n as f32)
        .ok_or_else(|| format!("{v} is not a number"))
}

pub(crate) fn parameter(v: &Value) -> Result<Parameter, String> {
    let (min, max) = if let Some(list) = v.as_array() {
        if list.len() != 2 {
            return Err(format!(
                "parameter range must have 2 elements, got {}",
                list.len()
            ));
        }
        (float(&list[0])?, float(&list[1])?)
    } else if v.is_object() {
        (float(field(v, "min")?)?, float(field(v, "max")?)?)
    } else {
        let point = float(v)?;
        (point, point)
    };
    for bound in [min, max] {
        if !(-2.0..=2.0).contains(&bound) {
            return Err(format!("parameter {bound} outside of range [-2.0:2.0]"));
        }
    }
    if min > max {
        return Err(format!(
            "Cannon construct interval, min > max ({min} > {max})"
        ));
    }
    Ok(Parameter::span(min, max))
}

impl NoiseSettings {
    fn parse(v: &Value) -> Result<Self, String> {
        let settings = Self {
            min_y: int(v, "min_y")?,
            height: int(v, "height")?,
        };
        if !(DIMENSION_MIN_Y..=DIMENSION_MAX_Y).contains(&settings.min_y) {
            return Err(format!(
                "min_y {} outside of range [{DIMENSION_MIN_Y}:{DIMENSION_MAX_Y}]",
                settings.min_y
            ));
        }
        if !(0..=DIMENSION_Y_SIZE).contains(&settings.height) {
            return Err(format!(
                "height {} outside of range [0:{DIMENSION_Y_SIZE}]",
                settings.height
            ));
        }
        if settings.min_y + settings.height > DIMENSION_MAX_Y + 1 {
            return Err(format!(
                "min_y + height cannot be higher than: {}",
                DIMENSION_MAX_Y + 1
            ));
        }
        if settings.height % 16 != 0 {
            return Err("height has to be a multiple of 16".to_owned());
        }
        if settings.min_y % 16 != 0 {
            return Err("min_y has to be a multiple of 16".to_owned());
        }
        Ok(settings)
    }
}

impl NoiseRouter {
    fn parse(registry: &DensityRegistry, v: &Value) -> Result<Self, String> {
        let f = |name: &str| registry.parse(field(v, name)?);
        Ok(Self {
            temperature: f("temperature")?,
            vegetation: f("vegetation")?,
            continents: f("continents")?,
            erosion: f("erosion")?,
            depth: f("depth")?,
            ridges: f("ridges")?,
            chunk_surface_level: f("chunk_surface_level")?,
            final_density: f("final_density")?,
        })
    }
}

impl SpawnTargetPoint {
    fn parse(registry: &DensityRegistry, v: &Value) -> Result<Self, String> {
        let map = v.as_object().ok_or("spawn target is not a map")?;
        let parameters = map
            .iter()
            .map(|(key, range)| Ok((registry.holder(key)?, parameter(range)?)))
            .collect::<Result<_, String>>()?;
        Ok(Self { parameters })
    }
}

impl NoiseGeneratorSettings {
    pub fn parse(
        registry: &DensityRegistry,
        materials: &MaterialRegistry,
        v: &Value,
    ) -> Result<Self, String> {
        let aquifers = v
            .get("aquifers")
            .map(|a| aquifer::Config::parse(registry, a))
            .transpose()?;
        let spawn_target = field(v, "spawn_target")?
            .as_array()
            .ok_or("spawn_target is not a list")?
            .iter()
            .map(|point| SpawnTargetPoint::parse(registry, point))
            .collect::<Result<_, String>>()?;
        Ok(Self {
            noise_settings: NoiseSettings::parse(field(v, "noise")?)?,
            default_block: block_state(v, "default_block")?,
            default_fluid: block_state(v, "default_fluid")?,
            noise_router: NoiseRouter::parse(registry, field(v, "noise_router")?)?,
            material_rule: materials.parse_rule(registry, field(v, "material_rule")?)?,
            spawn_target,
            sea_level: int(v, "sea_level")?,
            disable_mob_generation: boolean(v, "disable_mob_generation")?,
            aquifers,
            use_legacy_random_source: boolean(v, "legacy_random_source")?,
        })
    }

    pub fn load(
        pack: &Datapack,
        registry: &DensityRegistry,
        materials: &MaterialRegistry,
        id: &Key,
    ) -> Result<Self, DatapackError> {
        let relative = format!("worldgen/noise_settings/{}.json", id.path());
        let path =
            pack.files(id.namespace(), &relative)
                .pop()
                .ok_or_else(|| DatapackError::Read {
                    path: relative.clone().into(),
                    source: IoError::new(
                        ErrorKind::NotFound,
                        format!("{id} not present in the datapack"),
                    ),
                })?;
        let value: Value =
            serde_json::from_str(&read(&path)?).map_err(|source| DatapackError::Parse {
                path: path.clone(),
                source,
            })?;
        Self::parse(registry, materials, &value).map_err(|message| DatapackError::Parse {
            path,
            source: DeError::custom(message),
        })
    }
}
