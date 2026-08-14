use std::collections::BTreeMap;

use crate::chunk::proto::{GenerationChunk, SurfacePhase};
use crate::registry::REGISTRY;
use crate::registry::biome::TemperatureModifier;
use crate::registry::blocks::vanilla_blocks;
use crate::util::ident::BlockState;
use crate::util::noise::{NormalNoise, PerlinSimplexNoise};
use crate::util::rng::legacy_random::LegacyRandomSource;
use crate::util::rng::name_hash::NameHash;
use crate::util::rng::{RandomFactory, RandomSource};
use crate::worldgen::backend::ir::NoiseParameters;

const CLAY_BAND_LENGTH: usize = 192;

pub struct TemperatureXzCache {
    block_x: i32,
    block_z: i32,
    frozen_ground_large_variation: f64,
    frozen_ground_edge_variation: f64,
    frozen_ground_small_variation: f64,
    height_adjusted_temperature_noise: f64,
}

impl TemperatureXzCache {
    pub const fn new(block_x: i32, block_z: i32) -> Self {
        Self {
            block_x,
            block_z,
            frozen_ground_large_variation: f64::NAN,
            frozen_ground_edge_variation: f64::NAN,
            frozen_ground_small_variation: f64::NAN,
            height_adjusted_temperature_noise: f64::NAN,
        }
    }
}

pub struct SurfaceSystem {
    surface_noise: NormalNoise,
    surface_secondary_noise: NormalNoise,
    clay_bands_offset_noise: NormalNoise,
    clay_bands: [BlockState; CLAY_BAND_LENGTH],

    noise_random: RandomFactory,

    condition_noises: Vec<NormalNoise>,

    vertical_gradient_randoms: Vec<RandomFactory>,

    badlands_pillar_noise: NormalNoise,
    badlands_pillar_roof_noise: NormalNoise,
    badlands_surface_noise: NormalNoise,
    iceberg_pillar_noise: NormalNoise,
    iceberg_pillar_roof_noise: NormalNoise,
    iceberg_surface_noise: NormalNoise,

    temperature_noise: PerlinSimplexNoise,
    frozen_temperature_noise: PerlinSimplexNoise,
    biome_info_noise: PerlinSimplexNoise,

    pub default_block: BlockState,
    pub sea_level: i32,
}

impl SurfaceSystem {
    pub fn new(
        splitter: &RandomFactory,
        noise_params: &BTreeMap<String, NoiseParameters>,
        condition_noise_ids: &[&str],
        vertical_gradient_ids: &[&str],
        default_block: BlockState,
        sea_level: i32,
    ) -> Self {
        const CLAY_BANDS_HASH: NameHash = NameHash::new("minecraft:clay_bands");

        let noise_random = splitter.clone();
        let mut band_random = noise_random.with_hash_of(&CLAY_BANDS_HASH);
        let clay_bands = Self::generate_bands(&mut band_random);

        let condition_noises: Vec<NormalNoise> = condition_noise_ids
            .iter()
            .map(|&id| create_noise(splitter, id, noise_params))
            .collect();
        let vertical_gradient_randoms: Vec<RandomFactory> = vertical_gradient_ids
            .iter()
            .map(|&id| {
                let hash = NameHash::new(id);
                splitter.with_hash_of(&hash).next_positional()
            })
            .collect();

        Self {
            surface_noise: create_noise(splitter, "minecraft:surface", noise_params),
            surface_secondary_noise: create_noise(
                splitter,
                "minecraft:surface_secondary",
                noise_params,
            ),
            clay_bands_offset_noise: create_noise(
                splitter,
                "minecraft:clay_bands_offset",
                noise_params,
            ),
            clay_bands,
            noise_random,
            condition_noises,
            vertical_gradient_randoms,
            badlands_pillar_noise: create_noise(
                splitter,
                "minecraft:badlands_pillar",
                noise_params,
            ),
            badlands_pillar_roof_noise: create_noise(
                splitter,
                "minecraft:badlands_pillar_roof",
                noise_params,
            ),
            badlands_surface_noise: create_noise(
                splitter,
                "minecraft:badlands_surface",
                noise_params,
            ),
            iceberg_pillar_noise: create_noise(splitter, "minecraft:iceberg_pillar", noise_params),
            iceberg_pillar_roof_noise: create_noise(
                splitter,
                "minecraft:iceberg_pillar_roof",
                noise_params,
            ),
            iceberg_surface_noise: create_noise(
                splitter,
                "minecraft:iceberg_surface",
                noise_params,
            ),
            temperature_noise: {
                let mut rng = RandomSource::Legacy(LegacyRandomSource::from_seed(1234));
                PerlinSimplexNoise::new(&mut rng, &[0])
            },
            frozen_temperature_noise: {
                let mut rng = RandomSource::Legacy(LegacyRandomSource::from_seed(3456));
                PerlinSimplexNoise::new(&mut rng, &[-2, -1, 0])
            },
            biome_info_noise: {
                let mut rng = RandomSource::Legacy(LegacyRandomSource::from_seed(2345));
                PerlinSimplexNoise::new(&mut rng, &[0])
            },
            default_block,
            sea_level,
        }
    }

