use std::cell::Cell;

use crate::util::ident::BlockState;
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
    block_states: &'a [BlockState],
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
        block_states: &'a [BlockState],
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

    pub const fn block_state(&self, index: usize) -> BlockState {
        self.block_states[index]
    }

    pub fn condition_noise(&self, noise_index: usize) -> f64 {
        self.condition_noises
            .get(noise_index, self.system, self.block_x, self.block_z)
    }

    pub fn condition_noise_3d(&self, noise_index: usize) -> f64 {
        self.system
            .condition_noise_3d(noise_index, self.block_x, self.block_y, self.block_z)
    }

    pub const fn known_biome_id(&self) -> Option<u16> {
        self.biome_id
    }

    pub fn biome_id(&mut self) -> Option<u16> {
        if self.biome_id.is_some() {
            return self.biome_id;
        }

        let provider = self.biome_provider.as_mut()?;
        let biome_id = provider.biome_id(self.block_y);
        self.biome_id = Some(biome_id);
        Some(biome_id)
    }

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
    computed_at_update: &'a [Cell<u64>],
    last_update: Cell<u64>,
}

impl<'a> SurfaceConditionNoiseCache<'a> {
    pub fn new(values: &'a [Cell<f64>], computed_at_update: &'a [Cell<u64>]) -> Self {
        debug_assert_eq!(values.len(), computed_at_update.len());
        Self {
            values,
            computed_at_update,
            last_update: Cell::new(0),
        }
    }

    pub fn reset(&self) {
        self.last_update.set(self.last_update.get() + 1);
    }

    pub fn get(&self, noise_index: usize, system: &SurfaceSystem, x: i32, z: i32) -> f64 {
        let current_update = self.last_update.get();
        if self.computed_at_update[noise_index].get() == current_update {
            return self.values[noise_index].get();
        }

        let value = system.condition_noise(noise_index, x, z);
        self.values[noise_index].set(value);
        self.computed_at_update[noise_index].set(current_update);
        value
    }
}
