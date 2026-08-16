pub mod bending;
pub mod cherry;
pub mod dark_oak;
pub mod fancy;
pub mod forking;
pub mod giant;
pub mod mega_jungle;
pub mod straight;
pub mod upwards_branching;

use serde::Deserialize;

use crate::registry::REGISTRY;
use crate::registry::blocks::properties::Properties;
use crate::util::axis::Axis;
use crate::util::ident::{BlockState, Identifier};
use crate::util::math;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::worldgen::feature::features::tree::{valid_tree_pos, TreeConfig};
use crate::worldgen::feature::foliageplacers::FoliageAttachment;
use crate::worldgen::region::WorldGenRegion;

use bending::BendingTrunkPlacer;
use cherry::CherryTrunkPlacer;
use dark_oak::DarkOakTrunkPlacer;
use fancy::FancyTrunkPlacer;
use forking::ForkingTrunkPlacer;
use giant::GiantTrunkPlacer;
use mega_jungle::MegaJungleTrunkPlacer;
use straight::StraightTrunkPlacer;
use upwards_branching::UpwardsBranchingTrunkPlacer;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum TrunkPlacer {
    #[serde(rename = "minecraft:dark_oak_trunk_placer")]
    DarkOak(DarkOakTrunkPlacer),
    #[serde(rename = "minecraft:straight_trunk_placer")]
    Straight(StraightTrunkPlacer),
    #[serde(rename = "minecraft:fancy_trunk_placer")]
    Fancy(FancyTrunkPlacer),
    #[serde(rename = "minecraft:giant_trunk_placer")]
    Giant(GiantTrunkPlacer),
    #[serde(rename = "minecraft:mega_jungle_trunk_placer")]
    MegaJungle(MegaJungleTrunkPlacer),
    #[serde(rename = "minecraft:forking_trunk_placer")]
    Forking(ForkingTrunkPlacer),
    #[serde(rename = "minecraft:bending_trunk_placer")]
    Bending(BendingTrunkPlacer),
    #[serde(rename = "minecraft:upwards_branching_trunk_placer")]
    UpwardsBranching(UpwardsBranchingTrunkPlacer),
    #[serde(rename = "minecraft:cherry_trunk_placer")]
    Cherry(CherryTrunkPlacer),
}

impl TrunkPlacer {
    pub fn get_tree_height(&self, random: &mut RandomSource) -> i32 {
        let (base_height, height_rand_a, height_rand_b) = match self {
            Self::DarkOak(DarkOakTrunkPlacer {
                base_height,
                height_rand_a,
                height_rand_b,
            })
            | Self::Straight(StraightTrunkPlacer {
                base_height,
                height_rand_a,
                height_rand_b,
            })
            | Self::Fancy(FancyTrunkPlacer {
                base_height,
                height_rand_a,
                height_rand_b,
            })
            | Self::Giant(GiantTrunkPlacer {
                base_height,
                height_rand_a,
                height_rand_b,
            })
            | Self::Forking(ForkingTrunkPlacer {
                base_height,
                height_rand_a,
                height_rand_b,
            }) => (*base_height, *height_rand_a, *height_rand_b),
            Self::MegaJungle(MegaJungleTrunkPlacer { giant: p }) => {
                (p.base_height, p.height_rand_a, p.height_rand_b)
            }
            Self::Bending(BendingTrunkPlacer {
                base_height,
                height_rand_a,
                height_rand_b,
                ..
            })
            | Self::UpwardsBranching(UpwardsBranchingTrunkPlacer {
                base_height,
                height_rand_a,
                height_rand_b,
                ..
            })
            | Self::Cherry(CherryTrunkPlacer {
                base_height,
                height_rand_a,
                height_rand_b,
                ..
            }) => (*base_height, *height_rand_a, *height_rand_b),
        };
        base_height
            + random.next_i32_bounded(height_rand_a + 1)
            + random.next_i32_bounded(height_rand_b + 1)
    }

    pub fn place_trunk(
        &self,
        region: &WorldGenRegion<'_>,
        trunk_setter: &mut dyn FnMut(BlockPos, BlockState),
        random: &mut RandomSource,
        tree_height: i32,
        origin: BlockPos,
        config: &TreeConfig,
    ) -> Vec<FoliageAttachment> {
        match self {
            Self::Straight(p) => p.place_trunk(self, region, trunk_setter, random, tree_height, origin, config),
            Self::Fancy(p) => p.place_trunk(self, region, trunk_setter, random, tree_height, origin, config),
            Self::DarkOak(p) => p.place_trunk(self, region, trunk_setter, random, tree_height, origin, config),
            Self::Giant(p) => p.place_trunk(self, region, trunk_setter, random, tree_height, origin, config),
            Self::MegaJungle(p) => p.place_trunk(self, region, trunk_setter, random, tree_height, origin, config),
            Self::Forking(p) => p.place_trunk(self, region, trunk_setter, random, tree_height, origin, config),
            Self::Bending(p) => p.place_trunk(self, region, trunk_setter, random, tree_height, origin, config),
            Self::UpwardsBranching(p) => p.place_trunk(self, region, trunk_setter, random, tree_height, origin, config),
            Self::Cherry(p) => p.place_trunk(self, region, trunk_setter, random, tree_height, origin, config),
        }
    }