    pub fn get_surface_depth(&self, x: i32, z: i32) -> i32 {
        let noise_value = self
            .surface_noise
            .get_value(f64::from(x), 0.0, f64::from(z));
        let jitter = self.noise_random.at(x, 0, z).next_f64() * 0.25;
        (noise_value * 2.75 + 3.0 + jitter) as i32
    }

    pub fn get_surface_secondary(&self, x: i32, z: i32) -> f64 {
        self.surface_secondary_noise
            .get_value(f64::from(x), 0.0, f64::from(z))
    }

    #[inline]
    fn frozen_ground_large_variation(&self, xz: &mut TemperatureXzCache) -> f64 {
        if !xz.frozen_ground_large_variation.is_nan() {
            return xz.frozen_ground_large_variation;
        }
        let v = self
            .frozen_temperature_noise
            .get_value(f64::from(xz.block_x) * 0.05, f64::from(xz.block_z) * 0.05)
            * 7.0;
        xz.frozen_ground_large_variation = v;
        v
    }

    #[inline]
    fn frozen_ground_edge_variation(&self, xz: &mut TemperatureXzCache) -> f64 {
        if !xz.frozen_ground_edge_variation.is_nan() {
            return xz.frozen_ground_edge_variation;
        }
        let v = self
            .biome_info_noise
            .get_value(f64::from(xz.block_x) * 0.2, f64::from(xz.block_z) * 0.2);
        xz.frozen_ground_edge_variation = v;
        v
    }

    #[inline]
    fn frozen_ground_small_variation(&self, xz: &mut TemperatureXzCache) -> f64 {
        if !xz.frozen_ground_small_variation.is_nan() {
            return xz.frozen_ground_small_variation;
        }
        let v = self
            .biome_info_noise
            .get_value(f64::from(xz.block_x) * 0.09, f64::from(xz.block_z) * 0.09);
        xz.frozen_ground_small_variation = v;
        v
    }

    #[inline]
    fn height_adjusted_temperature_noise(&self, xz: &mut TemperatureXzCache) -> f64 {
        if !xz.height_adjusted_temperature_noise.is_nan() {
            return xz.height_adjusted_temperature_noise;
        }
        let v = self
            .temperature_noise
            .get_value(f64::from(xz.block_x) / 8.0, f64::from(xz.block_z) / 8.0)
            * 8.0;
        xz.height_adjusted_temperature_noise = v;
        v
    }

    fn get_temperature(&self, biome_id: u16, block_y: i32, xz: &mut TemperatureXzCache) -> f32 {
        let biome = REGISTRY
            .biomes
            .by_id(biome_id as usize)
            .expect("invalid biome id");
        let base_temp = biome.temperature;

        let modified_temp = match biome.temperature_modifier {
            TemperatureModifier::None => base_temp,
            TemperatureModifier::Frozen => {
                let combined = self.frozen_ground_large_variation(xz) + self.frozen_ground_edge_variation(xz);
                if combined < 0.3 {
                    if self.frozen_ground_small_variation(xz) < 0.8 {
                        0.2
                    } else {
                        base_temp
                    }
                } else {
                    base_temp
                }
            }
        };

        let snow_level = self.sea_level + 17;
        if block_y > snow_level {
            let v = self.height_adjusted_temperature_noise(xz) as f32;
            modified_temp - (v + block_y as f32 - snow_level as f32) * 0.05 / 40.0
        } else {
            modified_temp
        }
    }

    pub fn cold_enough_to_snow(
        &self,
        biome_id: u16,
        block_y: i32,
        xz: &mut TemperatureXzCache,
    ) -> bool {
        self.get_temperature(biome_id, block_y, xz) < 0.15
    }

