use std::cell::Cell;
use std::mem::replace;
use std::sync::LazyLock;

use glam::IVec3;
use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use crate::chunk::heightmap::Heightmap;
use crate::chunk::proto::{CarversPhase, GenerationChunk};
use crate::registry::REGISTRY;
use crate::registry::biome::BiomeRef;
use crate::util::ident::{BlockState, Identifier};
use crate::util::math::{lerp2, sin};
use crate::util::pos::{BlockPos, ChunkPos};
use crate::util::rng::{RandomSource, legacy_random::LegacyRandomSource};
use crate::worldgen::density::{Aquifer, AquiferResult, Noises};
use crate::worldgen::surface::{
    DatapackSurfaceRules, SurfaceConditionNoiseCache, SurfaceRuleContext, SurfaceSystem,
    TemperatureXzCache,
};

pub mod canyon;
pub mod cave;
mod mask;

pub use mask::CarvingMask;

#[derive(Debug, Clone, Copy)]
pub struct PreliminarySurfaceCorners {
    pub nw: i32,
    pub ne: i32,
    pub sw: i32,
    pub se: i32,
}

#[derive(Debug, Clone, Copy)]
pub struct SourceChunk {
    pub pos: ChunkPos,
    pub biome: BiomeRef,
}

pub struct CarvingContext<'a> {
    pub min_y: i32,
    pub gen_depth: i32,
    pub surface_system: &'a SurfaceSystem,
    pub surface_rules: &'a DatapackSurfaceRules,
    pub aquifer: &'a mut Aquifer,
    pub default_block_id: BlockState,
    pub psl_corners: PreliminarySurfaceCorners,
    pub chunk_min_x: i32,
    pub chunk_min_z: i32,
}

impl CarvingContext<'_> {
    pub fn min_surface_level(&self, block_x: i32, block_z: i32) -> i32 {
        let local_x = (block_x - self.chunk_min_x).clamp(0, 16);
        let local_z = (block_z - self.chunk_min_z).clamp(0, 16);
        let t_x = f64::from(local_x as u8) / 16.0;
        let t_z = f64::from(local_z as u8) / 16.0;
        let c = self.psl_corners;
        let interp = lerp2(
            t_x,
            t_z,
            f64::from(c.nw),
            f64::from(c.ne),
            f64::from(c.sw),
            f64::from(c.se),
        );
        interp.floor() as i32
    }

    pub fn top_material(
        &self,
        biome_id: u16,
        block_x: i32,
        block_y: i32,
        block_z: i32,
        steep: bool,
        under_fluid: bool,
    ) -> Option<BlockState> {
        let surface_depth = self.surface_system.get_surface_depth(block_x, block_z);
        let surface_secondary = self.surface_system.get_surface_secondary(block_x, block_z);
        let min_surface_level = self.min_surface_level(block_x, block_z) + surface_depth - 8;

        let water_height = if under_fluid { block_y + 1 } else { i32::MIN };
        let condition_noise_values: SmallVec<[Cell<f64>; 8]> = self
            .surface_rules
            .noise_ids()
            .iter()
            .map(|_| Cell::new(0.0))
            .collect();
        let condition_noise_computed_at: SmallVec<[Cell<u64>; 8]> = self
            .surface_rules
            .noise_ids()
            .iter()
            .map(|_| Cell::new(0))
            .collect();
        let condition_noise_cache =
            SurfaceConditionNoiseCache::new(&condition_noise_values, &condition_noise_computed_at);
        condition_noise_cache.reset();

        let mut xz = TemperatureXzCache::new(block_x, block_z);

        let mut ctx = SurfaceRuleContext::new(
            block_x,
            block_z,
            surface_depth,
            surface_secondary,
            min_surface_level,
            steep,
            block_y,
            1,
            1,
            water_height,
            Some(biome_id),
            None,
            self.surface_system,
            &condition_noise_cache,
            &mut xz,
            self.surface_rules.block_states(),
        );

        self.surface_rules.try_apply(&mut ctx)
    }
}

pub fn can_replace_block(state: BlockState, tag: &Identifier) -> bool {
    if state.is_air() {
        return false;
    }
    let Some(block) = REGISTRY.blocks.by_state_id(state) else {
        return false;
    };
    block.has_tag(tag)
}

#[derive(Debug)]
pub struct CarverReplaceableStates {
    states: Box<[bool]>,
}

