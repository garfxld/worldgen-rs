use std::simd::i32x4;

use rustc_hash::FxHashMap;

use crate::registry::REGISTRY;
use crate::registry::blocks::vanilla_blocks;
use crate::util::ident::BlockState;
use crate::util::math::{clamp, map, map_clamped};
use crate::util::rng::RandomFactory;
use crate::worldgen::backend::jit::RouterEntry;
use crate::worldgen::density::{ColumnCache, Noises};

const Y_SPACING: i32 = 12;

const X_RANGE: i32 = 10;
const Y_RANGE: i32 = 9;
const Z_RANGE: i32 = 10;

const SAMPLE_OFFSET_X: i32 = -5;
const SAMPLE_OFFSET_Y: i32 = 1;
const SAMPLE_OFFSET_Z: i32 = -5;

const LAVA_LEVEL: i32 = -54;
const WAY_BELOW_MIN_Y: i32 = -32512;
const FLOWING_UPDATE_SIMILARITY: f64 = 1.0 - ((12 * 12 - 10 * 10) as f64) / 25.0;

const SURFACE_SAMPLING_OFFSETS: [[i32; 2]; 13] = [
    [0, 0],
    [-2, -1],
    [-1, -1],
    [0, -1],
    [1, -1],
    [-3, 0],
    [-2, 0],
    [-1, 0],
    [1, 0],
    [-2, 1],
    [-1, 1],
    [0, 1],
    [1, 1],
];

#[derive(Clone, Copy, PartialEq, Eq)]
struct FluidStatus {
    fluid_level: i32,
    fluid_type: BlockState,
}

impl FluidStatus {
    const fn at(self, block_y: i32) -> Option<BlockState> {
        if block_y < self.fluid_level {
            Some(self.fluid_type)
        } else {
            None
        }
    }
}

pub enum AquiferResult {
    Solid,
    Air,
    Fluid(BlockState),
}

struct NeighborCellCache {
    world_x: i32,
    world_z: i32,
    y_anchor: i32,
    cell_loc_y: [i32; 16],
    cell_xz_dist_sq: [i32; 16],
    cell_idx: [u32; 12],
}

impl Default for NeighborCellCache {
    fn default() -> Self {
        Self {
            world_x: 0,
            world_z: 0,
            y_anchor: i32::MIN,
            cell_loc_y: [0; 16],
            cell_xz_dist_sq: [0; 16],
            cell_idx: [0; 12],
        }
    }
}

pub struct Aquifer {
    location_cache: Vec<i64>,
    status_cache: Vec<Option<FluidStatus>>,
    aquifer_random: RandomFactory,
    cache: ColumnCache,
    min_grid_x: i32,
    min_grid_y: i32,
    min_grid_z: i32,
    grid_size_x: i32,
    grid_size_z: i32,
    skip_sampling_above_y: i32,
    aquifers_enabled: bool,
    sea_level: i32,
    lava_floor: i32,
    water_id: BlockState,
    lava_id: BlockState,
    default_fluid_id: BlockState,
    should_schedule_fluid_update: bool,
    neighbor_cache: NeighborCellCache,
    prelim_cache: FxHashMap<(i32, i32), i32>,
}

#[inline]
const fn grid_x(block: i32) -> i32 {
    block >> 4
}
#[inline]
const fn grid_z(block: i32) -> i32 {
    block >> 4
}
#[inline]
const fn grid_y(block: i32) -> i32 {
    block.div_euclid(Y_SPACING)
}
#[inline]
const fn from_grid_x(grid: i32, offset: i32) -> i32 {
    (grid << 4) + offset
}
#[inline]
const fn from_grid_y(grid: i32, offset: i32) -> i32 {
    grid * Y_SPACING + offset
}
#[inline]
const fn from_grid_z(grid: i32, offset: i32) -> i32 {
    (grid << 4) + offset
}