    fn should_melt_frozen_ocean_iceberg_slightly(
        &self,
        biome_id: u16,
        block_x: i32,
        block_z: i32,
    ) -> bool {
        let mut xz = TemperatureXzCache::new(block_x, block_z);
        self.get_temperature(biome_id, self.sea_level, &mut xz) > 0.1
    }

    fn generate_bands(random: &mut RandomSource) -> [BlockState; CLAY_BAND_LENGTH] {
        let terracotta = vanilla_blocks::TERRACOTTA.default_state();
        let orange = vanilla_blocks::ORANGE_TERRACOTTA.default_state();
        let yellow = vanilla_blocks::YELLOW_TERRACOTTA.default_state();
        let brown = vanilla_blocks::BROWN_TERRACOTTA.default_state();
        let red = vanilla_blocks::RED_TERRACOTTA.default_state();
        let white = vanilla_blocks::WHITE_TERRACOTTA.default_state();
        let light_gray = vanilla_blocks::LIGHT_GRAY_TERRACOTTA.default_state();

        let mut bands = [terracotta; CLAY_BAND_LENGTH];

        let mut i = 0usize;
        while i < CLAY_BAND_LENGTH {
            i += random.next_i32_bounded(5) as usize + 1;
            if i < CLAY_BAND_LENGTH {
                bands[i] = orange;
            }
            i += 1;
        }

        Self::make_bands(random, &mut bands, 1, yellow);
        Self::make_bands(random, &mut bands, 2, brown);
        Self::make_bands(random, &mut bands, 1, red);

        let white_count = random.next_i32_between(9, 15);
        let mut placed = 0;
        let mut start = 0usize;
        while placed < white_count && start < CLAY_BAND_LENGTH {
            bands[start] = white;
            if start > 1 && random.next_bool() {
                bands[start - 1] = light_gray;
            }
            if start + 1 < CLAY_BAND_LENGTH && random.next_bool() {
                bands[start + 1] = light_gray;
            }
            placed += 1;
            start += random.next_i32_bounded(16) as usize + 4;
        }

        bands
    }

    fn make_bands(
        random: &mut RandomSource,
        bands: &mut [BlockState; CLAY_BAND_LENGTH],
        base_width: i32,
        state: BlockState,
    ) {
        let band_count = random.next_i32_between(6, 15);
        for _ in 0..band_count {
            let width = (base_width + random.next_i32_bounded(3)) as usize;
            let start = random.next_i32_bounded(CLAY_BAND_LENGTH as i32) as usize;
            for p in 0..width {
                if start + p >= CLAY_BAND_LENGTH {
                    break;
                }
                bands[start + p] = state;
            }
        }
    }

    pub fn condition_noise(&self, noise_index: usize, x: i32, z: i32) -> f64 {
        self.condition_noises[noise_index].get_value(f64::from(x), 0.0, f64::from(z))
    }

    pub fn condition_noise_3d(&self, noise_index: usize, x: i32, y: i32, z: i32) -> f64 {
        self.condition_noises[noise_index].get_value(f64::from(x), f64::from(y), f64::from(z))
    }

    pub fn get_band(&self, x: i32, y: i32, z: i32) -> BlockState {
        let offset = (self
            .clay_bands_offset_noise
            .get_value(f64::from(x), 0.0, f64::from(z))
            * 4.0
            + 0.5)
            .floor() as i32;
        let index = ((y + offset) % CLAY_BAND_LENGTH as i32 + CLAY_BAND_LENGTH as i32) as usize
            % CLAY_BAND_LENGTH;
        self.clay_bands[index]
    }

