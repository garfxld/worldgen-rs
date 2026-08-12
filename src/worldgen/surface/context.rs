use std::cell::Cell;

use crate::util::ident::BlockStateId;
use crate::worldgen::surface::system::{SurfaceSystem, TemperatureXzCache};

pub struct SurfaceRuleContext<'a> {
    pub block_x: i32,
    pub block_z: i32,
    pub surface_depth: i32,
    pub surface_secondary: f64,
    pub min_surface_level: i32,
    pub steep: bool,
    pub block_y: i32,
    pub stone_depth_above: i32,
    pub stone_depth_below: i32,
    pub water_height: i32,
    biome_id: Option<u16>,
    biome_provider: Option<&'a mut dyn SurfaceBiomeProvider>,
    pub system: &'a SurfaceSystem,
    condition_noises: &'a SurfaceConditionNoiseCache<'a>,
    xz: &'a mut TemperatureXzCache,
    block_states: &'a [BlockStateId],
}

impl<'a> SurfaceRuleContext<'a> {
    #[expect(
        clippy::too_many_arguments,
        reason = "surface rule context mirrors vanilla's flat condition input"
    )]
    pub fn new(
        block_x: i32,
        block_z: i32,
        surface_depth: i32,
        surface_secondary: f64,
        min_surface_level: i32,
        steep: bool,
        block_y: i32,
        stone_depth_above: i32,
        stone_depth_below: i32,
        water_height: i32,
        biome_id: Option<u16>,
        biome_provider: Option<&'a mut dyn SurfaceBiomeProvider>,
        system: &'a SurfaceSystem,
        condition_noises: &'a SurfaceConditionNoiseCache<'a>,
        xz: &'a mut TemperatureXzCache,
        block_states: &'a [BlockStateId],
    ) -> Self {
        Self {
            block_x,
            block_z,
            surface_depth,
            surface_secondary,
            min_surface_level,
            steep,
            block_y,
            stone_depth_above,
            stone_depth_below,
            water_height,
            biome_id,
            biome_provider,
            system,
            condition_noises,
            xz,
            block_states,
        }
    }

    #[must_use]
    pub const fn block_state(&self, index: usize) -> BlockStateId {
        self.block_states[index]
    }

    #[must_use]
    pub fn condition_noise(&self, noise_index: usize) -> f64 {
        self.condition_noises
            .get(noise_index, self.system, self.block_x, self.block_z)
    }

    #[must_use]
    pub fn condition_noise_3d(&self, noise_index: usize) -> f64 {
        self.system
            .condition_noise_3d(noise_index, self.block_x, self.block_y, self.block_z)
    }

    #[must_use]
    pub const fn known_biome_id(&self) -> Option<u16> {
        self.biome_id
    }

    #[must_use]
    pub fn biome_id(&mut self) -> Option<u16> {
        if self.biome_id.is_some() {
            return self.biome_id;
        }

        let provider = self.biome_provider.as_mut()?;
        let biome_id = provider.biome_id(self.block_y);
        self.biome_id = Some(biome_id);
        Some(biome_id)
    }

    #[must_use]
    pub fn cold_enough_to_snow(&mut self) -> bool {
        let Some(biome_id) = self.biome_id() else {
            return false;
        };
        self.system
            .cold_enough_to_snow(biome_id, self.block_y, self.xz)
    }
}

pub trait SurfaceBiomeProvider {
    fn biome_id(&mut self, block_y: i32) -> u16;
}

pub struct SurfaceConditionNoiseCache<'a> {
    values: &'a [Cell<f64>],
    initialized: &'a [Cell<bool>],
}

impl<'a> SurfaceConditionNoiseCache<'a> {
    #[must_use]
    pub fn new(values: &'a [Cell<f64>], initialized: &'a [Cell<bool>]) -> Self {
        debug_assert_eq!(values.len(), initialized.len());
        Self {
            values,
            initialized,
        }
    }

    pub fn reset(&self) {
        for initialized in self.initialized {
            initialized.set(false);
        }
    }

    #[must_use]
    pub fn get(&self, noise_index: usize, system: &SurfaceSystem, x: i32, z: i32) -> f64 {
        if self.initialized[noise_index].get() {
            return self.values[noise_index].get();
        }

        let value = system.condition_noise(noise_index, x, z);
        self.values[noise_index].set(value);
        self.initialized[noise_index].set(true);
        value
    }
}
