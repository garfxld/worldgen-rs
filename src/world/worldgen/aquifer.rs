use rustc_hash::FxHashMap;
use serde_json::Value;
use shulkr::world::block::{Block, BlockState};

use crate::util::math::{clamp, map, map_clamped};
use crate::util::rng::RandomFactory;
use crate::world::worldgen::densityfunction::context::{SamplerContext, block_pos_as_long};
use crate::world::worldgen::densityfunction::eval::{sample_value, sample_volume};
use crate::world::worldgen::densityfunction::function::Df;
use crate::world::worldgen::densityfunction::registry::DensityRegistry;
use crate::world::worldgen::densityfunction::sampler::S;
use crate::world::worldgen::densityfunction::volume::DensityVolume;
use crate::world::worldgen::random_state::RandomState;

const DIMENSION_MIN_Y: i32 = -2032;
const WAY_BELOW_MIN_Y: i32 = DIMENSION_MIN_Y << 4;

#[derive(Clone)]
pub struct Config {
    pub barrier_noise: Df,
    pub fluid_level_floodedness_noise: Df,
    pub fluid_level_spread_noise: Df,
    pub lava_noise: Df,
    pub exclusion: Df,
    pub surface_level: Df,
}

impl Config {
    pub fn parse(registry: &DensityRegistry, v: &Value) -> Result<Self, String> {
        let f = |name: &str| {
            registry.parse(v.get(name).ok_or_else(|| format!("missing field {name}"))?)
        };
        Ok(Self {
            barrier_noise: f("barrier")?,
            fluid_level_floodedness_noise: f("fluid_level_floodedness")?,
            fluid_level_spread_noise: f("fluid_level_spread")?,
            lava_noise: f("lava")?,
            exclusion: f("exclusion")?,
            surface_level: f("surface_level")?,
        })
    }