const PACKED_X_MASK: i64 = 0x3FF_FFFF;
const PACKED_Y_MASK: i64 = 0xFFF;
const PACKED_Z_MASK: i64 = 0x3FF_FFFF;
const X_OFFSET: i32 = 38;
const Z_OFFSET: i32 = 12;

#[inline]
fn pack_pos(x: i32, y: i32, z: i32) -> i64 {
    ((i64::from(x) & PACKED_X_MASK) << X_OFFSET)
        | (i64::from(y) & PACKED_Y_MASK)
        | ((i64::from(z) & PACKED_Z_MASK) << Z_OFFSET)
}

#[inline]
const fn unpack_x(packed: i64) -> i32 {
    (packed >> X_OFFSET) as i32
}
#[inline]
const fn unpack_y(packed: i64) -> i32 {
    ((packed << 52) >> 52) as i32
}
#[inline]
const fn unpack_z(packed: i64) -> i32 {
    ((packed << 26) >> X_OFFSET) as i32
}

#[inline]
fn router(entry: Option<RouterEntry>, name: &str) -> RouterEntry {
    entry.unwrap_or_else(|| panic!("no router entry named {name}"))
}

#[inline]
fn similarity(dist_sq1: i32, dist_sq2: i32) -> f64 {
    1.0 - f64::from(dist_sq2 - dist_sq1) / 25.0
}

fn is_deep_dark_region(noises: &Noises, cache: &mut ColumnCache, x: i32, y: i32, z: i32) -> bool {
    cache.ensure(x, z, noises);
    let erosion = cache.router_at(router(noises.compiled().routers().erosion, "erosion"), x, y, z, noises);
    let depth = cache.router_at(router(noises.compiled().routers().depth, "depth"), x, y, z, noises);
    erosion < -0.225 && depth > 0.9
}

const fn global_fluid(
    y: i32,
    lava_floor: i32,
    sea_level: i32,
    lava_id: BlockState,
    default_fluid_id: BlockState,
) -> FluidStatus {
    if y < lava_floor {
        FluidStatus {
            fluid_level: LAVA_LEVEL,
            fluid_type: lava_id,
        }
    } else {
        FluidStatus {
            fluid_level: sea_level,
            fluid_type: default_fluid_id,
        }
    }
}