    pub fn vertical_gradient(
        &self,
        gradient_index: usize,
        block_x: i32,
        block_y: i32,
        block_z: i32,
        true_at_and_below: i32,
        false_at_and_above: i32,
    ) -> bool {
        if block_y <= true_at_and_below {
            return true;
        }
        if block_y >= false_at_and_above {
            return false;
        }
        let probability = f64::from(false_at_and_above - block_y)
            / f64::from(false_at_and_above - true_at_and_below);

        let factory = &self.vertical_gradient_randoms[gradient_index];
        let random_value = f64::from(factory.at(block_x, block_y, block_z).next_f32());
        random_value < probability
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "matches vanilla SurfaceSystem.erodedBadlandsExtension signature"
    )]
    pub fn eroded_badlands_extension(
        &self,
        chunk: GenerationChunk<'_, SurfacePhase>,
        local_x: usize,
        local_z: usize,
        block_x: i32,
        block_z: i32,
        height: i32,
        min_y: i32,
    ) -> i32 {
        let pillar_buffer = f64::min(
            (self
                .badlands_surface_noise
                .get_value(f64::from(block_x), 0.0, f64::from(block_z))
                * 8.25)
                .abs(),
            self.badlands_pillar_noise.get_value(
                f64::from(block_x) * 0.2,
                0.0,
                f64::from(block_z) * 0.2,
            ) * 15.0,
        );

        if pillar_buffer <= 0.0 {
            return height;
        }

        let pillar_floor = (self.badlands_pillar_roof_noise.get_value(
            f64::from(block_x) * 0.75,
            0.0,
            f64::from(block_z) * 0.75,
        ) * 1.5)
            .abs();

        let extension_top = 64.0
            + f64::min(
                pillar_buffer * pillar_buffer * 2.5,
                (pillar_floor * 50.0).ceil() + 24.0,
            );
        let start_y = extension_top.floor() as i32;

        if height > start_y {
            return height;
        }

        for y in (min_y..=start_y).rev() {
            let rel_y = (y - min_y) as usize;
            let state = chunk
                .get_relative_block(local_x, rel_y, local_z)
                .unwrap_or(BlockState(0));
            if state == self.default_block {
                break;
            }
            if state.get_block().config.liquid {
                return height;
            }
        }

        for y in (min_y..=start_y).rev() {
            let rel_y = (y - min_y) as usize;
            let state = chunk
                .get_relative_block(local_x, rel_y, local_z)
                .unwrap_or(BlockState(0));
            if !state.is_air() {
                break;
            }
            chunk.set_relative_block(local_x, rel_y, local_z, self.default_block);
        }

        start_y + 1
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "keeps the vanilla frozenOceanExtension inputs explicit"
    )]
    pub fn collect_frozen_ocean_extension_writes(
        &self,
        biome_id: u16,
        block_x: i32,
        block_z: i32,
        height: i32,
        min_surface_level: i32,
        min_y: i32,
        column: &[BlockState],
        writes: &mut Vec<(usize, BlockState)>,
    ) {
        let iceberg = f64::min(
            (self
                .iceberg_surface_noise
                .get_value(f64::from(block_x), 0.0, f64::from(block_z))
                * 8.25)
                .abs(),
            self.iceberg_pillar_noise.get_value(
                f64::from(block_x) * 1.28,
                0.0,
                f64::from(block_z) * 1.28,
            ) * 15.0,
        );

        if iceberg <= 1.8 {
            return;
        }

        let iceberg_roof = (self.iceberg_pillar_roof_noise.get_value(
            f64::from(block_x) * 1.17,
            0.0,
            f64::from(block_z) * 1.17,
        ) * 1.5)
            .abs();

        let mut top = f64::min(iceberg * iceberg * 1.2, (iceberg_roof * 40.0).ceil() + 14.0);

        if self.should_melt_frozen_ocean_iceberg_slightly(biome_id, block_x, block_z) {
            top -= 2.0;
        }

        let extension_bottom;
        if top > 2.0 {
            extension_bottom = f64::from(self.sea_level) - top - 7.0;
            top += f64::from(self.sea_level);
        } else {
            top = 0.0;
            extension_bottom = 0.0;
        }

        let extension_top = top;
        let mut random = self.noise_random.at(block_x, 0, block_z);
        let max_snow_depth = 2 + random.next_i32_bounded(4);
        let min_snow_height = self.sea_level + 18 + random.next_i32_bounded(10);
        let mut snow_depth = 0;

        let snow_block = vanilla_blocks::SNOW_BLOCK.default_state();
        let packed_ice = vanilla_blocks::PACKED_ICE.default_state();
        let air = vanilla_blocks::AIR.default_state();

        let start_y = i32::max(height, top as i32 + 1);
        for y in (min_surface_level..=start_y).rev() {
            let rel_y = (y - min_y) as usize;
            let state = column.get(rel_y).copied().unwrap_or(air);

            let is_air = state.is_air();
            let is_water = state.get_block() == &vanilla_blocks::WATER;

            if (is_air && y < extension_top as i32 && random.next_f64() > 0.01)
                || (is_water
                    && y > extension_bottom as i32
                    && y < self.sea_level
                    && extension_bottom != 0.0
                    && random.next_f64() > 0.15)
            {
                if snow_depth <= max_snow_depth && y > min_snow_height {
                    writes.push((rel_y, snow_block));
                    snow_depth += 1;
                } else {
                    writes.push((rel_y, packed_ice));
                }
            }
        }
    }
}

