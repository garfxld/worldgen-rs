use std::cell::Cell;

use glam::IVec3;

use crate::chunk::proto::{GenerationChunk, SurfacePhase};
use crate::util::ident::BlockState;
use crate::util::math::lerp2;
use crate::worldgen::density::{ColumnCache, preliminary_surface_level};
use crate::worldgen::generator::Generator;
use crate::worldgen::generator::fuzzed_biome::FuzzedBiomeColumn;
use crate::worldgen::surface::{
    SurfaceBiomeProvider, SurfaceConditionNoiseCache, SurfaceRuleContext, TemperatureXzCache,
};

#[derive(Clone, Copy)]
pub(super) struct PreliminarySurfaceCorners {
    pub(super) nw: i32,
    pub(super) ne: i32,
    pub(super) sw: i32,
    pub(super) se: i32,
}

impl Generator {
    fn preliminary_surface_corners(
        &self,
        chunk_min_x: i32,
        chunk_min_z: i32,
    ) -> PreliminarySurfaceCorners {
        let noises = &self.noises;
        let mut cache = ColumnCache::new(noises);
        PreliminarySurfaceCorners {
            nw: preliminary_surface_level(noises, &mut cache, chunk_min_x, chunk_min_z),
            ne: preliminary_surface_level(noises, &mut cache, chunk_min_x + 16, chunk_min_z),
            sw: preliminary_surface_level(noises, &mut cache, chunk_min_x, chunk_min_z + 16),
            se: preliminary_surface_level(noises, &mut cache, chunk_min_x + 16, chunk_min_z + 16),
        }
    }