impl CarverReplaceableStates {
    fn build(tag: &Identifier) -> Self {
        let states = REGISTRY
            .blocks
            .state_to_block_lookup
            .iter()
            .map(|&block| block.has_tag(tag))
            .collect();
        Self { states }
    }

    #[inline]
    pub fn contains(&self, state: BlockState) -> bool {
        self.states.get(state.0 as usize).copied().unwrap_or(false)
    }
}

static CARVER_REPLACEABLE_STATES: LazyLock<FxHashMap<Identifier, CarverReplaceableStates>> =
    LazyLock::new(|| {
        let mut states_by_tag = FxHashMap::default();
        for (_, carver) in REGISTRY.carvers.iter() {
            let tag = &carver.base().replaceable_tag;
            if !states_by_tag.contains_key(tag) {
                states_by_tag.insert(tag.clone(), CarverReplaceableStates::build(tag));
            }
        }
        states_by_tag
    });

pub fn cached_replaceable_states(tag: &Identifier) -> Option<&'static CarverReplaceableStates> {
    CARVER_REPLACEABLE_STATES.get(tag)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CarverStyle {
    Overworld,
    Nether,
}

#[derive(Debug, Clone, Copy)]
pub struct CarverBlockIds {
    pub air: BlockState,
    pub cave_air: BlockState,
    pub lava: BlockState,
    pub grass_block: BlockState,
    pub mycelium: BlockState,
    pub dirt: BlockState,
}

impl CarverBlockIds {
    pub fn load() -> Self {
        static IDS: LazyLock<CarverBlockIds> = LazyLock::new(CarverBlockIds::load_uncached);
        *IDS
    }

    fn load_uncached() -> Self {
        use crate::registry::blocks::vanilla_blocks;
        Self {
            air: REGISTRY.blocks.get_default_state_id(&vanilla_blocks::AIR),
            cave_air: REGISTRY
                .blocks
                .get_default_state_id(&vanilla_blocks::CAVE_AIR),
            lava: REGISTRY.blocks.get_default_state_id(&vanilla_blocks::LAVA),
            grass_block: REGISTRY
                .blocks
                .get_default_state_id(&vanilla_blocks::GRASS_BLOCK),
            mycelium: REGISTRY
                .blocks
                .get_default_state_id(&vanilla_blocks::MYCELIUM),
            dirt: REGISTRY.blocks.get_default_state_id(&vanilla_blocks::DIRT),
        }
    }

    pub const fn is_air_like(&self, state: BlockState) -> bool {
        state.0 == self.air.0 || state.0 == self.cave_air.0
    }
}

pub trait CarveSkipChecker {
    fn should_skip(&mut self, xd: f64, yd: f64, zd: f64, world_y: i32) -> bool;
}

impl<F: FnMut(f64, f64, f64, i32) -> bool> CarveSkipChecker for F {
    fn should_skip(&mut self, xd: f64, yd: f64, zd: f64, world_y: i32) -> bool {
        self(xd, yd, zd, world_y)
    }
}

pub struct CarveParams<'a> {
    pub replaceable_tag: &'a Identifier,
    pub replaceable_states: Option<&'static CarverReplaceableStates>,
    pub lava_level_y: i32,
    pub style: CarverStyle,
}

pub(super) fn with_random_source<T>(
    random: &mut LegacyRandomSource,
    f: impl FnOnce(&mut RandomSource) -> T,
) -> T {
    let mut wrapped = RandomSource::Legacy(replace(random, LegacyRandomSource::from_seed(0)));
    let result = f(&mut wrapped);
    let RandomSource::Legacy(inner) = wrapped else {
        unreachable!()
    };
    *random = inner;
    result
}

#[inline]
pub(super) fn horizontal_tunnel_radius(progress_arg: f32, thickness: f32) -> f64 {
    let radius_offset = f64::from(sin(f64::from(progress_arg))) * f64::from(thickness);
    1.5 + radius_offset
}

enum CarveState {
    Place(BlockState),
    Skip,
}

pub struct CarveRun<'a, 'b, F>
where
    F: FnMut(BlockPos) -> u16,
{
    pub ctx: &'a mut CarvingContext<'b>,
    pub noises: &'a Noises,
    pub chunk: GenerationChunk<'a, CarversPhase>,
    pub chunk_min_x: i32,
    pub chunk_min_z: i32,
    pub biome_getter: &'a mut F,
    pub mask: &'a mut CarvingMask,
    pub ids: CarverBlockIds,
}

