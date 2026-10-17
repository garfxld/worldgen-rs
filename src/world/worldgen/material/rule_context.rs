use std::sync::Arc;

use rustc_hash::{FxHashMap, FxHashSet};
use shulkr::util::BlockPosition;
use shulkr::world::block::BlockState;

use crate::registry::biome::{BiomeRef, cold_enough_to_snow};
use crate::util::rng::RandomFactory;
use crate::util::value_providers::VerticalAnchor;
use crate::world::worldgen::densityfunction::context::SamplerContext;
use crate::world::worldgen::densityfunction::eval::{sample_value, sample_volume};
use crate::world::worldgen::densityfunction::function::Df;
use crate::world::worldgen::densityfunction::sampler::S;
use crate::world::worldgen::densityfunction::volume::DensityVolume;
use crate::world::worldgen::random_state::RandomState;
use crate::world::worldgen::synth::Noise;
use crate::world::worldgen::worldgen_context::WorldGenContext;

use super::MaterialSystem;

const HOW_FAR_BELOW_PRELIMINARY_SURFACE_LEVEL_TO_BUILD_SURFACE: i32 = 8;

struct NoiseSampler {
    noise: Arc<Noise>,
    is_3d: bool,
    last_update: i64,
    last_noise: f64,
}

pub struct DensityGetter {
    sampler: S,
    buffer: Option<Vec<f32>>,
}

pub struct MaterialRuleContext<'a, 'b> {
    system: &'a MaterialSystem,
    random_state: &'a RandomState,
    expected_volume: DensityVolume,
    density_samplers: &'a mut SamplerContext<'b>,
    biome_getter: &'a dyn Fn(BlockPosition) -> BiomeRef,
    context: WorldGenContext,
    possible_biomes: Option<&'a FxHashSet<u16>>,
    preliminary_surface_volume: DensityVolume,
    preliminary_surface_buffer: Option<Vec<f32>>,
    noise_sampler_index: FxHashMap<(String, bool), usize>,
    noise_samplers: Vec<NoiseSampler>,
    last_update_xz: i64,
    block_x: i32,
    block_z: i32,
    surface_gradient_x: i32,
    surface_gradient_z: i32,
    surface_depth: i32,
    last_surface_depth2_update: i64,
    surface_secondary: f64,
    last_min_surface_level_update: i64,
    min_surface_level: i32,
    last_update_y: i64,
    biome: Option<BiomeRef>,
    block_y: i32,
    water_height: i32,
    stone_depth_below: i32,
    stone_depth_above: i32,
}

impl<'a, 'b> MaterialRuleContext<'a, 'b> {
    pub fn new(
        system: &'a MaterialSystem,
        random_state: &'a RandomState,
        expected_volume: DensityVolume,
        density_samplers: &'a mut SamplerContext<'b>,
        biome_getter: &'a dyn Fn(BlockPosition) -> BiomeRef,
        context: WorldGenContext,
        possible_biomes: Option<&'a FxHashSet<u16>>,
    ) -> Self {
        let last_update_xz = i64::MIN + 1;
        Self {
            system,
            random_state,
            expected_volume,
            density_samplers,
            biome_getter,
            context,
            possible_biomes,
            preliminary_surface_volume: DensityVolume::new(
                [expected_volume.size_x, 1, expected_volume.size_z],
                [expected_volume.min_block_x, 0, expected_volume.min_block_z],
            ),
            preliminary_surface_buffer: None,
            noise_sampler_index: FxHashMap::default(),
            noise_samplers: Vec::new(),
            last_update_xz,
            block_x: 0,
            block_z: 0,
            surface_gradient_x: 0,
            surface_gradient_z: 0,
            surface_depth: 0,
            last_surface_depth2_update: last_update_xz - 1,
            surface_secondary: 0.0,
            last_min_surface_level_update: last_update_xz - 1,
            min_surface_level: 0,
            last_update_y: i64::MIN + 1,
            biome: None,
            block_y: 0,
            water_height: 0,
            stone_depth_below: 0,
            stone_depth_above: 0,
        }
    }

    pub fn update_xz(
        &mut self,
        block_x: i32,
        block_z: i32,
        surface_gradient_x: i32,
        surface_gradient_z: i32,
    ) {
        self.last_update_xz += 1;
        self.last_update_y += 1;
        self.block_x = block_x;
        self.block_z = block_z;
        self.surface_gradient_x = surface_gradient_x;
        self.surface_gradient_z = surface_gradient_z;
        self.surface_depth = self.system.get_surface_depth(block_x, block_z);
    }

    pub const fn update_y(
        &mut self,
        stone_depth_above: i32,
        stone_depth_below: i32,
        water_height: i32,
        block_y: i32,
    ) {
        self.last_update_y += 1;
        self.biome = None;
        self.block_y = block_y;
        self.water_height = water_height;
        self.stone_depth_below = stone_depth_below;
        self.stone_depth_above = stone_depth_above;
    }

    pub const fn last_update_xz(&self) -> i64 {
        self.last_update_xz
    }

    pub const fn last_update_y(&self) -> i64 {
        self.last_update_y
    }

    pub fn get_surface_secondary(&mut self) -> f64 {
        if self.last_surface_depth2_update != self.last_update_xz {
            self.last_surface_depth2_update = self.last_update_xz;
            self.surface_secondary = self
                .system
                .get_surface_secondary(self.block_x, self.block_z);
        }
        self.surface_secondary
    }

    pub fn get_biome(&mut self) -> BiomeRef {
        if let Some(biome) = self.biome {
            return biome;
        }
        let biome = (self.biome_getter)(self.block_pos());
        self.biome = Some(biome);
        biome
    }