    pub fn build_surface(
        &self,
        chunk: GenerationChunk<'_, SurfacePhase>,
        neighbor_biomes: &dyn Fn(IVec3) -> u16,
    ) {
        let min_y = self.noises.min_y();
        let pos = chunk.pos();
        let chunk_min_x = pos.0.x * 16;
        let chunk_min_z = pos.0.y * 16;
        let default_block_id = self.default_block_id;
        let surface_rules = &self.surface_rules;
        let block_states = surface_rules.block_states();
        let surface_rule_uses_biome = surface_rules.uses_biome();
        let surface_rule_uses_preliminary_surface = surface_rules.uses_preliminary_surface();
        let surface_rule_uses_surface_secondary = surface_rules.uses_surface_secondary();
        let surface_rule_uses_steep = surface_rules.uses_steep();
        let lazy_surface_rule_biome =
            surface_rule_uses_biome && surface_rule_uses_preliminary_surface;
        let surface_needs_min_surface_level = surface_rule_uses_preliminary_surface
            || self.surface_extension_biomes.frozen_ocean.is_some()
            || self.surface_extension_biomes.deep_frozen_ocean.is_some();
        let surface_needs_biomes =
            surface_rule_uses_biome || self.surface_extension_biomes.needs_surface_biome();
        let chunk_quart_x = pos.0.x * 4;
        let chunk_quart_z = pos.0.y * 4;

        chunk.prime_world_surface_heightmap();

        let preliminary_surface_corners = surface_needs_min_surface_level
            .then(|| self.preliminary_surface_corners(chunk_min_x, chunk_min_z));

        let eroded_badlands_id = self
            .surface_extension_biomes
            .eroded_badlands
            .map(|b| b.id() as u16);
        let frozen_ocean_id = self
            .surface_extension_biomes
            .frozen_ocean
            .map(|b| b.id() as u16);
        let deep_frozen_ocean_id = self
            .surface_extension_biomes
            .deep_frozen_ocean
            .map(|b| b.id() as u16);

        let biome_data = surface_needs_biomes.then(|| chunk.read_all_biomes());
        let section_count = chunk.section_count();

        let mut pending_writes: Vec<(usize, BlockState)> = Vec::new();
        let mut column_buf: Vec<BlockState> = Vec::new();
        let condition_noise_values: Vec<Cell<f64>> = surface_rules
            .noise_ids()
            .iter()
            .map(|_| Cell::new(0.0))
            .collect();
        let condition_noise_computed_at: Vec<Cell<u64>> = surface_rules
            .noise_ids()
            .iter()
            .map(|_| Cell::new(0))
            .collect();
        let condition_noise_cache =
            SurfaceConditionNoiseCache::new(&condition_noise_values, &condition_noise_computed_at);

        for local_x in 0..16usize {
            for local_z in 0..16usize {
                let block_x = chunk_min_x + local_x as i32;
                let block_z = chunk_min_z + local_z as i32;

                let mut start_height = chunk.world_surface_height_at(local_x, local_z);

                let mut biome_col = biome_data.as_deref().map(|biome_data| {
                    FuzzedBiomeColumn::new(
                        biome_data,
                        section_count,
                        self.biome_zoom_seed,
                        block_x,
                        block_z,
                        min_y,
                        chunk_quart_x,
                        chunk_quart_z,
                        neighbor_biomes,
                    )
                });

                let surface_biome_id = if self.surface_extension_biomes.needs_surface_biome() {
                    biome_col
                        .as_mut()
                        .map(|biome_col| biome_col.get(start_height))
                } else {
                    None
                };
                if let Some(eroded_badlands_id) = eroded_badlands_id
                    && surface_biome_id == Some(eroded_badlands_id)
                {
                    start_height = self.surface_system.eroded_badlands_extension(
                        chunk,
                        local_x,
                        local_z,
                        block_x,
                        block_z,
                        start_height,
                        min_y,
                    );
                }

                chunk.read_column_into(local_x, local_z, &mut column_buf);

                let surface_depth = self.surface_system.get_surface_depth(block_x, block_z);
                let surface_secondary = if surface_rule_uses_surface_secondary {
                    self.surface_system.get_surface_secondary(block_x, block_z)
                } else {
                    0.0
                };
                condition_noise_cache.reset();

                let min_surface_level = if let Some(corners) = preliminary_surface_corners {
                    let t_x = f64::from(local_x as u8) / 16.0;
                    let t_z = f64::from(local_z as u8) / 16.0;
                    let interp = lerp2(
                        t_x,
                        t_z,
                        f64::from(corners.nw),
                        f64::from(corners.ne),
                        f64::from(corners.sw),
                        f64::from(corners.se),
                    );
                    interp.floor() as i32 + surface_depth - 8
                } else {
                    0
                };

                let steep = surface_rule_uses_steep && {
                    let z_north = local_z.saturating_sub(1);
                    let z_south = (local_z + 1).min(15);
                    let h_north = chunk.world_surface_height_at(local_x, z_north) - 1;
                    let h_south = chunk.world_surface_height_at(local_x, z_south) - 1;
                    if h_south >= h_north + 4 {
                        true
                    } else {
                        let x_west = local_x.saturating_sub(1);
                        let x_east = (local_x + 1).min(15);
                        let h_west = chunk.world_surface_height_at(x_west, local_z) - 1;
                        let h_east = chunk.world_surface_height_at(x_east, local_z) - 1;
                        h_west >= h_east + 4
                    }
                };

                let mut stone_depth_above: i32 = 0;
                let mut water_height: i32 = i32::MIN;
                let mut next_ceiling_stone_y: i32 = i32::MAX;
                pending_writes.clear();
                let mut xz = TemperatureXzCache::new(block_x, block_z);

                for y in (min_y..=start_height).rev() {
                    let relative_y = (y - min_y) as usize;
                    let state = column_buf[relative_y];

                    if state.is_air() {
                        stone_depth_above = 0;
                        water_height = i32::MIN;
                        continue;
                    }

                    if state.get_block().config.liquid {
                        if water_height == i32::MIN {
                            water_height = y + 1;
                        }
                        continue;
                    }

                    if next_ceiling_stone_y >= y {
                        next_ceiling_stone_y = i32::MIN;
                        for la_y in (min_y - 1..y).rev() {
                            if la_y < min_y {
                                next_ceiling_stone_y = la_y + 1;
                                break;
                            }
                            let la_rel = (la_y - min_y) as usize;
                            let la_state = column_buf[la_rel];
                            if la_state.is_air() || la_state.get_block().config.liquid {
                                next_ceiling_stone_y = la_y + 1;
                                break;
                            }
                        }
                    }

                    stone_depth_above += 1;
                    let stone_depth_below = y - next_ceiling_stone_y + 1;

                    if state == default_block_id {
                        let eager_biome_id = if surface_rule_uses_biome && !lazy_surface_rule_biome
                        {
                            biome_col.as_mut().map(|biome_col| biome_col.get(y))
                        } else {
                            None
                        };
                        let biome_provider = if lazy_surface_rule_biome {
                            biome_col
                                .as_mut()
                                .map(|biome_col| biome_col as &mut dyn SurfaceBiomeProvider)
                        } else {
                            None
                        };

                        let mut ctx = SurfaceRuleContext::new(
                            block_x,
                            block_z,
                            surface_depth,
                            surface_secondary,
                            min_surface_level,
                            steep,
                            y,
                            stone_depth_above,
                            stone_depth_below,
                            water_height,
                            eager_biome_id,
                            biome_provider,
                            &self.surface_system,
                            &condition_noise_cache,
                            &mut xz,
                            block_states,
                        );

                        let rule_result = surface_rules.try_apply(&mut ctx);

                        if let Some(new_block) = rule_result {
                            pending_writes.push((relative_y, new_block));
                        }
                    }
                }

                if !pending_writes.is_empty() {
                    chunk.write_column(local_x, local_z, &pending_writes);
                    for &(relative_y, state) in &pending_writes {
                        column_buf[relative_y] = state;
                    }
                }

                if let (Some(frozen_ocean_id), Some(deep_frozen_ocean_id)) =
                    (frozen_ocean_id, deep_frozen_ocean_id)
                    && let Some(surface_biome_id) = surface_biome_id
                        .filter(|id| *id == frozen_ocean_id || *id == deep_frozen_ocean_id)
                {
                    pending_writes.clear();
                    self.surface_system.collect_frozen_ocean_extension_writes(
                        surface_biome_id,
                        block_x,
                        block_z,
                        start_height,
                        min_surface_level,
                        min_y,
                        &column_buf,
                        &mut pending_writes,
                    );
                    if !pending_writes.is_empty() {
                        chunk.write_column(local_x, local_z, &pending_writes);
                    }
                }
            }
        }
    }
}