    pub fn create(
        &self,
        random_state: &RandomState,
        ctx: &mut SamplerContext<'_>,
        positional_random_factory: RandomFactory,
        volume: &DensityVolume,
        fluid_rule: FluidPicker,
    ) -> Aquifer {
        Aquifer::NoiseBased(Box::new(NoiseBasedAquifer::new(
            random_state,
            ctx,
            self,
            positional_random_factory,
            volume,
            fluid_rule,
        )))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FluidStatus {
    pub fluid_level: i32,
    pub fluid_type: BlockState,
}

impl FluidStatus {
    pub const fn at(&self, block_y: i32) -> BlockState {
        if block_y < self.fluid_level {
            self.fluid_type
        } else {
            Block::AIR.default_state()
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FluidPicker {
    lava_status: FluidStatus,
    sea_status: FluidStatus,
    sea_level: i32,
}

impl FluidPicker {
    pub const fn new(sea_level: i32, default_fluid: BlockState) -> Self {
        Self {
            lava_status: FluidStatus {
                fluid_level: -54,
                fluid_type: Block::LAVA.default_state(),
            },
            sea_status: FluidStatus {
                fluid_level: sea_level,
                fluid_type: default_fluid,
            },
            sea_level,
        }
    }

    pub const fn compute_fluid(&self, _block_x: i32, block_y: i32, _block_z: i32) -> FluidStatus {
        if block_y < (-54).min(self.sea_level) {
            self.lava_status
        } else {
            self.sea_status
        }
    }
}

pub enum Aquifer {
    Disabled(FluidPicker),
    NoiseBased(Box<NoiseBasedAquifer>),
}

impl Aquifer {
    pub const fn create_disabled(fluid_rule: FluidPicker) -> Self {
        Self::Disabled(fluid_rule)
    }

    pub fn compute_substance(
        &mut self,
        ctx: &mut SamplerContext<'_>,
        block_x: i32,
        block_y: i32,
        block_z: i32,
        density: f64,
    ) -> Option<BlockState> {
        match self {
            Self::Disabled(fluid_rule) => {
                if density > 0.0 {
                    None
                } else {
                    Some(
                        fluid_rule
                            .compute_fluid(block_x, block_y, block_z)
                            .at(block_y),
                    )
                }
            }
            Self::NoiseBased(aquifer) => {
                aquifer.compute_substance(ctx, block_x, block_y, block_z, density)
            }
        }
    }

    pub fn should_schedule_fluid_update(&self) -> bool {
        match self {
            Self::Disabled(_) => false,
            Self::NoiseBased(aquifer) => aquifer.should_schedule_fluid_update,
        }
    }
}

const SURFACE_SAMPLING_OFFSETS_IN_CHUNKS: [[i32; 2]; 13] = [
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

fn similarity(distance_sqr1: i32, distance_sqr2: i32) -> f64 {
    1.0 - f64::from(distance_sqr2 - distance_sqr1) / 25.0
}

fn flowing_update_similarity() -> f64 {
    similarity(10 * 10, 12 * 12)
}

const fn grid_x(block_coord: i32) -> i32 {
    block_coord >> 4
}

const fn from_grid_x(grid_coord: i32, block_offset: i32) -> i32 {
    (grid_coord << 4) + block_offset
}

const fn grid_y(block_coord: i32) -> i32 {
    block_coord.div_euclid(12)
}

const fn from_grid_y(grid_coord: i32, block_offset: i32) -> i32 {
    grid_coord * 12 + block_offset
}

const fn grid_z(block_coord: i32) -> i32 {
    block_coord >> 4
}

const fn from_grid_z(grid_coord: i32, block_offset: i32) -> i32 {
    (grid_coord << 4) + block_offset
}

const fn block_pos_x(packed: i64) -> i32 {
    (packed >> 38) as i32
}

const fn block_pos_y(packed: i64) -> i32 {
    ((packed << 52) >> 52) as i32
}

const fn block_pos_z(packed: i64) -> i32 {
    ((packed << 26) >> 38) as i32
}

const fn quart_quantize(block: i32) -> i32 {
    (block >> 2) << 2
}

pub struct NoiseBasedAquifer {
    barrier_noise: S,
    fluid_level_floodedness_noise: S,
    fluid_level_spread_noise: S,
    lava_noise: S,
    positional_random_factory: RandomFactory,
    aquifer_cache: Vec<Option<FluidStatus>>,
    aquifer_location_cache: Vec<i64>,
    global_fluid_picker: FluidPicker,
    exclusion: S,
    should_schedule_fluid_update: bool,
    skip_sampling_above_y: i32,
    min_grid_x: i32,
    min_grid_y: i32,
    min_grid_z: i32,
    grid_size_x: i32,
    grid_size_z: i32,
    surface_level: S,
    surface_level_cache: FxHashMap<(i32, i32), i32>,
}

impl NoiseBasedAquifer {
    fn new(
        random_state: &RandomState,
        ctx: &mut SamplerContext<'_>,
        config: &Config,
        positional_random_factory: RandomFactory,
        volume: &DensityVolume,
        global_fluid_picker: FluidPicker,
    ) -> Self {
        let min_grid_x = grid_x(volume.min_block_x + -5);
        let max_grid_x = grid_x(volume.max_block_x() + -5) + 1;
        let grid_size_x = max_grid_x - min_grid_x + 1;
        let min_grid_y = grid_y(volume.min_block_y + 1) + -1;
        let max_grid_y = grid_y(volume.max_block_y() + 1) + 1;
        let grid_size_y = max_grid_y - min_grid_y + 1;
        let min_grid_z = grid_z(volume.min_block_z + -5);
        let max_grid_z = grid_z(volume.max_block_z() + -5) + 1;
        let grid_size_z = max_grid_z - min_grid_z + 1;
        let total_grid_size = (grid_size_x * grid_size_y * grid_size_z) as usize;
        let mut aquifer = Self {
            barrier_noise: random_state.get_sampler(&config.barrier_noise),
            fluid_level_floodedness_noise: random_state
                .get_sampler(&config.fluid_level_floodedness_noise),
            fluid_level_spread_noise: random_state.get_sampler(&config.fluid_level_spread_noise),
            lava_noise: random_state.get_sampler(&config.lava_noise),
            exclusion: random_state.get_sampler(&config.exclusion),
            surface_level: random_state.get_sampler(&config.surface_level),
            positional_random_factory,
            aquifer_cache: vec![None; total_grid_size],
            aquifer_location_cache: vec![i64::MAX; total_grid_size],
            global_fluid_picker,
            should_schedule_fluid_update: false,
            skip_sampling_above_y: 0,
            min_grid_x,
            min_grid_y,
            min_grid_z,
            grid_size_x,
            grid_size_z,
            surface_level_cache: FxHashMap::default(),
        };
        let max_adjusted_surface_level = Self::adjust_surface_level(aquifer.max_surface_level(
            ctx,
            from_grid_x(min_grid_x, 0),
            from_grid_z(min_grid_z, 0),
            from_grid_x(max_grid_x, 9),
            from_grid_z(max_grid_z, 9),
        ));
        let skip_sampling_above_grid_y = grid_y(max_adjusted_surface_level + 12) - -1;
        aquifer.skip_sampling_above_y = from_grid_y(skip_sampling_above_grid_y, 11) - 1;
        aquifer
    }

    fn surface_level(&mut self, ctx: &mut SamplerContext<'_>, block_x: i32, block_z: i32) -> i32 {
        let quantized_x = quart_quantize(block_x);
        let quantized_z = quart_quantize(block_z);
        let surface_level = &self.surface_level;
        *self
            .surface_level_cache
            .entry((quantized_x, quantized_z))
            .or_insert_with(|| {
                sample_value(surface_level, ctx, quantized_x, 0, quantized_z).floor() as i32
            })
    }

    fn max_surface_level(
        &mut self,
        ctx: &mut SamplerContext<'_>,
        min_block_x: i32,
        min_block_z: i32,
        max_block_x: i32,
        max_block_z: i32,
    ) -> i32 {
        let min_quart_x = min_block_x >> 2;
        let max_quart_x = max_block_x >> 2;
        let min_quart_z = min_block_z >> 2;
        let max_quart_z = max_block_z >> 2;
        let volume = DensityVolume::stepped(
            [
                max_quart_x - min_quart_x + 1,
                1,
                max_quart_z - min_quart_z + 1,
            ],
            [min_quart_x << 2, 0, min_quart_z << 2],
            [4, 1, 4],
        );
        let mut buffer = vec![0.0f32; volume.size()];
        sample_volume(&self.surface_level, ctx, &mut buffer, &volume);
        let mut max_y = i32::MIN;
        for z in 0..volume.size_z {
            for x in 0..volume.size_x {
                let surface_level = buffer[volume.index_unchecked(x, 0, z)].floor() as i32;
                self.surface_level_cache
                    .insert((volume.block_x(x), volume.block_z(z)), surface_level);
                if surface_level > max_y {
                    max_y = surface_level;
                }
            }
        }
        max_y
    }

    const fn get_index(&self, grid_x: i32, grid_y: i32, grid_z: i32) -> usize {
        let x = grid_x - self.min_grid_x;
        let y = grid_y - self.min_grid_y;
        let z = grid_z - self.min_grid_z;
        ((y * self.grid_size_z + z) * self.grid_size_x + x) as usize
    }

    #[expect(
        clippy::too_many_lines,
        reason = "literal port of NoiseBasedAquifer.computeSubstance"
    )]
    fn compute_substance(
        &mut self,
        ctx: &mut SamplerContext<'_>,
        block_x: i32,
        block_y: i32,
        block_z: i32,
        density: f64,
    ) -> Option<BlockState> {
        if density > 0.0 {
            self.should_schedule_fluid_update = false;
            return None;
        }

        let global_fluid = self
            .global_fluid_picker
            .compute_fluid(block_x, block_y, block_z);
        if block_y > self.skip_sampling_above_y {
            self.should_schedule_fluid_update = false;
            return Some(global_fluid.at(block_y));
        }

        if global_fluid.at(block_y).is(Block::LAVA) {
            self.should_schedule_fluid_update = false;
            return Some(Block::LAVA.default_state());
        }

        let x_anchor = grid_x(block_x + -5);
        let y_anchor = grid_y(block_y + 1);
        let z_anchor = grid_z(block_z + -5);
        let mut distance_sqr1 = i32::MAX;
        let mut distance_sqr2 = i32::MAX;
        let mut distance_sqr3 = i32::MAX;
        let mut distance_sqr4 = i32::MAX;
        let mut closest_index1 = 0;
        let mut closest_index2 = 0;
        let mut closest_index3 = 0;
        let mut closest_index4 = 0;

        for x1 in 0..=1 {
            for y1 in -1..=1 {
                for z1 in 0..=1 {
                    let spaced_grid_x = x_anchor + x1;
                    let spaced_grid_y = y_anchor + y1;
                    let spaced_grid_z = z_anchor + z1;
                    let index = self.get_index(spaced_grid_x, spaced_grid_y, spaced_grid_z);
                    let existing_location = self.aquifer_location_cache[index];
                    let location = if existing_location == i64::MAX {
                        let mut random = self.positional_random_factory.at(
                            spaced_grid_x,
                            spaced_grid_y,
                            spaced_grid_z,
                        );
                        let x = from_grid_x(spaced_grid_x, random.next_i32_bounded(10));
                        let y = from_grid_y(spaced_grid_y, random.next_i32_bounded(9));
                        let z = from_grid_z(spaced_grid_z, random.next_i32_bounded(10));
                        let location = block_pos_as_long(x, y, z);
                        self.aquifer_location_cache[index] = location;
                        location
                    } else {
                        existing_location
                    };

                    let dx = block_pos_x(location) - block_x;
                    let dy = block_pos_y(location) - block_y;
                    let dz = block_pos_z(location) - block_z;
                    let new_distance = dx * dx + dy * dy + dz * dz;
                    if distance_sqr1 >= new_distance {
                        closest_index4 = closest_index3;
                        closest_index3 = closest_index2;
                        closest_index2 = closest_index1;
                        closest_index1 = index;
                        distance_sqr4 = distance_sqr3;
                        distance_sqr3 = distance_sqr2;
                        distance_sqr2 = distance_sqr1;
                        distance_sqr1 = new_distance;
                    } else if distance_sqr2 >= new_distance {
                        closest_index4 = closest_index3;
                        closest_index3 = closest_index2;
                        closest_index2 = index;
                        distance_sqr4 = distance_sqr3;
                        distance_sqr3 = distance_sqr2;
                        distance_sqr2 = new_distance;
                    } else if distance_sqr3 >= new_distance {
                        closest_index4 = closest_index3;
                        closest_index3 = index;
                        distance_sqr4 = distance_sqr3;
                        distance_sqr3 = new_distance;
                    } else if distance_sqr4 >= new_distance {
                        closest_index4 = index;
                        distance_sqr4 = new_distance;
                    }
                }
            }
        }

        let flowing_update_similarity = flowing_update_similarity();
        let closest_status1 = self.get_aquifer_status(ctx, closest_index1);
        let similarity12 = similarity(distance_sqr1, distance_sqr2);
        let fluid_state = closest_status1.at(block_y);
        if similarity12 <= 0.0 {
            if similarity12 >= flowing_update_similarity {
                let closest_status2 = self.get_aquifer_status(ctx, closest_index2);
                self.should_schedule_fluid_update = closest_status1 != closest_status2;
            } else {
                self.should_schedule_fluid_update = false;
            }
            return Some(fluid_state);
        }

        if fluid_state.is(Block::WATER)
            && self
                .global_fluid_picker
                .compute_fluid(block_x, block_y - 1, block_z)
                .at(block_y - 1)
                .is(Block::LAVA)
        {
            self.should_schedule_fluid_update = true;
            return Some(fluid_state);
        }

        let mut barrier_noise_value = f64::NAN;
        let closest_status2 = self.get_aquifer_status(ctx, closest_index2);
        let barrier12 = similarity12
            * self.calculate_pressure(
                ctx,
                block_x,
                block_y,
                block_z,
                &mut barrier_noise_value,
                closest_status1,
                closest_status2,
            );
        if density + barrier12 > 0.0 {
            self.should_schedule_fluid_update = false;
            return None;
        }

        let closest_status3 = self.get_aquifer_status(ctx, closest_index3);
        let similarity13 = similarity(distance_sqr1, distance_sqr3);
        if similarity13 > 0.0 {
            let barrier13 = similarity12
                * similarity13
                * self.calculate_pressure(
                    ctx,
                    block_x,
                    block_y,
                    block_z,
                    &mut barrier_noise_value,
                    closest_status1,
                    closest_status3,
                );
            if density + barrier13 > 0.0 {
                self.should_schedule_fluid_update = false;
                return None;
            }
        }

        let similarity23 = similarity(distance_sqr2, distance_sqr3);
        if similarity23 > 0.0 {
            let barrier23 = similarity12
                * similarity23
                * self.calculate_pressure(
                    ctx,
                    block_x,
                    block_y,
                    block_z,
                    &mut barrier_noise_value,
                    closest_status2,
                    closest_status3,
                );
            if density + barrier23 > 0.0 {
                self.should_schedule_fluid_update = false;
                return None;
            }
        }

        let may_flow12 = closest_status1 != closest_status2;
        let may_flow23 =
            similarity23 >= flowing_update_similarity && closest_status2 != closest_status3;
        let may_flow13 =
            similarity13 >= flowing_update_similarity && closest_status1 != closest_status3;
        if !may_flow12 && !may_flow23 && !may_flow13 {
            self.should_schedule_fluid_update = similarity13 >= flowing_update_similarity
                && similarity(distance_sqr1, distance_sqr4) >= flowing_update_similarity
                && closest_status1 != self.get_aquifer_status(ctx, closest_index4);
        } else {
            self.should_schedule_fluid_update = true;
        }

        Some(fluid_state)
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors NoiseBasedAquifer.calculatePressure"
    )]
    fn calculate_pressure(
        &self,
        ctx: &mut SamplerContext<'_>,
        block_x: i32,
        block_y: i32,
        block_z: i32,
        barrier_noise_value: &mut f64,
        status_closest1: FluidStatus,
        status_closest2: FluidStatus,
    ) -> f64 {
        let type1 = status_closest1.at(block_y);
        let type2 = status_closest2.at(block_y);
        if (type1.is(Block::LAVA) && type2.is(Block::WATER))
            || (type1.is(Block::WATER) && type2.is(Block::LAVA))
        {
            return 2.0;
        }

        let fluid_y_diff = (status_closest1.fluid_level - status_closest2.fluid_level).abs();
        if fluid_y_diff == 0 {
            return 0.0;
        }

        let average_fluid_y =
            0.5 * f64::from(status_closest1.fluid_level + status_closest2.fluid_level);
        let how_far_above_average_fluid_point = f64::from(block_y) + 0.5 - average_fluid_y;
        let base_value = f64::from(fluid_y_diff) / 2.0;
        let distance_from_barrier_edge_towards_middle =
            base_value - how_far_above_average_fluid_point.abs();
        let gradient = if how_far_above_average_fluid_point > 0.0 {
            let center_point = 0.0 + distance_from_barrier_edge_towards_middle;
            if center_point > 0.0 {
                center_point / 1.5
            } else {
                center_point / 2.5
            }
        } else {
            let center_point = 3.0 + distance_from_barrier_edge_towards_middle;
            if center_point > 0.0 {
                center_point / 3.0
            } else {
                center_point / 10.0
            }
        };

        #[expect(
            clippy::neg_cmp_op_on_partial_ord,
            reason = "NaN passes like vanilla's !(a < b)"
        )]
        let in_range = !(gradient < -2.0) && !(gradient > 2.0);
        let noise_value = if in_range {
            if barrier_noise_value.is_nan() {
                let barrier_noise = f64::from(sample_value(
                    &self.barrier_noise,
                    ctx,
                    block_x,
                    block_y,
                    block_z,
                ));
                *barrier_noise_value = barrier_noise;
                barrier_noise
            } else {
                *barrier_noise_value
            }
        } else {
            0.0
        };

        2.0 * (noise_value + gradient)
    }

    fn get_aquifer_status(&mut self, ctx: &mut SamplerContext<'_>, index: usize) -> FluidStatus {
        if let Some(old_status) = self.aquifer_cache[index] {
            return old_status;
        }
        let location = self.aquifer_location_cache[index];
        let status = self.compute_fluid(
            ctx,
            block_pos_x(location),
            block_pos_y(location),
            block_pos_z(location),
        );
        self.aquifer_cache[index] = Some(status);
        status
    }

    fn compute_fluid(
        &mut self,
        ctx: &mut SamplerContext<'_>,
        x: i32,
        y: i32,
        z: i32,
    ) -> FluidStatus {
        let global_fluid = self.global_fluid_picker.compute_fluid(x, y, z);
        let mut lowest_preliminary_surface = i32::MAX;
        let top_of_aquifer_cell = y + 12;
        let bottom_of_aquifer_cell = y - 12;
        let mut surface_at_center_is_under_global_fluid_level = false;

        for offset in SURFACE_SAMPLING_OFFSETS_IN_CHUNKS {
            let sample_x = x + (offset[0] << 4);
            let sample_z = z + (offset[1] << 4);
            let surface_level = self.surface_level(ctx, sample_x, sample_z);
            let adjusted_surface_level = Self::adjust_surface_level(surface_level);
            let start = offset[0] == 0 && offset[1] == 0;
            if start && bottom_of_aquifer_cell > adjusted_surface_level {
                return global_fluid;
            }

            let top_of_aquifer_cell_pokes_above_surface =
                top_of_aquifer_cell > adjusted_surface_level;
            if top_of_aquifer_cell_pokes_above_surface || start {
                let global_fluid_at_surface = self.global_fluid_picker.compute_fluid(
                    sample_x,
                    adjusted_surface_level,
                    sample_z,
                );
                if !global_fluid_at_surface.at(adjusted_surface_level).is_air() {
                    if start {
                        surface_at_center_is_under_global_fluid_level = true;
                    }
                    if top_of_aquifer_cell_pokes_above_surface {
                        return global_fluid_at_surface;
                    }
                }
            }

            lowest_preliminary_surface = lowest_preliminary_surface.min(surface_level);
        }

        let fluid_surface_level = self.compute_surface_level(
            ctx,
            x,
            y,
            z,
            global_fluid,
            lowest_preliminary_surface,
            surface_at_center_is_under_global_fluid_level,
        );
        FluidStatus {
            fluid_level: fluid_surface_level,
            fluid_type: self.compute_fluid_type(ctx, x, y, z, global_fluid, fluid_surface_level),
        }
    }

    const fn adjust_surface_level(preliminary_surface_level: i32) -> i32 {
        preliminary_surface_level + 8
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors NoiseBasedAquifer.computeSurfaceLevel"
    )]
    fn compute_surface_level(
        &self,
        ctx: &mut SamplerContext<'_>,
        x: i32,
        y: i32,
        z: i32,
        global_fluid: FluidStatus,
        lowest_surface_level: i32,
        surface_at_center_is_under_global_fluid_level: bool,
    ) -> i32 {
        let (partially_floodedness, fully_floodedness) =
            if sample_value(&self.exclusion, ctx, x, y, z) > 0.0 {
                (-1.0, -1.0)
            } else {
                let distance_below_surface = Self::adjust_surface_level(lowest_surface_level) - y;
                let floodedness_factor = if surface_at_center_is_under_global_fluid_level {
                    map_clamped(f64::from(distance_below_surface), 0.0, 64.0, 1.0, 0.0)
                } else {
                    0.0
                };
                let floodedness_noise_value = clamp(
                    f64::from(sample_value(
                        &self.fluid_level_floodedness_noise,
                        ctx,
                        x,
                        y,
                        z,
                    )),
                    -1.0,
                    1.0,
                );
                let fully_flooded_threshold = map(floodedness_factor, 1.0, 0.0, -0.3, 0.8);
                let partially_flooded_threshold = map(floodedness_factor, 1.0, 0.0, -0.8, 0.4);
                (
                    floodedness_noise_value - partially_flooded_threshold,
                    floodedness_noise_value - fully_flooded_threshold,
                )
            };

        if fully_floodedness > 0.0 {
            global_fluid.fluid_level
        } else if partially_floodedness > 0.0 {
            self.compute_randomized_fluid_surface_level(ctx, x, y, z, lowest_surface_level)
        } else {
            WAY_BELOW_MIN_Y
        }
    }

