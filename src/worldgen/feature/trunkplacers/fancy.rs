use std::f64::consts::PI;

use serde::Deserialize;

use crate::util::ident::BlockState;
use crate::util::math;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::worldgen::feature::features::tree::TreeConfig;
use crate::worldgen::feature::foliageplacers::FoliageAttachment;
use crate::worldgen::region::WorldGenRegion;

#[derive(Debug, Deserialize)]
pub struct FancyTrunkPlacer {
    pub base_height: i32,
    pub height_rand_a: i32,
    pub height_rand_b: i32,
}

impl FancyTrunkPlacer {
    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors vanilla FancyTrunkPlacer.placeTrunk"
    )]
    pub fn place_trunk(
        &self,
        placer: &super::TrunkPlacer,
        region: &WorldGenRegion<'_>,
        trunk_setter: &mut dyn FnMut(BlockPos, BlockState),
        random: &mut RandomSource,
        tree_height: i32,
        origin: BlockPos,
        config: &TreeConfig,
    ) -> Vec<FoliageAttachment> {
        let height = tree_height + 2;
        let trunk_height = math::fast_floor(f64::from(height) * 0.618);
        super::place_below_trunk_block(region, trunk_setter, random, origin.below(), config);
        let clusters_per_y = 1.min(math::fast_floor(
            1.382 + (f64::from(height) / 13.0).powf(2.0),
        ));
        let trunk_top = origin.y() + trunk_height;
        let mut foliage_coords: Vec<(BlockPos, i32)> =
            vec![(origin.above_n(height - 5), trunk_top)];

        for relative_y in (0..=height - 5).rev() {
            let shape = super::tree_shape(height, relative_y);
            if shape < 0.0 {
                continue;
            }
            for _ in 0..clusters_per_y {
                let radius = f64::from(shape) * (f64::from(random.next_f32()) + 0.328);
                let angle = f64::from(random.next_f32() * 2.0) * PI;
                let x = radius * angle.sin() + 0.5;
                let z = radius * angle.cos() + 0.5;
                let check_start = BlockPos::new(
                    origin.x() + math::fast_floor(x),
                    origin.y() + relative_y - 1,
                    origin.z() + math::fast_floor(z),
                );
                let check_end = check_start.above_n(5);
                if !super::make_limb(
                    placer,
                    region,
                    trunk_setter,
                    random,
                    check_start,
                    check_end,
                    false,
                    config,
                ) {
                    continue;
                }
                let dx = origin.x() - check_start.x();
                let dz = origin.z() - check_start.z();
                let branch_height =
                    f64::from(check_start.y()) - (f64::from(dx * dx + dz * dz)).sqrt() * 0.381;
                let branch_top = if branch_height > f64::from(trunk_top) {
                    trunk_top
                } else {
                    branch_height as i32
                };
                let check_branch_base = origin.at_y(branch_top);
                if super::make_limb(
                    placer,
                    region,
                    trunk_setter,
                    random,
                    check_branch_base,
                    check_start,
                    false,
                    config,
                ) {
                    foliage_coords.push((check_start, check_branch_base.y()));
                }
            }
        }

        super::make_limb(
            placer,
            region,
            trunk_setter,
            random,
            origin,
            origin.above_n(trunk_height),
            true,
            config,
        );

        for &(pos, branch_base) in &foliage_coords {
            let base_coord = origin.at_y(branch_base);
            if base_coord != pos && super::trim_branches(height, branch_base - origin.y()) {
                super::make_limb(
                    placer,
                    region,
                    trunk_setter,
                    random,
                    base_coord,
                    pos,
                    true,
                    config,
                );
            }
        }

        foliage_coords
            .iter()
            .filter(|(_, branch_base)| super::trim_branches(height, branch_base - origin.y()))
            .map(|&(pos, _)| FoliageAttachment {
                pos,
                radius_offset: 0,
                double_trunk: false,
            })
            .collect()
    }
}