    pub fn valid_tree_pos(&self, region: &WorldGenRegion<'_>, pos: BlockPos) -> bool {
        match self {
            Self::UpwardsBranching(p) => {
                valid_tree_pos(region, pos) || p.can_grow_through.contains_block(region.get_block(pos))
            }
            _ => valid_tree_pos(region, pos),
        }
    }

    pub fn is_free(&self, region: &WorldGenRegion<'_>, pos: BlockPos) -> bool {
        const LOGS: Identifier = Identifier::const_vanilla("logs");
        self.valid_tree_pos(region, pos)
            || REGISTRY
                .blocks
                .is_in_tag(&region.get_block(pos).get_block().key, &LOGS)
    }

    fn place_log(
        &self,
        region: &WorldGenRegion<'_>,
        trunk_setter: &mut dyn FnMut(BlockPos, BlockState),
        random: &mut RandomSource,
        pos: BlockPos,
        config: &TreeConfig,
    ) -> bool {
        self.place_log_with(region, trunk_setter, random, pos, config, |state| state)
    }

    fn place_log_with(
        &self,
        region: &WorldGenRegion<'_>,
        trunk_setter: &mut dyn FnMut(BlockPos, BlockState),
        random: &mut RandomSource,
        pos: BlockPos,
        config: &TreeConfig,
        state_modifier: impl FnOnce(BlockState) -> BlockState,
    ) -> bool {
        if self.valid_tree_pos(region, pos) {
            let state = config.trunk_provider.get_state(region, random, pos);
            trunk_setter(pos, state_modifier(state));
            true
        } else {
            false
        }
    }

    fn place_log_if_free(
        &self,
        region: &WorldGenRegion<'_>,
        trunk_setter: &mut dyn FnMut(BlockPos, BlockState),
        random: &mut RandomSource,
        pos: BlockPos,
        config: &TreeConfig,
    ) {
        if self.is_free(region, pos) {
            self.place_log(region, trunk_setter, random, pos, config);
        }
    }
}

#[expect(clippy::too_many_arguments, reason = "mirrors vanilla FancyTrunkPlacer.makeLimb")]
fn make_limb(
    placer: &TrunkPlacer,
    region: &WorldGenRegion<'_>,
    trunk_setter: &mut dyn FnMut(BlockPos, BlockState),
    random: &mut RandomSource,
    start_pos: BlockPos,
    end_pos: BlockPos,
    do_place: bool,
    config: &TreeConfig,
) -> bool {
    if !do_place && start_pos == end_pos {
        return true;
    }

    let dx = end_pos.x() - start_pos.x();
    let dy = end_pos.y() - start_pos.y();
    let dz = end_pos.z() - start_pos.z();
    let steps = dx.abs().max(dy.abs()).max(dz.abs());
    let sx = dx as f32 / steps as f32;
    let sy = dy as f32 / steps as f32;
    let sz = dz as f32 / steps as f32;

    for i in 0..=steps {
        let pos = BlockPos::new(
            start_pos.x() + math::floor_f32(0.5 + i as f32 * sx),
            start_pos.y() + math::floor_f32(0.5 + i as f32 * sy),
            start_pos.z() + math::floor_f32(0.5 + i as f32 * sz),
        );
        if do_place {
            let axis = log_axis(start_pos, pos);
            placer.place_log_with(region, trunk_setter, random, pos, config, |state| {
                set_axis(state, axis)
            });
        } else if !placer.is_free(region, pos) {
            return false;
        }
    }

    true
}

const fn log_axis(start_pos: BlockPos, pos: BlockPos) -> Axis {
    let xdiff = (pos.x() - start_pos.x()).abs();
    let zdiff = (pos.z() - start_pos.z()).abs();
    let maxdiff = if xdiff > zdiff { xdiff } else { zdiff };
    if maxdiff > 0 {
        if xdiff == maxdiff { Axis::X } else { Axis::Z }
    } else {
        Axis::Y
    }
}

fn set_axis(state: BlockState, axis: Axis) -> BlockState {
    if state.try_get_value(&Properties::AXIS).is_some() {
        state.set_value(&Properties::AXIS, axis)
    } else {
        state
    }
}

fn trim_branches(height: i32, local_y: i32) -> bool {
    f64::from(local_y) >= f64::from(height) * 0.2
}

fn tree_shape(height: i32, y: i32) -> f32 {
    if (y as f32) < height as f32 * 0.3 {
        return -1.0;
    }

    let radius = height as f32 / 2.0;
    let adjacent = radius - y as f32;
    let mut distance = (radius * radius - adjacent * adjacent).sqrt();
    if adjacent == 0.0 {
        distance = radius;
    } else if adjacent.abs() >= radius {
        return 0.0;
    }

    distance * 0.5
}

fn place_below_trunk_block(
    region: &WorldGenRegion<'_>,
    trunk_setter: &mut dyn FnMut(BlockPos, BlockState),
    random: &mut RandomSource,
    pos: BlockPos,
    config: &TreeConfig,
) {
    if let Some(state) = config.below_trunk_provider.get_optional_state(region, random, pos) {
        trunk_setter(pos, state);
    }
}