impl<F> CarveRun<'_, '_, F>
where
    F: FnMut(BlockPos) -> u16,
{
    #[expect(
        clippy::similar_names,
        reason = "min_x_idx / min_z_idx / max_x_idx / max_z_idx mirror vanilla"
    )]
    #[expect(
        clippy::too_many_arguments,
        reason = "params + x/y/z/horizontal_radius/vertical_radius + skip_checker mirrors vanilla"
    )]
    pub fn carve_ellipsoid<S: CarveSkipChecker>(
        &mut self,
        params: &CarveParams<'_>,
        x: f64,
        y: f64,
        z: f64,
        horizontal_radius: f64,
        vertical_radius: f64,
        mut skip_checker: S,
    ) -> bool {
        let middle_x = f64::from(self.chunk_min_x) + 8.0;
        let middle_z = f64::from(self.chunk_min_z) + 8.0;
        let max_delta = 16.0 + horizontal_radius * 2.0;
        if (x - middle_x).abs() > max_delta || (z - middle_z).abs() > max_delta {
            return false;
        }

        let min_x_idx = ((x - horizontal_radius).floor() as i32 - self.chunk_min_x - 1).max(0);
        let max_x_idx = ((x + horizontal_radius).floor() as i32 - self.chunk_min_x).min(15);
        let min_y = ((y - vertical_radius).floor() as i32 - 1).max(self.ctx.min_y + 1);
        let protected_blocks_on_top = 7;
        let max_y = ((y + vertical_radius).floor() as i32 + 1)
            .min(self.ctx.min_y + self.ctx.gen_depth - 1 - protected_blocks_on_top);
        let min_z_idx = ((z - horizontal_radius).floor() as i32 - self.chunk_min_z - 1).max(0);
        let max_z_idx = ((z + horizontal_radius).floor() as i32 - self.chunk_min_z).min(15);

        let mut carved = false;

        for x_idx in min_x_idx..=max_x_idx {
            let world_x = self.chunk_min_x + x_idx;
            let xd = (f64::from(world_x) + 0.5 - x) / horizontal_radius;

            for z_idx in min_z_idx..=max_z_idx {
                let world_z = self.chunk_min_z + z_idx;
                let zd = (f64::from(world_z) + 0.5 - z) / horizontal_radius;
                if xd * xd + zd * zd >= 1.0 {
                    continue;
                }

                let mut has_grass = false;

                for world_y in (min_y + 1..=max_y).rev() {
                    let yd = (f64::from(world_y) - 0.5 - y) / vertical_radius;
                    if skip_checker.should_skip(xd, yd, zd, world_y) {
                        continue;
                    }
                    if !self.mask.set_if_unset(x_idx, world_y, z_idx) {
                        continue;
                    }
                    if self.carve_block(params, world_x, world_y, world_z, &mut has_grass) {
                        carved = true;
                    }
                }
            }
        }

        carved
    }

    fn carve_block(
        &mut self,
        params: &CarveParams<'_>,
        world_x: i32,
        world_y: i32,
        world_z: i32,
        has_grass: &mut bool,
    ) -> bool {
        let pos = BlockPos::new(world_x, world_y, world_z);
        let existing = self.chunk.get_block_state(pos);

        if existing == self.ids.grass_block || existing == self.ids.mycelium {
            *has_grass = true;
        }

        if !Self::can_replace(params, existing) {
            return false;
        }

        let state = match self.get_carve_state(params, world_x, world_y, world_z) {
            CarveState::Place(id) => id,
            CarveState::Skip => return false,
        };

        self.chunk.set_block_state(pos, state);
        if params.style == CarverStyle::Overworld
            && self.ctx.aquifer.should_schedule_fluid_update()
            && state.has_fluid()
        {
            self.chunk.mark_pos_for_postprocessing(pos);
        }

        if params.style == CarverStyle::Overworld && *has_grass {
            let below_pos = BlockPos::new(world_x, world_y - 1, world_z);
            if self.chunk.get_block_state(below_pos) == self.ids.dirt {
                let under_fluid = !self.ids.is_air_like(state);
                let steep = self.steep_material_condition(world_x, world_z);
                let biome_id =
                    (self.biome_getter)(BlockPos(IVec3::new(world_x, world_y - 1, world_z)));
                if let Some(top) = self.ctx.top_material(
                    biome_id,
                    world_x,
                    world_y - 1,
                    world_z,
                    steep,
                    under_fluid,
                ) {
                    self.chunk.set_block_state(below_pos, top);
                    if top.has_fluid() {
                        self.chunk.mark_pos_for_postprocessing(below_pos);
                    }
                }
            }
        }

        true
    }

    #[inline]
    fn can_replace(params: &CarveParams<'_>, state: BlockState) -> bool {
        if state.is_air() {
            return false;
        }
        if let Some(states) = params.replaceable_states {
            return states.contains(state);
        }
        can_replace_block(state, params.replaceable_tag)
    }

    fn steep_material_condition(&self, world_x: i32, world_z: i32) -> bool {
        let Some(steep) = self.chunk.with_world_surface_heightmap(|worldgen_surface| {
            steep_material_condition(worldgen_surface, world_x, world_z)
        }) else {
            return false;
        };
        steep
    }

    fn get_carve_state(&mut self, params: &CarveParams<'_>, x: i32, y: i32, z: i32) -> CarveState {
        match params.style {
            CarverStyle::Overworld => {
                if y <= params.lava_level_y {
                    return CarveState::Place(self.ids.lava);
                }
                match self
                    .ctx
                    .aquifer
                    .compute_substance(self.noises, x, y, z, 0.0)
                {
                    AquiferResult::Solid => CarveState::Skip,
                    AquiferResult::Fluid(id) => CarveState::Place(id),
                    AquiferResult::Air => CarveState::Place(self.ids.air),
                }
            }
            CarverStyle::Nether => {
                if y <= self.ctx.min_y + 31 {
                    CarveState::Place(self.ids.lava)
                } else {
                    CarveState::Place(self.ids.cave_air)
                }
            }
        }
    }
}

