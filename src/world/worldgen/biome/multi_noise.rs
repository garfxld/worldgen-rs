use shulkr::util::Key;

use crate::registry::biome::BiomeRef;
use crate::world::worldgen::biome::climate::{
    Parameter, ParameterList, ParameterPoint, quantize_coord,
};
use crate::world::worldgen::biome::overworld_biome_builder::OverworldBiomeBuilder;
use shulkr::registry::registries;

fn parameters(
    temperature: f32,
    humidity: f32,
    continentalness: f32,
    erosion: f32,
    depth: f32,
    weirdness: f32,
    offset: f32,
) -> ParameterPoint {
    ParameterPoint::new(
        Parameter::point(temperature),
        Parameter::point(humidity),
        Parameter::point(continentalness),
        Parameter::point(erosion),
        Parameter::point(depth),
        Parameter::point(weirdness),
        quantize_coord(f64::from(offset)),
    )
}

pub fn preset(id: &Key) -> Option<Vec<(ParameterPoint, &'static str)>> {
    match (id.namespace(), id.path()) {
        ("minecraft", "nether") => Some(vec![
            (
                parameters(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
                "minecraft:nether_wastes",
            ),
            (
                parameters(0.0, -0.5, 0.0, 0.0, 0.0, 0.0, 0.0),
                "minecraft:soul_sand_valley",
            ),
            (
                parameters(0.4, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
                "minecraft:crimson_forest",
            ),
            (
                parameters(0.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.375),
                "minecraft:warped_forest",
            ),
            (
                parameters(-0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.175),
                "minecraft:basalt_deltas",
            ),
        ]),
        ("minecraft", "overworld") => {
            let mut values = Vec::new();
            OverworldBiomeBuilder::default().add_biomes(&mut values);
            Some(values)
        }
        _ => None,
    }
}

pub fn parameter_list(
    values: Vec<(ParameterPoint, String)>,
) -> Result<ParameterList<BiomeRef>, String> {
    let biomes = &registries::BIOME;
    let values = values
        .into_iter()
        .map(|(point, biome)| {
            let key = Key::of(biome);
            biomes
                .by_key(&key)
                .map(|biome| (point, biome))
                .ok_or_else(|| format!("unknown biome {key}"))
        })
        .collect::<Result<_, _>>()?;
    Ok(ParameterList::new(values))
}

pub fn preset_parameter_list(id: &Key) -> Option<Result<ParameterList<BiomeRef>, String>> {
    preset(id).map(|values| {
        parameter_list(
            values
                .into_iter()
                .map(|(point, biome)| (point, biome.to_owned()))
                .collect(),
        )
    })
}