fn create_noise(
    splitter: &RandomFactory,
    id: &str,
    params: &BTreeMap<String, NoiseParameters>,
) -> NormalNoise {
    let p = params
        .get(id)
        .unwrap_or_else(|| panic!("Missing noise parameters for {id}"));
    NormalNoise::create(splitter, id, p.first_octave, &p.amplitudes)
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test loads the checked-in vanilla datapack"
)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Once;

    use super::*;
    use crate::registry::Registries;
    use crate::util::ident::Identifier;
    use crate::util::rng::RandomSource as RootRandomSource;
    use crate::util::rng::xoroshiro::XoroshiroRandomSource;
    use crate::worldgen::backend::ir::loader::DatapackSource;

    fn datapack_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/datapack/worldgen")
    }

    fn init_global_registry() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let registries = Registries::new_vanilla().expect("vanilla datapack should load");
            let _ = REGISTRY.init(registries);
        });
    }

    fn build_system(seed: u64) -> SurfaceSystem {
        let source = DatapackSource::new(datapack_root());
        let params = source.noise_parameters().unwrap();
        let mut root = RootRandomSource::Xoroshiro(XoroshiroRandomSource::from_seed(seed));
        let splitter = root.next_positional();
        let default_block = REGISTRY.blocks.get_default_state_id(&vanilla_blocks::STONE);
        SurfaceSystem::new(&splitter, &params, &[], &[], default_block, 63)
    }

    #[test]
    fn surface_depth_and_secondary_are_finite_and_plausible() {
        init_global_registry();
        let system = build_system(42);

        for &(x, z) in &[(0, 0), (8, 8), (100, -50), (-15, 15)] {
            let depth = system.get_surface_depth(x, z);
            let secondary = system.get_surface_secondary(x, z);
            assert!(secondary.is_finite());
            assert!(
                (0..=8).contains(&depth),
                "surface depth {depth} out of vanilla's plausible range at ({x}, {z})"
            );
        }
    }

    #[test]
    fn get_band_is_deterministic_and_matches_across_seeds() {
        init_global_registry();
        let a = build_system(42);
        let b = build_system(42);

        for &(x, y, z) in &[(0, 64, 0), (100, -20, 50), (8, 200, 8)] {
            assert_eq!(
                a.get_band(x, y, z),
                a.get_band(x, y, z),
                "get_band should be deterministic"
            );
            assert_eq!(
                a.get_band(x, y, z),
                b.get_band(x, y, z),
                "same seed should produce the same clay bands"
            );
        }

        let c = build_system(7);
        let differs = [(0, 64, 0), (100, -20, 50), (8, 200, 8)]
            .iter()
            .any(|&(x, y, z)| a.get_band(x, y, z) != c.get_band(x, y, z));
        assert!(
            differs,
            "different seeds should (almost certainly) produce different clay bands somewhere"
        );
    }

    #[test]
    fn cold_enough_to_snow_reflects_biome_temperature() {
        init_global_registry();
        let system = build_system(42);

        let plains = REGISTRY
            .biomes
            .by_key(&Identifier::const_vanilla("plains"))
            .unwrap();
        let snowy_plains = REGISTRY
            .biomes
            .by_key(&Identifier::const_vanilla("snowy_plains"))
            .unwrap();

        let mut xz_plains = TemperatureXzCache::new(0, 0);
        let mut xz_snowy = TemperatureXzCache::new(0, 0);
        assert!(!system.cold_enough_to_snow(plains.id() as u16, 64, &mut xz_plains));
        assert!(system.cold_enough_to_snow(snowy_plains.id() as u16, 64, &mut xz_snowy));
    }

    #[test]
    fn xz_cache_reuse_matches_a_fresh_cache() {
        init_global_registry();
        let system = build_system(42);
        let plains = REGISTRY
            .biomes
            .by_key(&Identifier::const_vanilla("plains"))
            .unwrap();

        let mut shared = TemperatureXzCache::new(10, -10);
        let first = system.cold_enough_to_snow(plains.id() as u16, 64, &mut shared);
        let second = system.cold_enough_to_snow(plains.id() as u16, 300, &mut shared);

        let mut fresh_low = TemperatureXzCache::new(10, -10);
        let mut fresh_high = TemperatureXzCache::new(10, -10);
        assert_eq!(
            first,
            system.cold_enough_to_snow(plains.id() as u16, 64, &mut fresh_low)
        );
        assert_eq!(
            second,
            system.cold_enough_to_snow(plains.id() as u16, 300, &mut fresh_high)
        );
    }
}