impl Aquifer {
    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors vanilla's Aquifer.create shape"
    )]
    pub fn new(
        chunk_min_x: i32,
        chunk_min_z: i32,
        min_block_y: i32,
        y_block_size: i32,
        aquifer_random: &RandomFactory,
        aquifers_enabled: bool,
        default_fluid_id: BlockState,
        noises: &Noises,
        cache: ColumnCache,
    ) -> Self {
        Self::new_sized(
            chunk_min_x,
            chunk_min_z,
            16,
            16,
            min_block_y,
            y_block_size,
            aquifer_random,
            aquifers_enabled,
            default_fluid_id,
            noises,
            cache,
        )
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors vanilla's Aquifer constructor shape"
    )]
    pub fn new_sized(
        chunk_min_x: i32,
        chunk_min_z: i32,
        width_x: i32,
        width_z: i32,
        min_block_y: i32,
        y_block_size: i32,
        aquifer_random: &RandomFactory,
        aquifers_enabled: bool,
        default_fluid_id: BlockState,
        noises: &Noises,
        mut cache: ColumnCache,
    ) -> Self {
        let sea_level = noises.sea_level();
        let water_id = REGISTRY.blocks.get_default_state_id(&vanilla_blocks::WATER);
        let lava_id = REGISTRY.blocks.get_default_state_id(&vanilla_blocks::LAVA);

        if !aquifers_enabled {
            return Self {
                location_cache: Vec::new(),
                status_cache: Vec::new(),
                aquifer_random: aquifer_random.clone(),
                cache,
                neighbor_cache: NeighborCellCache::default(),
                min_grid_x: 0,
                min_grid_y: 0,
                min_grid_z: 0,
                grid_size_x: 0,
                grid_size_z: 0,
                skip_sampling_above_y: 0,
                aquifers_enabled,
                sea_level,
                lava_floor: LAVA_LEVEL.min(sea_level),
                water_id,
                lava_id,
                default_fluid_id,
                should_schedule_fluid_update: false,
                prelim_cache: FxHashMap::default(),
            };
        }

        let chunk_max_x = chunk_min_x + width_x - 1;
        let chunk_max_z = chunk_min_z + width_z - 1;

        let min_grid_x = grid_x(chunk_min_x + SAMPLE_OFFSET_X);
        let max_grid_x = grid_x(chunk_max_x + SAMPLE_OFFSET_X) + 1;
        let grid_size_x = max_grid_x - min_grid_x + 1;

        let min_grid_y = grid_y(min_block_y + SAMPLE_OFFSET_Y) - 1;
        let max_grid_y = grid_y(min_block_y + y_block_size + SAMPLE_OFFSET_Y) + 1;
        let grid_size_y = max_grid_y - min_grid_y + 1;

        let min_grid_z = grid_z(chunk_min_z + SAMPLE_OFFSET_Z);
        let max_grid_z = grid_z(chunk_max_z + SAMPLE_OFFSET_Z) + 1;
        let grid_size_z = max_grid_z - min_grid_z + 1;

        let total = (grid_size_x * grid_size_y * grid_size_z) as usize;
        let location_cache = vec![i64::MAX; total];
        let status_cache = vec![None; total];

        let mut prelim_cache = FxHashMap::default();
        let max_surface = Self::max_preliminary_surface_level(
            noises,
            &mut cache,
            &mut prelim_cache,
            from_grid_x(min_grid_x, 0),
            from_grid_z(min_grid_z, 0),
            from_grid_x(max_grid_x, X_RANGE - 1),
            from_grid_z(max_grid_z, Z_RANGE - 1),
        );
        let adjusted = max_surface + 8;
        let skip_grid_y = grid_y(adjusted + 12) + 1;
        let skip_sampling_above_y = from_grid_y(skip_grid_y, Y_RANGE + 2) - 1;

        Self {
            location_cache,
            status_cache,
            aquifer_random: aquifer_random.clone(),
            cache,
            neighbor_cache: NeighborCellCache::default(),
            min_grid_x,
            min_grid_y,
            min_grid_z,
            grid_size_x,
            grid_size_z,
            skip_sampling_above_y,
            aquifers_enabled,
            sea_level,
            lava_floor: LAVA_LEVEL.min(sea_level),
            water_id,
            lava_id,
            default_fluid_id,
            should_schedule_fluid_update: false,
            prelim_cache,
        }
    }

    fn max_preliminary_surface_level(
        noises: &Noises,
        cache: &mut ColumnCache,
        prelim_cache: &mut FxHashMap<(i32, i32), i32>,
        min_x: i32,
        min_z: i32,
        max_x: i32,
        max_z: i32,
    ) -> i32 {
        let mut max_level = i32::MIN;
        let mut z = min_z;
        while z <= max_z {
            let mut x = min_x;
            while x <= max_x {
                let level = cached_preliminary_surface_level(noises, cache, prelim_cache, x, z);
                if level > max_level {
                    max_level = level;
                }
                x += 4;
            }
            z += 4;
        }
        max_level
    }

    #[inline]
    const fn get_index(&self, gx: i32, gy: i32, gz: i32) -> usize {
        let x = gx - self.min_grid_x;
        let y = gy - self.min_grid_y;
        let z = gz - self.min_grid_z;
        ((y * self.grid_size_z + z) * self.grid_size_x + x) as usize
    }

    fn refill_neighbor_cache(&mut self, world_x: i32, world_y: i32, world_z: i32) -> i32 {
        let x_anchor = grid_x(world_x + SAMPLE_OFFSET_X);
        let y_anchor = grid_y(world_y + SAMPLE_OFFSET_Y);
        let z_anchor = grid_z(world_z + SAMPLE_OFFSET_Z);

        let mut i = 0;
        for x1 in 0..=1i32 {
            for y1 in -1..=1i32 {
                for z1 in 0..=1i32 {
                    let gx = x_anchor + x1;
                    let gy = y_anchor + y1;
                    let gz = z_anchor + z1;
                    let idx = self.get_index(gx, gy, gz);

                    let loc = self.location_cache[idx];
                    let loc = if loc == i64::MAX {
                        let mut rng = self.aquifer_random.at(gx, gy, gz);
                        let packed = pack_pos(
                            from_grid_x(gx, rng.next_i32_bounded(X_RANGE)),
                            from_grid_y(gy, rng.next_i32_bounded(Y_RANGE)),
                            from_grid_z(gz, rng.next_i32_bounded(Z_RANGE)),
                        );
                        self.location_cache[idx] = packed;
                        packed
                    } else {
                        loc
                    };

                    let dx = unpack_x(loc) - world_x;
                    let dz = unpack_z(loc) - world_z;
                    self.neighbor_cache.cell_loc_y[i] = unpack_y(loc);
                    self.neighbor_cache.cell_xz_dist_sq[i] = dx * dx + dz * dz;
                    self.neighbor_cache.cell_idx[i] = idx as u32;
                    i += 1;
                }
            }
        }

        self.neighbor_cache.world_x = world_x;
        self.neighbor_cache.world_z = world_z;
        self.neighbor_cache.y_anchor = y_anchor;
        y_anchor
    }

    pub fn compute_substance(
        &mut self,
        noises: &Noises,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        density: f64,
    ) -> AquiferResult {
        if density > 0.0 {
            self.should_schedule_fluid_update = false;
            return AquiferResult::Solid;
        }

        if !self.aquifers_enabled {
            self.should_schedule_fluid_update = false;
            let gf = global_fluid(
                world_y,
                self.lava_floor,
                self.sea_level,
                self.lava_id,
                self.default_fluid_id,
            );
            return match gf.at(world_y) {
                Some(id) => AquiferResult::Fluid(id),
                None => AquiferResult::Air,
            };
        }

        let gf = global_fluid(
            world_y,
            self.lava_floor,
            self.sea_level,
            self.lava_id,
            self.default_fluid_id,
        );

        if world_y > self.skip_sampling_above_y {
            self.should_schedule_fluid_update = false;
            return match gf.at(world_y) {
                Some(id) => AquiferResult::Fluid(id),
                None => AquiferResult::Air,
            };
        }

        if gf.fluid_type == self.lava_id && world_y < gf.fluid_level {
            self.should_schedule_fluid_update = false;
            return AquiferResult::Fluid(self.lava_id);
        }

        let y_anchor = grid_y(world_y + SAMPLE_OFFSET_Y);
        if self.neighbor_cache.world_x != world_x
            || self.neighbor_cache.world_z != world_z
            || self.neighbor_cache.y_anchor != y_anchor
        {
            self.refill_neighbor_cache(world_x, world_y, world_z);
        }

        let world_y_v = i32x4::splat(world_y);
        let mut dists = [0i32; 12];
        for batch in 0..3 {
            let base = batch * 4;
            let loc_y_v = i32x4::from_slice(&self.neighbor_cache.cell_loc_y[base..base + 4]);
            let xz_v = i32x4::from_slice(&self.neighbor_cache.cell_xz_dist_sq[base..base + 4]);
            let dy = loc_y_v - world_y_v;
            let dist_v = xz_v + dy * dy;
            dists[base..base + 4].copy_from_slice(&dist_v.to_array());
        }

        let mut dist_sq = [i32::MAX; 4];
        let mut closest_idx = [0usize; 4];

        for (i, &new_dist) in dists.iter().enumerate() {
            let index = self.neighbor_cache.cell_idx[i] as usize;

            if dist_sq[0] >= new_dist {
                dist_sq[3] = dist_sq[2];
                closest_idx[3] = closest_idx[2];
                dist_sq[2] = dist_sq[1];
                closest_idx[2] = closest_idx[1];
                dist_sq[1] = dist_sq[0];
                closest_idx[1] = closest_idx[0];
                dist_sq[0] = new_dist;
                closest_idx[0] = index;
            } else if dist_sq[1] >= new_dist {
                dist_sq[3] = dist_sq[2];
                closest_idx[3] = closest_idx[2];
                dist_sq[2] = dist_sq[1];
                closest_idx[2] = closest_idx[1];
                dist_sq[1] = new_dist;
                closest_idx[1] = index;
            } else if dist_sq[2] >= new_dist {
                dist_sq[3] = dist_sq[2];
                closest_idx[3] = closest_idx[2];
                dist_sq[2] = new_dist;
                closest_idx[2] = index;
            } else if dist_sq[3] >= new_dist {
                dist_sq[3] = new_dist;
                closest_idx[3] = index;
            }
        }

        let status1 = self.get_aquifer_status(closest_idx[0], noises);
        let fluid_at = status1.at(world_y);

        let dist12_delta = dist_sq[1] - dist_sq[0];
        if dist12_delta >= 25 {
            if dist12_delta <= 12 * 12 - 10 * 10 {
                let status2 = self.get_aquifer_status(closest_idx[1], noises);
                self.should_schedule_fluid_update = status1 != status2;
            } else {
                self.should_schedule_fluid_update = false;
            }
            return match fluid_at {
                Some(id) => AquiferResult::Fluid(id),
                None => AquiferResult::Air,
            };
        }
        let sim12 = similarity(dist_sq[0], dist_sq[1]);

        if let Some(id) = fluid_at
            && id == self.water_id
        {
            let below = global_fluid(
                world_y - 1,
                self.lava_floor,
                self.sea_level,
                self.lava_id,
                self.default_fluid_id,
            );
            if below.fluid_type == self.lava_id && (world_y - 1) < below.fluid_level {
                self.should_schedule_fluid_update = true;
                return AquiferResult::Fluid(id);
            }
        }

        let mut barrier_noise = f64::NAN;
        let status2 = self.get_aquifer_status(closest_idx[1], noises);
        let barrier12 = sim12
            * self.calculate_pressure(
                noises,
                world_x,
                world_y,
                world_z,
                &mut barrier_noise,
                status1,
                status2,
            );
        if density + barrier12 > 0.0 {
            self.should_schedule_fluid_update = false;
            return AquiferResult::Solid;
        }

        let status3 = self.get_aquifer_status(closest_idx[2], noises);
        let sim13 = similarity(dist_sq[0], dist_sq[2]);
        if sim13 > 0.0 {
            let barrier13 = sim12
                * sim13
                * self.calculate_pressure(
                    noises,
                    world_x,
                    world_y,
                    world_z,
                    &mut barrier_noise,
                    status1,
                    status3,
                );
            if density + barrier13 > 0.0 {
                self.should_schedule_fluid_update = false;
                return AquiferResult::Solid;
            }
        }

        let sim23 = similarity(dist_sq[1], dist_sq[2]);
        if sim23 > 0.0 {
            let barrier23 = sim12
                * sim23
                * self.calculate_pressure(
                    noises,
                    world_x,
                    world_y,
                    world_z,
                    &mut barrier_noise,
                    status2,
                    status3,
                );
            if density + barrier23 > 0.0 {
                self.should_schedule_fluid_update = false;
                return AquiferResult::Solid;
            }
        }

        let may_flow12 = status1 != status2;
        let may_flow23 = sim23 >= FLOWING_UPDATE_SIMILARITY && status2 != status3;
        let may_flow13 = sim13 >= FLOWING_UPDATE_SIMILARITY && status1 != status3;
        if may_flow12 || may_flow23 || may_flow13 {
            self.should_schedule_fluid_update = true;
        } else {
            self.should_schedule_fluid_update = sim13 >= FLOWING_UPDATE_SIMILARITY
                && similarity(dist_sq[0], dist_sq[3]) >= FLOWING_UPDATE_SIMILARITY
                && status1 != self.get_aquifer_status(closest_idx[3], noises);
        }

        match fluid_at {
            Some(id) => AquiferResult::Fluid(id),
            None => AquiferResult::Air,
        }
    }

    pub const fn should_schedule_fluid_update(&self) -> bool {
        self.should_schedule_fluid_update
    }

    pub fn preliminary_surface_level(&mut self, noises: &Noises, x: i32, z: i32) -> i32 {
        cached_preliminary_surface_level(noises, &mut self.cache, &mut self.prelim_cache, x, z)
    }

    fn get_aquifer_status(&mut self, index: usize, noises: &Noises) -> FluidStatus {
        if let Some(status) = self.status_cache[index] {
            return status;
        }

        let loc = self.location_cache[index];
        let x = unpack_x(loc);
        let y = unpack_y(loc);
        let z = unpack_z(loc);
        let status = self.compute_fluid(x, y, z, noises);
        self.status_cache[index] = Some(status);
        status
    }

    fn compute_fluid(&mut self, x: i32, y: i32, z: i32, noises: &Noises) -> FluidStatus {
        let gf = global_fluid(
            y,
            self.lava_floor,
            self.sea_level,
            self.lava_id,
            self.default_fluid_id,
        );
        let mut lowest_surface = i32::MAX;
        let top_of_cell = y + Y_SPACING;
        let bottom_of_cell = y - Y_SPACING;
        let mut surface_under_global = false;

        for offset in &SURFACE_SAMPLING_OFFSETS {
            let sx = x + offset[0] * 16;
            let sz = z + offset[1] * 16;

            let preliminary = cached_preliminary_surface_level(
                noises,
                &mut self.cache,
                &mut self.prelim_cache,
                sx,
                sz,
            );
            let adjusted = preliminary + 8;

            let is_center = offset[0] == 0 && offset[1] == 0;

            if is_center && bottom_of_cell > adjusted {
                return gf;
            }

            let top_pokes_above = top_of_cell > adjusted;
            if top_pokes_above || is_center {
                let gf_at_surface = global_fluid(
                    adjusted,
                    self.lava_floor,
                    self.sea_level,
                    self.lava_id,
                    self.default_fluid_id,
                );
                let has_fluid = adjusted < gf_at_surface.fluid_level;
                if has_fluid {
                    if is_center {
                        surface_under_global = true;
                    }
                    if top_pokes_above {
                        return gf_at_surface;
                    }
                }
            }

            if preliminary < lowest_surface {
                lowest_surface = preliminary;
            }
        }

        let fluid_level =
            self.compute_surface_level(x, y, z, noises, gf, lowest_surface, surface_under_global);
        let fluid_type = self.compute_fluid_type(x, y, z, noises, gf, fluid_level);
        FluidStatus {
            fluid_level,
            fluid_type,
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "matches vanilla NoiseBasedAquifer.computeSurface signature"
    )]
    fn compute_surface_level(
        &mut self,
        x: i32,
        y: i32,
        z: i32,
        noises: &Noises,
        gf: FluidStatus,
        lowest_surface: i32,
        surface_under_global: bool,
    ) -> i32 {
        let (partially_flooded, fully_flooded) =
            if is_deep_dark_region(noises, &mut self.cache, x, y, z) {
                (-1.0, -1.0)
            } else {
                let dist_below = lowest_surface + 8 - y;
                let floodedness_factor = if surface_under_global {
                    map_clamped(f64::from(dist_below), 0.0, 64.0, 1.0, 0.0)
                } else {
                    0.0
                };

                self.cache.ensure(x, z, noises);
                let floodedness_noise = clamp(
                    self.cache.router_at(
                        router(
                            noises.compiled().routers().fluid_level_floodedness,
                            "fluid_level_floodedness",
                        ),
                        x,
                        y,
                        z,
                        noises,
                    ),
                    -1.0,
                    1.0,
                );

                let fully_threshold = map(floodedness_factor, 1.0, 0.0, -0.3, 0.8);
                let partially_threshold = map(floodedness_factor, 1.0, 0.0, -0.8, 0.4);

                (
                    floodedness_noise - partially_threshold,
                    floodedness_noise - fully_threshold,
                )
            };

        if fully_flooded > 0.0 {
            gf.fluid_level
        } else if partially_flooded > 0.0 {
            self.compute_randomized_fluid_surface_level(x, y, z, noises, lowest_surface)
        } else {
            WAY_BELOW_MIN_Y
        }
    }

    fn compute_randomized_fluid_surface_level(
        &mut self,
        x: i32,
        y: i32,
        z: i32,
        noises: &Noises,
        lowest_surface: i32,
    ) -> i32 {
        let cell_x = x.div_euclid(16);
        let cell_y = y.div_euclid(40);
        let cell_z = z.div_euclid(16);
        let cell_middle_y = cell_y * 40 + 20;

        self.cache.ensure(cell_x, cell_z, noises);
        let spread = self
            .cache
            .router_at(
                router(
                    noises.compiled().routers().fluid_level_spread,
                    "fluid_level_spread",
                ),
                cell_x,
                cell_y,
                cell_z,
                noises,
            )
            * 10.0;
        let spread_quantized = quantize(spread, 3);
        let target = cell_middle_y + spread_quantized;

        lowest_surface.min(target)
    }

    fn compute_fluid_type(
        &mut self,
        x: i32,
        y: i32,
        z: i32,
        noises: &Noises,
        gf: FluidStatus,
        fluid_level: i32,
    ) -> BlockState {
        if fluid_level <= -10 && fluid_level != WAY_BELOW_MIN_Y && gf.fluid_type != self.lava_id {
            let cell_x = x.div_euclid(64);
            let cell_y = y.div_euclid(40);
            let cell_z = z.div_euclid(64);
            self.cache.ensure(cell_x, cell_z, noises);
            let lava_noise = self.cache.router_at(
                router(noises.compiled().routers().lava, "lava"),
                cell_x,
                cell_y,
                cell_z,
                noises,
            );
            if lava_noise.abs() > 0.3 {
                return self.lava_id;
            }
        }
        gf.fluid_type
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "matches vanilla NoiseBasedAquifer.calculatePressure signature"
    )]
    fn calculate_pressure(
        &mut self,
        noises: &Noises,
        x: i32,
        y: i32,
        z: i32,
        barrier_noise: &mut f64,
        s1: FluidStatus,
        s2: FluidStatus,
    ) -> f64 {
        let f1 = s1.at(y);
        let f2 = s2.at(y);
        let f1_is_lava = f1 == Some(self.lava_id);
        let f2_is_lava = f2 == Some(self.lava_id);
        let f1_is_water = f1 == Some(self.water_id);
        let f2_is_water = f2 == Some(self.water_id);

        if (f1_is_lava && f2_is_water) || (f1_is_water && f2_is_lava) {
            return 2.0;
        }

        let fluid_y_diff = (s1.fluid_level - s2.fluid_level).abs();
        if fluid_y_diff == 0 {
            return 0.0;
        }

        let avg_fluid_y = 0.5 * f64::from(s1.fluid_level + s2.fluid_level);
        let above_avg = f64::from(y) + 0.5 - avg_fluid_y;
        let base = f64::from(fluid_y_diff) / 2.0;
        let edge_dist = base - above_avg.abs();

        let gradient = if above_avg > 0.0 {
            if edge_dist > 0.0 {
                edge_dist / 1.5
            } else {
                edge_dist / 2.5
            }
        } else {
            let center = 3.0 + edge_dist;
            if center > 0.0 {
                center / 3.0
            } else {
                center / 10.0
            }
        };

        let noise_val = if !(-2.0..=2.0).contains(&gradient) {
            0.0
        } else if barrier_noise.is_nan() {
            self.cache.ensure(x, z, noises);
            let n = self.cache.router_at(
                router(noises.compiled().routers().barrier, "barrier"),
                x,
                y,
                z,
                noises,
            );
            *barrier_noise = n;
            n
        } else {
            *barrier_noise
        };

        2.0 * (noise_val + gradient)
    }
}