fn steep_material_condition(worldgen_surface: &Heightmap, block_x: i32, block_z: i32) -> bool {
    let local_x = (block_x & 15) as usize;
    let local_z = (block_z & 15) as usize;

    let z_north = local_z.saturating_sub(1);
    let z_south = (local_z + 1).min(15);
    let h_north = worldgen_surface.get_highest_taken(local_x, z_north);
    let h_south = worldgen_surface.get_highest_taken(local_x, z_south);
    if h_south >= h_north + 4 {
        return true;
    }

    let x_west = local_x.saturating_sub(1);
    let x_east = (local_x + 1).min(15);
    let h_west = worldgen_surface.get_highest_taken(x_west, local_z);
    let h_east = worldgen_surface.get_highest_taken(x_east, local_z);
    h_west >= h_east + 4
}

pub fn can_reach(
    chunk_min_x: i32,
    chunk_min_z: i32,
    x: f64,
    z: f64,
    current_step: i32,
    total_steps: i32,
    thickness: f32,
) -> bool {
    let x_mid = f64::from(chunk_min_x) + 8.0;
    let z_mid = f64::from(chunk_min_z) + 8.0;
    let xd = x - x_mid;
    let zd = z - z_mid;
    let remaining = f64::from(total_steps - current_step);
    let rr = f64::from(thickness + 2.0_f32 + 16.0_f32);
    xd * xd + zd * zd - remaining * remaining <= rr * rr
}

#[cfg(test)]
mod tests {
    use crate::chunk::heightmap::{Heightmap, HeightmapType};

    use super::steep_material_condition;

    fn flat_world_surface(highest_taken: i32) -> Heightmap {
        let mut heightmap = Heightmap::new(HeightmapType::WorldSurfaceWg, 0);
        for x in 0..16 {
            for z in 0..16 {
                heightmap.set_height(x, z, highest_taken + 1);
            }
        }
        heightmap
    }

    #[test]
    fn steep_material_condition_matches_vanilla_asymmetry() {
        let mut heightmap = flat_world_surface(63);
        heightmap.set_height(5, 4, 61);
        heightmap.set_height(5, 6, 65);
        assert!(steep_material_condition(&heightmap, 5, 5));

        let mut heightmap = flat_world_surface(63);
        heightmap.set_height(5, 4, 65);
        heightmap.set_height(5, 6, 61);
        assert!(!steep_material_condition(&heightmap, 5, 5));

        let mut heightmap = flat_world_surface(63);
        heightmap.set_height(4, 5, 65);
        heightmap.set_height(6, 5, 61);
        assert!(steep_material_condition(&heightmap, 5, 5));

        let mut heightmap = flat_world_surface(63);
        heightmap.set_height(4, 5, 61);
        heightmap.set_height(6, 5, 65);
        assert!(!steep_material_condition(&heightmap, 5, 5));
    }
}
