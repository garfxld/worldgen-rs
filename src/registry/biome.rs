use std::path::Path;
use std::sync::LazyLock;

use shulkr::registry::{Ref, Registry};
use shulkr::world::biome::{Biome, TemperatureModifier};
use shulkr::world::block::Block;

use crate::datapack::{
    DatapackError, collect_json_files, parse_id, path_to_id, read_and_parse_normalized,
};

use crate::registry::fluid::is_water_fluid;
use crate::util::rng::RandomSource;
use crate::util::rng::legacy_random::LegacyRandomSource;
use crate::world::worldgen::block_survival::can_survive;
use crate::world::worldgen::region::WorldGenRegion;
use crate::world::worldgen::synth::{Noise, NoiseStack, SimplexNoise};
use shulkr::util::BlockPosition;

pub type BiomeRef = &'static Ref<Biome>;

fn height_adjusted_temperature(biome: &Biome, pos: BlockPosition, sea_level: i32) -> f32 {
    let adjusted_temperature =
        modify_temperature(biome.temperature_modifier(), pos, biome.temperature());
    let snow_level = sea_level + 17;
    if pos.y() > snow_level {
        let v = temperature_noise().get2(
            f64::from(pos.x() as f32 / 8.0),
            f64::from(pos.z() as f32 / 8.0),
        ) * 8.0;
        adjusted_temperature - (v + pos.y() as f32 - snow_level as f32) * 0.05 / 40.0
    } else {
        adjusted_temperature
    }
}

pub fn warm_enough_to_rain(biome: &Biome, pos: BlockPosition, sea_level: i32) -> bool {
    height_adjusted_temperature(biome, pos, sea_level) >= 0.15
}

pub fn cold_enough_to_snow(biome: &Biome, pos: BlockPosition, sea_level: i32) -> bool {
    !warm_enough_to_rain(biome, pos, sea_level)
}

pub fn should_melt_frozen_ocean_iceberg_slightly(
    biome: &Biome,
    pos: BlockPosition,
    sea_level: i32,
) -> bool {
    height_adjusted_temperature(biome, pos, sea_level) > 0.1
}

pub fn should_freeze(
    biome: &Biome,
    region: &WorldGenRegion<'_>,
    pos: BlockPosition,
    check_neighbors: bool,
) -> bool {
    if warm_enough_to_rain(biome, pos, region.sea_level()) {
        return false;
    }

    if region.is_inside_build_height(pos.y()) {
        let state = region.get_block(pos);
        if is_water_fluid(state.get_fluid_state().fluid()) && state.is(Block::WATER) {
            if !check_neighbors {
                return true;
            }

            let surrounded_by_water = is_water_at(region, pos.west())
                && is_water_at(region, pos.east())
                && is_water_at(region, pos.north())
                && is_water_at(region, pos.south());
            if !surrounded_by_water {
                return true;
            }
        }
    }

    false
}

pub fn should_snow(biome: &Biome, region: &WorldGenRegion<'_>, pos: BlockPosition) -> bool {
    if !biome.has_precipitation() || !cold_enough_to_snow(biome, pos, region.sea_level()) {
        return false;
    }

    if region.is_inside_build_height(pos.y()) {
        let state = region.get_block(pos);
        if (state.is_air() || state.is(Block::SNOW))
            && can_survive(region, pos, Block::SNOW.default_state())
        {
            return true;
        }
    }

    false
}

fn is_water_at(region: &WorldGenRegion<'_>, pos: BlockPosition) -> bool {
    is_water_fluid(region.get_block(pos).get_fluid_state().fluid())
}

fn temperature_noise() -> &'static SimplexNoise {
    static NOISE: LazyLock<SimplexNoise> = LazyLock::new(|| {
        let mut random = RandomSource::Legacy(LegacyRandomSource::from_seed(1234));
        SimplexNoise::new_discarding_offset(&mut random, true)
    });
    &NOISE
}