#[inline]
fn quantize(value: f64, quantum: i32) -> i32 {
    let q = f64::from(quantum);
    (value / q).floor() as i32 * quantum
}

pub fn preliminary_surface_level(noises: &Noises, cache: &mut ColumnCache, x: i32, z: i32) -> i32 {
    let qx = (x >> 2) << 2;
    let qz = (z >> 2) << 2;
    cache.ensure(qx, qz, noises);
    cache
        .router_at(
            router(
                noises.compiled().routers().preliminary_surface_level,
                "preliminary_surface_level",
            ),
            qx,
            0,
            qz,
            noises,
        )
        .floor() as i32
}

fn cached_preliminary_surface_level(
    noises: &Noises,
    cache: &mut ColumnCache,
    prelim_cache: &mut FxHashMap<(i32, i32), i32>,
    x: i32,
    z: i32,
) -> i32 {
    let key = ((x >> 2) << 2, (z >> 2) << 2);
    if let Some(&level) = prelim_cache.get(&key) {
        return level;
    }
    let level = preliminary_surface_level(noises, cache, x, z);
    prelim_cache.insert(key, level);
    level
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test loads the checked-in vanilla datapack"
)]
mod tests {
    use crate::util::ident::Identifier;

    use std::sync::Once;

    use super::*;
    use crate::registry::Registries;
    use crate::worldgen::backend::ir::loader::DatapackSource;

