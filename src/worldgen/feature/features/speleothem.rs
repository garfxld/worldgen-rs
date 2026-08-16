use std::f32::consts::TAU;

use crate::registry::blocks::BlockRef;
use crate::registry::blocks::properties::{Properties, SpeleothemThickness};
use crate::registry::blocks::vanilla_blocks;
use crate::registry::fluid::is_water_fluid;
use crate::util::direction::Direction;
use crate::util::ident::BlockState;
use crate::util::math;
use crate::util::pos::BlockPos;
use crate::worldgen::block_predicate::HolderSet;
use crate::worldgen::region::WorldGenRegion;

pub fn is_empty_or_water(state: BlockState) -> bool {
    state.is_air() || state.get_block() == &vanilla_blocks::WATER
}

pub fn is_neither_empty_nor_water(state: BlockState) -> bool {
    !state.is_air() && state.get_block() != &vanilla_blocks::WATER
}

pub fn is_base(state: BlockState, base_block: BlockRef, replaceable_blocks: &HolderSet) -> bool {
    state.get_block() == base_block || replaceable_blocks.contains_block(state)
}

pub fn is_base_or_lava(
    state: BlockState,
    base_block: BlockRef,
    replaceable_blocks: &HolderSet,
) -> bool {
    is_base(state, base_block, replaceable_blocks) || state.get_block() == &vanilla_blocks::LAVA
}

pub fn is_empty_or_water_or_lava(state: BlockState) -> bool {
    state.is_air()
        || state.get_block() == &vanilla_blocks::WATER
        || state.get_block() == &vanilla_blocks::LAVA
}

pub fn get_speleothem_height(
    mut xz_distance_from_center: f64,
    speleothem_radius: f64,
    scale: f64,
    bluntness: f64,
) -> f64 {
    if xz_distance_from_center < bluntness {
        xz_distance_from_center = bluntness;
    }

    const CUTOFF: f64 = 0.384;

    let r = xz_distance_from_center / speleothem_radius * CUTOFF;
    let part1 = 0.75 * r.powf(1.333_333_333_333_333_3);
    let part2 = r.powf(0.666_666_666_666_666_6);
    let part3 = 0.333_333_333_333_333_3 * r.ln();
    let height_relative_to_max_radius = (scale * (part1 - part2 - part3)).max(0.0);

    height_relative_to_max_radius / CUTOFF * speleothem_radius
}

pub fn is_circle_mostly_embedded_in_stone(
    region: &WorldGenRegion<'_>,
    center: BlockPos,
    xz_radius: i32,
) -> bool {
    if is_empty_or_water_or_lava(region.get_block(center)) {
        return false;
    }

    let angle_increment = 6.0 / xz_radius as f32;
    let mut angle = 0.0;
    while angle < TAU {
        let dx = (math::cos(angle) * xz_radius as f32) as i32;
        let dz = (math::sin(angle) * xz_radius as f32) as i32;
        if is_empty_or_water_or_lava(region.get_block(center.offset(dx, 0, dz))) {
            return false;
        }
        angle += angle_increment;
    }

    true
}

fn create_pointed_block(
    direction: Direction,
    thickness: SpeleothemThickness,
    pointed_block: BlockRef,
) -> BlockState {
    pointed_block
        .default_state()
        .set_value(&Properties::VERTICAL_DIRECTION, direction)
        .set_value(&Properties::SPELEOTHEM_THICKNESS, thickness)
}

pub fn build_base_to_tip_column(
    direction: Direction,
    total_length: i32,
    merged_tip: bool,
    pointed_block: BlockRef,
    mut consumer: impl FnMut(BlockState),
) {
    if total_length >= 3 {
        consumer(create_pointed_block(
            direction,
            SpeleothemThickness::Base,
            pointed_block,
        ));

        for _ in 0..total_length - 3 {
            consumer(create_pointed_block(
                direction,
                SpeleothemThickness::Middle,
                pointed_block,
            ));
        }
    }

    if total_length >= 2 {
        consumer(create_pointed_block(
            direction,
            SpeleothemThickness::Frustum,
            pointed_block,
        ));
    }

    if total_length >= 1 {
        consumer(create_pointed_block(
            direction,
            if merged_tip {
                SpeleothemThickness::TipMerge
            } else {
                SpeleothemThickness::Tip
            },
            pointed_block,
        ));
    }
}

#[expect(clippy::too_many_arguments, reason = "vanilla parameter list")]
pub fn grow_speleothem(
    region: &WorldGenRegion<'_>,
    start_pos: BlockPos,
    tip_direction: Direction,
    height: i32,
    merged_tip: bool,
    base_block: BlockRef,
    pointed_block: BlockRef,
    replaceable_blocks: &HolderSet,
) {
    let check_pos = tip_direction.opposite().relative(start_pos);
    if !is_base(region.get_block(check_pos), base_block, replaceable_blocks) {
        return;
    }

    let mut pos = start_pos;
    build_base_to_tip_column(tip_direction, height, merged_tip, pointed_block, |state| {
        let state = if state.get_block() == pointed_block {
            let waterlogged = is_water_fluid(region.get_block(pos).get_fluid_state().fluid_id);
            state.set_value(&Properties::WATERLOGGED, waterlogged)
        } else {
            state
        };

        region.set_block(pos, state);
        pos = tip_direction.relative(pos);
    });
}

pub fn place_base_block_if_possible(
    region: &WorldGenRegion<'_>,
    pos: BlockPos,
    base_block: BlockRef,
    replaceable_blocks: &HolderSet,
) -> bool {
    if replaceable_blocks.contains_block(region.get_block(pos)) {
        region.set_block(pos, base_block.default_state());
        true
    } else {
        false
    }
}
