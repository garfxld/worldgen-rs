use serde::Deserialize;

use crate::util::rng::RandomSource;
use crate::util::value_providers::int::IntProvider;
use crate::world::worldgen::feature::features::tree::TreeConfig;
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util::Axis;
use shulkr::util::BlockPosition;
use shulkr::world::block::Properties;

use super::{FoliageAttachment, FoliageSetter, try_place_leaf};

#[derive(Debug, Deserialize)]
pub struct PoplarFoliagePlacer {
    pub radius: IntProvider,
    pub offset: IntProvider,
    pub height: IntProvider,
    pub side_hole_chance: f32,
}

impl PoplarFoliagePlacer {
    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors vanilla PoplarFoliagePlacer.createFoliage"
    )]
    pub fn create_foliage(
        &self,
        region: &WorldGenRegion<'_>,
        foliage_setter: &mut FoliageSetter,
        random: &mut RandomSource,
        config: &TreeConfig,
        attachment: &FoliageAttachment,
        foliage_height: i32,
        leaf_radius: i32,
        offset: i32,
    ) {
        let double_trunk = attachment.double_trunk;
        let foliage_pos = attachment.pos.above_n(offset);
        let current_radius = leaf_radius + attachment.radius_offset - 1;
        let flip = random.next_bool();
        let row =
            |random: &mut RandomSource, foliage_setter: &mut FoliageSetter, radius: i32, y: i32| {
                self.place_leaves_row(
                    region,
                    foliage_setter,
                    random,
                    config,
                    foliage_pos,
                    radius,
                    y,
                    double_trunk,
                    foliage_height,
                    flip,
                );
            };
        row(
            random,
            foliage_setter,
            current_radius - 2,
            foliage_height - 1,
        );
        row(
            random,
            foliage_setter,
            current_radius - 1,
            foliage_height - 2,
        );
        row(
            random,
            foliage_setter,
            current_radius - 1,
            foliage_height - 3,
        );
        for y in (1..=foliage_height - 4).rev() {
            row(random, foliage_setter, current_radius, y);
        }
        self.replace_leaves_with_log(
            region,
            foliage_setter,
            config,
            random,
            foliage_pos,
            current_radius,
            foliage_height - 4,
            double_trunk,
            foliage_height,
            flip,
        );
        row(random, foliage_setter, current_radius - 1, 0);
        row(random, foliage_setter, (current_radius - 2).clamp(1, 2), -1);
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors vanilla PoplarFoliagePlacer.replaceLeavesWithLog"
    )]
    fn replace_leaves_with_log(
        &self,
        region: &WorldGenRegion<'_>,
        foliage_setter: &mut FoliageSetter,
        config: &TreeConfig,
        random: &mut RandomSource,
        origin: BlockPosition,
        current_radius: i32,
        y: i32,
        double_trunk: bool,
        foliage_height: i32,
        flip: bool,
    ) {
        let offset = i32::from(double_trunk);
        for dx in -current_radius..=current_radius + offset {
            for dz in -current_radius..=current_radius + offset {
                let abs_dz = dz.abs();
                let abs_dx = dx.abs();
                let partial = should_row_be_partial_rhombus_shape(foliage_height, y);
                if is_within_rhombus_shape(
                    current_radius,
                    abs_dx,
                    abs_dz,
                    corner_blocks_to_cut(dx, dz, current_radius, partial, flip),
                    2,
                ) && ((abs_dz == 0 && current_radius - abs_dx >= 4)
                    || (abs_dx == 0 && current_radius - abs_dz >= 4))
                {
                    let pos = origin.offset(dx, y, dz);
                    let axis = if abs_dz == 0 { Axis::X } else { Axis::Z };
                    let foliage = config.foliage_provider.get_state(region, random, pos);
                    if region.get_block(pos) == foliage {
                        let mut state = config.trunk_provider.get_state(region, random, pos);
                        if state.get(Properties::AXIS).is_some() {
                            state = state.with(Properties::AXIS, axis);
                        }
                        foliage_setter.set(region, pos, state);
                    }
                }
            }
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors vanilla PoplarFoliagePlacer.placeLeavesRow"
    )]
    fn place_leaves_row(
        &self,
        region: &WorldGenRegion<'_>,
        foliage_setter: &mut FoliageSetter,
        random: &mut RandomSource,
        config: &TreeConfig,
        origin: BlockPosition,
        current_radius: i32,
        y: i32,
        double_trunk: bool,
        foliage_height: i32,
        flip: bool,
    ) {
        let offset = i32::from(double_trunk);
        for dx in -current_radius..=current_radius + offset {
            for dz in -current_radius..=current_radius + offset {
                if !self.should_skip_location(
                    random,
                    dx,
                    y,
                    dz,
                    current_radius,
                    foliage_height,
                    flip,
                ) {
                    try_place_leaf(
                        region,
                        foliage_setter,
                        random,
                        config,
                        origin.offset(dx, y, dz),
                    );
                }
            }
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors vanilla PoplarFoliagePlacer.shouldSkipLocation"
    )]
    fn should_skip_location(
        &self,
        random: &mut RandomSource,
        dx: i32,
        y: i32,
        dz: i32,
        current_radius: i32,
        foliage_height: i32,
        flip: bool,
    ) -> bool {
        let partial = should_row_be_partial_rhombus_shape(foliage_height, y);
        let corner = corner_blocks_to_cut(dx, dz, current_radius, partial, flip);
        let abs_dx = dx.abs();
        let abs_dz = dz.abs();
        let is_rhombus_edge_block = abs_dx == current_radius || abs_dz == current_radius;
        if partial && is_rhombus_edge_block {
            return true;
        }
        let additional_side_removal = i32::from(random.next_f32() <= self.side_hole_chance);
        !is_within_rhombus_shape(
            current_radius,
            abs_dx,
            abs_dz,
            corner,
            additional_side_removal,
        )
    }
}

const fn corner_blocks_to_cut(
    dx: i32,
    dz: i32,
    current_radius: i32,
    partial: bool,
    flip: bool,
) -> i32 {
    let small_corner = if flip {
        (dx > 0 && dz > 0) || (dz < 0 && dx < 0)
    } else {
        (dx > 0 && dz < 0) || (dz > 0 && dx < 0)
    };
    if small_corner {
        current_radius - 1
    } else if partial {
        current_radius + 1
    } else {
        current_radius
    }
}

const fn is_within_rhombus_shape(
    current_radius: i32,
    abs_dx: i32,
    abs_dz: i32,
    corner_blocks_to_cut: i32,
    additional_side_removal: i32,
) -> bool {
    abs_dx + abs_dz <= current_radius * 2 - (corner_blocks_to_cut + additional_side_removal)
}

const fn should_row_be_partial_rhombus_shape(foliage_height: i32, y: i32) -> bool {
    foliage_height - 1 == y || foliage_height - 2 == y
}