    fn init_global_registry() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let registries = Registries::new_vanilla().expect("vanilla datapack should load");
            let _ = REGISTRY.init(registries);
        });
    }

    #[test]
    fn compute_substance_never_places_fluid_above_solid_density() {
        init_global_registry();

        let source = DatapackSource::builtin();
        let registry = source.density_function_registry().unwrap();
        let (program, settings) = source
            .program(
                &Identifier::new(Identifier::VANILLA_NAMESPACE, "overworld"),
                &registry,
            )
            .unwrap();
        let params = source.noise_parameters().unwrap();
        let noises = Noises::new(&program, &settings, &params, 42).unwrap();

        let aquifer_random = noises.aquifer_random().clone();
        let default_fluid_id = REGISTRY.blocks.get_default_state_id(&vanilla_blocks::WATER);

        let mut aquifer = Aquifer::new(
            0,
            0,
            noises.min_y(),
            noises.height(),
            &aquifer_random,
            settings.aquifers_enabled,
            default_fluid_id,
            &noises,
            ColumnCache::new(&noises),
        );

        assert!(matches!(
            aquifer.compute_substance(&noises, 8, 64, 8, 1.0),
            AquiferResult::Solid
        ));

        for y in (noises.min_y()..noises.min_y() + noises.height()).step_by(8) {
            match aquifer.compute_substance(&noises, 8, y, 8, -0.1) {
                AquiferResult::Fluid(id) => {
                    assert!(id == aquifer.water_id || id == aquifer.lava_id);
                }
                AquiferResult::Air | AquiferResult::Solid => {}
            }
        }
    }
}