    fn compute_randomized_fluid_surface_level(
        &self,
        ctx: &mut SamplerContext<'_>,
        x: i32,
        y: i32,
        z: i32,
        lowest_surface_level: i32,
    ) -> i32 {
        let fluid_level_cell_x = x.div_euclid(16);
        let fluid_level_cell_y = y.div_euclid(40);
        let fluid_level_cell_z = z.div_euclid(16);
        let fluid_cell_middle_y = fluid_level_cell_y * 40 + 20;
        let fluid_level_spread = f64::from(
            sample_value(
                &self.fluid_level_spread_noise,
                ctx,
                fluid_level_cell_x,
                fluid_level_cell_y,
                fluid_level_cell_z,
            ) * 10.0,
        );
        let fluid_level_spread_quantized = (fluid_level_spread / 3.0).floor() as i32 * 3;
        let target_fluid_surface_level = fluid_cell_middle_y + fluid_level_spread_quantized;
        lowest_surface_level.min(target_fluid_surface_level)
    }

    fn compute_fluid_type(
        &self,
        ctx: &mut SamplerContext<'_>,
        x: i32,
        y: i32,
        z: i32,
        global_fluid: FluidStatus,
        fluid_surface_level: i32,
    ) -> BlockState {
        let mut fluid_type = global_fluid.fluid_type;
        if fluid_surface_level <= -10
            && fluid_surface_level != WAY_BELOW_MIN_Y
            && global_fluid.fluid_type != Block::LAVA.default_state()
        {
            let fluid_type_cell_x = x.div_euclid(64);
            let fluid_type_cell_y = y.div_euclid(40);
            let fluid_type_cell_z = z.div_euclid(64);
            let lava_noise_value = sample_value(
                &self.lava_noise,
                ctx,
                fluid_type_cell_x,
                fluid_type_cell_y,
                fluid_type_cell_z,
            );
            if f64::from(lava_noise_value).abs() > 0.3 {
                fluid_type = Block::LAVA.default_state();
            }
        }
        fluid_type
    }
}