    pub const fn get_sea_level(&self) -> i32 {
        self.system.get_sea_level()
    }

    pub fn cold_enough_to_snow(&mut self) -> bool {
        let biome = self.get_biome();
        cold_enough_to_snow(biome.value(), self.block_pos(), self.get_sea_level())
    }

    pub fn get_min_surface_level(&mut self) -> i32 {
        if self.last_min_surface_level_update != self.last_update_xz {
            self.last_min_surface_level_update = self.last_update_xz;
            let sampler = self
                .random_state
                .get_sampler(self.system.preliminary_surface_function());
            let preliminary_surface_level =
                match self
                    .preliminary_surface_volume
                    .index_of_block(self.block_x, 0, self.block_z)
                {
                    Some(index) => {
                        if self.preliminary_surface_buffer.is_none() {
                            let mut buffer = vec![0.0f32; self.preliminary_surface_volume.size()];
                            sample_volume(
                                &sampler,
                                self.density_samplers,
                                &mut buffer,
                                &self.preliminary_surface_volume,
                            );
                            self.preliminary_surface_buffer = Some(buffer);
                        }
                        self.preliminary_surface_buffer
                            .as_ref()
                            .map_or(0.0, |b| b[index])
                    }
                    None => sample_value(
                        &sampler,
                        self.density_samplers,
                        self.block_x,
                        0,
                        self.block_z,
                    ),
                };
            self.min_surface_level = preliminary_surface_level.floor() as i32 + self.surface_depth
                - HOW_FAR_BELOW_PRELIMINARY_SURFACE_LEVEL_TO_BUILD_SURFACE;
        }
        self.min_surface_level
    }

    pub fn get_noise_sampler(&mut self, noise: &str, is_3d: bool) -> usize {
        if let Some(index) = self.noise_sampler_index.get(&(noise.to_owned(), is_3d)) {
            return *index;
        }
        let index = self.noise_samplers.len();
        let last_update = if is_3d {
            self.last_update_y - 1
        } else {
            self.last_update_xz - 1
        };
        self.noise_samplers.push(NoiseSampler {
            noise: self.random_state.get_or_create_noise(noise),
            is_3d,
            last_update,
            last_noise: 0.0,
        });
        self.noise_sampler_index
            .insert((noise.to_owned(), is_3d), index);
        index
    }

    pub fn sample_noise(&mut self, index: usize) -> f64 {
        let (last_update_xz, last_update_y) = (self.last_update_xz, self.last_update_y);
        let (block_x, block_y, block_z) = (self.block_x, self.block_y, self.block_z);
        let sampler = &mut self.noise_samplers[index];
        let current = if sampler.is_3d {
            last_update_y
        } else {
            last_update_xz
        };
        if sampler.last_update != current {
            sampler.last_noise = if sampler.is_3d {
                f64::from(sampler.noise.get(
                    f64::from(block_x),
                    f64::from(block_y),
                    f64::from(block_z),
                ))
            } else {
                f64::from(
                    sampler
                        .noise
                        .get(f64::from(block_x), 0.0, f64::from(block_z)),
                )
            };
            sampler.last_update = current;
        }
        sampler.last_noise
    }

    pub fn get_or_create_random_factory(&self, name: &str) -> RandomFactory {
        self.random_state.get_or_create_random_factory(name)
    }

    pub fn get_densities_in_chunk(&mut self, function: &Df, prefill: bool) -> DensityGetter {
        let sampler = self.random_state.get_sampler(function);
        let buffer = prefill.then(|| {
            let mut buffer = vec![0.0f32; self.expected_volume.size()];
            sample_volume(
                &sampler,
                self.density_samplers,
                &mut buffer,
                &self.expected_volume,
            );
            buffer
        });
        DensityGetter { sampler, buffer }
    }

    pub fn get_density(&mut self, getter: &DensityGetter) -> f32 {
        if let Some(buffer) = &getter.buffer
            && let Some(index) =
                self.expected_volume
                    .index_of_block(self.block_x, self.block_y, self.block_z)
        {
            return buffer[index];
        }
        sample_value(
            &getter.sampler,
            self.density_samplers,
            self.block_x,
            self.block_y,
            self.block_z,
        )
    }

    pub const fn possible_biomes(&self) -> Option<&FxHashSet<u16>> {
        self.possible_biomes
    }

    pub const fn stone_depth_above(&self) -> i32 {
        self.stone_depth_above
    }

    pub const fn stone_depth_below(&self) -> i32 {
        self.stone_depth_below
    }

    pub const fn surface_depth(&self) -> i32 {
        self.surface_depth
    }

    pub const fn water_height(&self) -> i32 {
        self.water_height
    }

    pub const fn block_x(&self) -> i32 {
        self.block_x
    }

    pub const fn block_y(&self) -> i32 {
        self.block_y
    }

    pub const fn block_z(&self) -> i32 {
        self.block_z
    }

    pub const fn block_pos(&self) -> BlockPosition {
        BlockPosition::new(self.block_x, self.block_y, self.block_z)
    }

    pub const fn surface_gradient_x(&self) -> i32 {
        self.surface_gradient_x
    }

    pub const fn surface_gradient_z(&self) -> i32 {
        self.surface_gradient_z
    }

    pub const fn resolve_anchor_y(&self, anchor: VerticalAnchor) -> i32 {
        anchor.resolve_y(&self.context)
    }

    pub fn get_band(&self, x: i32, y: i32, z: i32) -> BlockState {
        self.system.get_band(x, y, z)
    }
}