pub fn frozen_temperature_noise() -> &'static Noise {
    static NOISE: LazyLock<Noise> = LazyLock::new(|| {
        let mut random = RandomSource::Legacy(LegacyRandomSource::from_seed(3456));
        let a = SimplexNoise::new_discarding_offset(&mut random, true);
        let b = SimplexNoise::new_discarding_offset(&mut random, true);
        let c = SimplexNoise::new_discarding_offset(&mut random, true);
        Noise::Stack(
            NoiseStack::builder()
                .add(Noise::Simplex(a), 1.0, 0.142_857_15)
                .add(Noise::Simplex(b), 0.5, 0.285_714_3)
                .add(Noise::Simplex(c), 0.25, 0.571_428_6)
                .build(),
        )
    });
    &NOISE
}

pub fn biome_info_noise() -> &'static SimplexNoise {
    static NOISE: LazyLock<SimplexNoise> = LazyLock::new(|| {
        let mut random = RandomSource::Legacy(LegacyRandomSource::from_seed(2345));
        SimplexNoise::new_discarding_offset(&mut random, true)
    });
    &NOISE
}

fn modify_temperature(
    modifier: TemperatureModifier,
    pos: BlockPosition,
    base_temperature: f32,
) -> f32 {
    match modifier {
        TemperatureModifier::None => base_temperature,
        TemperatureModifier::Frozen => {
            let ground_value_large_variation = f64::from(
                frozen_temperature_noise()
                    .get2(f64::from(pos.x()) * 0.05, f64::from(pos.z()) * 0.05)
                    * 7.0,
            );
            let ground_value_edge_variation = f64::from(
                biome_info_noise().get2(f64::from(pos.x()) * 0.2, f64::from(pos.z()) * 0.2),
            );
            let ice_patches = ground_value_large_variation + ground_value_edge_variation;
            if ice_patches < 0.3 {
                let ground_value_small_variation = f64::from(
                    biome_info_noise().get2(f64::from(pos.x()) * 0.09, f64::from(pos.z()) * 0.09),
                );
                if ground_value_small_variation < 0.8 {
                    return 0.2;
                }
            }
            base_temperature
        }
    }
}

pub fn load_biomes(
    registry: &Registry<Biome>,
    namespace: &str,
    worldgen_dir: &Path,
) -> Result<(), DatapackError> {
    let dir = worldgen_dir.join("biome");
    for file in collect_json_files(&dir) {
        let biome: Biome = read_and_parse_normalized(&file)?;
        registry.register(parse_id(&path_to_id(namespace, &file, &dir)), biome)?;
    }
    Ok(())
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "biome registry tests assert required vanilla properties are present"
)]
mod tests {
    use super::*;
    use crate::datapack::Datapack;
    use crate::world::worldgen;
    use shulkr::registry::registries;
    use shulkr::util::Key;

    fn create_test_registry() -> &'static Registry<Biome> {
        worldgen::ensure_registry_initialized(&Datapack::builtin())
            .expect("vanilla datapack should load");
        &registries::BIOME
    }

    #[test]
    fn plains_has_expected_climate() {
        let registry = create_test_registry();
        let plains = registry.by_key(&Key::const_vanilla("plains")).unwrap();

        assert!(plains.has_precipitation());
        assert_eq!(plains.temperature(), 0.8);
        assert_eq!(plains.temperature_modifier(), TemperatureModifier::None);
        assert!(
            plains
                .generation_settings()
                .carvers()
                .contains(&Key::const_vanilla("cave"))
        );
    }

    #[test]
    fn frozen_ocean_uses_frozen_temperature_modifier() {
        let registry = create_test_registry();
        let frozen_ocean = registry
            .by_key(&Key::const_vanilla("frozen_ocean"))
            .unwrap();

        assert_eq!(
            frozen_ocean.temperature_modifier(),
            TemperatureModifier::Frozen
        );
    }

    #[test]
    fn nether_wastes_single_carver_is_not_dropped() {
        let registry = create_test_registry();
        let nether_wastes = registry
            .by_key(&Key::const_vanilla("nether_wastes"))
            .unwrap();

        assert_eq!(
            nether_wastes.generation_settings().carvers(),
            vec![Key::const_vanilla("nether_cave")]
        );
    }

    #[test]
    fn ids_are_stable_and_cached() {
        let registry = create_test_registry();
        let plains = registry.by_key(&Key::const_vanilla("plains")).unwrap();

        assert_eq!(registry.by_id(plains.id()).unwrap().key(), plains.key());
    }
}
