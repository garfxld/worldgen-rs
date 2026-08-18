pub mod acacia;
pub mod blob;
pub mod bush;
pub mod cherry;
pub mod dark_oak;
pub mod fancy;
pub mod jungle;
pub mod mega_pine;
pub mod pine;
pub mod random_spread;
pub mod spruce;

use serde::Deserialize;

use crate::registry::blocks::properties::Properties;
use crate::util::direction::Direction;
use crate::util::hash_set::JavaHashSet;
use crate::util::ident::BlockState;
use crate::util::math::floor_f32;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::util::value_providers::int::IntProvider;
use crate::worldgen::feature::features::tree::{TreeConfig, valid_tree_pos};
use crate::worldgen::region::WorldGenRegion;

use acacia::AcaciaFoliagePlacer;
use blob::BlobFoliagePlacer;
use bush::BushFoliagePlacer;
use cherry::CherryFoliagePlacer;
use dark_oak::DarkOakFoliagePlacer;
use fancy::FancyFoliagePlacer;
use jungle::JungleFoliagePlacer;
use mega_pine::MegaPineFoliagePlacer;
use pine::PineFoliagePlacer;
use random_spread::RandomSpreadFoliagePlacer;
use spruce::SpruceFoliagePlacer;

pub struct FoliageAttachment {
    pub pos: BlockPos,
    pub radius_offset: i32,
    pub double_trunk: bool,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum FoliagePlacer {
    #[serde(rename = "minecraft:dark_oak_foliage_placer")]
    DarkOak(DarkOakFoliagePlacer),
    #[serde(rename = "minecraft:blob_foliage_placer")]
    Blob(BlobFoliagePlacer),
    #[serde(rename = "minecraft:fancy_foliage_placer")]
    Fancy(FancyFoliagePlacer),
    #[serde(rename = "minecraft:acacia_foliage_placer")]
    Acacia(AcaciaFoliagePlacer),
    #[serde(rename = "minecraft:pine_foliage_placer")]
    Pine(PineFoliagePlacer),
    #[serde(rename = "minecraft:spruce_foliage_placer")]
    Spruce(SpruceFoliagePlacer),
    #[serde(rename = "minecraft:jungle_foliage_placer")]
    Jungle(JungleFoliagePlacer),
    #[serde(rename = "minecraft:cherry_foliage_placer")]
    Cherry(CherryFoliagePlacer),
    #[serde(rename = "minecraft:mega_pine_foliage_placer")]
    MegaPine(MegaPineFoliagePlacer),
    #[serde(rename = "minecraft:bush_foliage_placer")]
    Bush(BushFoliagePlacer),
    #[serde(rename = "minecraft:random_spread_foliage_placer")]
    RandomSpread(RandomSpreadFoliagePlacer),
}

impl FoliagePlacer {
    const fn offset_provider(&self) -> &IntProvider {
        match self {
            Self::DarkOak(DarkOakFoliagePlacer { offset, .. })
            | Self::Blob(BlobFoliagePlacer { offset, .. })
            | Self::Fancy(FancyFoliagePlacer { offset, .. })
            | Self::Acacia(AcaciaFoliagePlacer { offset, .. })
            | Self::Pine(PineFoliagePlacer { offset, .. })
            | Self::Spruce(SpruceFoliagePlacer { offset, .. })
            | Self::Jungle(JungleFoliagePlacer { offset, .. })
            | Self::Cherry(CherryFoliagePlacer { offset, .. })
            | Self::MegaPine(MegaPineFoliagePlacer { offset, .. })
            | Self::Bush(BushFoliagePlacer { offset, .. })
            | Self::RandomSpread(RandomSpreadFoliagePlacer { offset, .. }) => offset,
        }
    }

    pub fn foliage_height(&self, random: &mut RandomSource, tree_height: i32) -> i32 {
        match self {
            Self::DarkOak(_) => 4,
            Self::Blob(BlobFoliagePlacer { height, .. })
            | Self::Fancy(FancyFoliagePlacer { height, .. })
            | Self::Jungle(JungleFoliagePlacer { height, .. })
            | Self::Bush(BushFoliagePlacer { height, .. }) => *height,
            Self::Cherry(CherryFoliagePlacer { height, .. }) => height.sample(random),
            Self::RandomSpread(RandomSpreadFoliagePlacer { foliage_height, .. }) => {
                foliage_height.sample(random)
            }
            Self::Acacia(_) => 0,
            Self::Pine(PineFoliagePlacer { height, .. })
            | Self::MegaPine(MegaPineFoliagePlacer {
                crown_height: height,
                ..
            }) => height.sample(random),
            Self::Spruce(SpruceFoliagePlacer { trunk_height, .. }) => {
                (tree_height - trunk_height.sample(random)).max(4)
            }
        }
    }

    pub fn foliage_radius(&self, random: &mut RandomSource, trunk_height: i32) -> i32 {
        match self {
            Self::Pine(PineFoliagePlacer { radius, .. }) => {
                radius.sample(random) + random.next_i32_bounded((trunk_height + 1).max(1))
            }
            Self::DarkOak(DarkOakFoliagePlacer { radius, .. })
            | Self::Blob(BlobFoliagePlacer { radius, .. })
            | Self::Fancy(FancyFoliagePlacer { radius, .. })
            | Self::Acacia(AcaciaFoliagePlacer { radius, .. })
            | Self::Spruce(SpruceFoliagePlacer { radius, .. })
            | Self::Jungle(JungleFoliagePlacer { radius, .. })
            | Self::Cherry(CherryFoliagePlacer { radius, .. })
            | Self::MegaPine(MegaPineFoliagePlacer { radius, .. })
            | Self::Bush(BushFoliagePlacer { radius, .. })
            | Self::RandomSpread(RandomSpreadFoliagePlacer { radius, .. }) => radius.sample(random),
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors vanilla FoliagePlacer.createFoliage"
    )]
    pub fn create_foliage(
        &self,
        region: &WorldGenRegion<'_>,
        foliage_setter: &mut FoliageSetter,
        random: &mut RandomSource,
        config: &TreeConfig,
        _tree_height: i32,
        attachment: &FoliageAttachment,
        foliage_height: i32,
        leaf_radius: i32,
    ) {
        let offset = self.offset_provider().sample(random);
        match self {
            Self::DarkOak(_) => {
                let pos = attachment.pos.above_n(offset);
                let double_trunk = attachment.double_trunk;
                if double_trunk {
                    self.place_leaves_row(
                        region,
                        foliage_setter,
                        random,
                        config,
                        pos,
                        leaf_radius + 2,
                        -1,
                        double_trunk,
                    );
                    self.place_leaves_row(
                        region,
                        foliage_setter,
                        random,
                        config,
                        pos,
                        leaf_radius + 3,
                        0,
                        double_trunk,
                    );
                    self.place_leaves_row(
                        region,
                        foliage_setter,
                        random,
                        config,
                        pos,
                        leaf_radius + 2,
                        1,
                        double_trunk,
                    );
                    if random.next_bool() {
                        self.place_leaves_row(
                            region,
                            foliage_setter,
                            random,
                            config,
                            pos,
                            leaf_radius,
                            2,
                            double_trunk,
                        );
                    }
                } else {
                    self.place_leaves_row(
                        region,
                        foliage_setter,
                        random,
                        config,
                        pos,
                        leaf_radius + 2,
                        -1,
                        double_trunk,
                    );
                    self.place_leaves_row(
                        region,
                        foliage_setter,
                        random,
                        config,
                        pos,
                        leaf_radius + 1,
                        0,
                        double_trunk,
                    );
                }
            }
            Self::Blob(_) => {
                for yo in (offset - foliage_height..=offset).rev() {
                    let current_radius =
                        (leaf_radius + attachment.radius_offset - 1 - yo / 2).max(0);
                    self.place_leaves_row(
                        region,
                        foliage_setter,
                        random,
                        config,
                        attachment.pos,
                        current_radius,
                        yo,
                        attachment.double_trunk,
                    );
                }
            }
            Self::Fancy(_) => {
                for yo in (offset - foliage_height..=offset).rev() {
                    let current_radius =
                        leaf_radius + i32::from(yo != offset && yo != offset - foliage_height);
                    self.place_leaves_row(
                        region,
                        foliage_setter,
                        random,
                        config,
                        attachment.pos,
                        current_radius,
                        yo,
                        attachment.double_trunk,
                    );
                }
            }
            Self::Acacia(_) => {
                let double_trunk = attachment.double_trunk;
                let foliage_pos = attachment.pos.above_n(offset);
                for (current_radius, y) in [
                    (leaf_radius + attachment.radius_offset, -1 - foliage_height),
                    (leaf_radius - 1, -foliage_height),
                    (leaf_radius + attachment.radius_offset - 1, 0),
                ] {
                    self.place_leaves_row(
                        region,
                        foliage_setter,
                        random,
                        config,
                        foliage_pos,
                        current_radius,
                        y,
                        double_trunk,
                    );
                }
            }
            Self::Pine(_) => {
                let mut current_radius = 0;
                let y_start = offset;
                let y_end = offset - foliage_height;
                for yo in (y_end..=y_start).rev() {
                    self.place_leaves_row(
                        region,
                        foliage_setter,
                        random,
                        config,
                        attachment.pos,
                        current_radius,
                        yo,
                        attachment.double_trunk,
                    );
                    if current_radius >= 1 && yo == y_end + 1 {
                        current_radius -= 1;
                    } else if current_radius < leaf_radius + attachment.radius_offset {
                        current_radius += 1;
                    }
                }
            }
            Self::Spruce(_) => {
                let mut current_radius = random.next_i32_bounded(2);
                let mut max_radius = 1;
                let mut min_radius = 0;
                for yo in (-foliage_height..=offset).rev() {
                    self.place_leaves_row(
                        region,
                        foliage_setter,
                        random,
                        config,
                        attachment.pos,
                        current_radius,
                        yo,
                        attachment.double_trunk,
                    );
                    if current_radius >= max_radius {
                        current_radius = min_radius;
                        min_radius = 1;
                        max_radius = (max_radius + 1).min(leaf_radius + attachment.radius_offset);
                    } else {
                        current_radius += 1;
                    }
                }
            }
            Self::Jungle(_) => {
                let leaf_height = if attachment.double_trunk {
                    foliage_height
                } else {
                    1 + random.next_i32_bounded(2)
                };
                for yo in (offset - leaf_height..=offset).rev() {
                    let current_radius = leaf_radius + attachment.radius_offset + 1 - yo;
                    self.place_leaves_row(
                        region,
                        foliage_setter,
                        random,
                        config,
                        attachment.pos,
                        current_radius,
                        yo,
                        attachment.double_trunk,
                    );
                }
            }
            Self::Cherry(p) => {
                let double_trunk = attachment.double_trunk;
                let foliage_pos = attachment.pos.above_n(offset);
                let current_radius = leaf_radius + attachment.radius_offset - 1;
                self.place_leaves_row(
                    region,
                    foliage_setter,
                    random,
                    config,
                    foliage_pos,
                    current_radius - 2,
                    foliage_height - 3,
                    double_trunk,
                );
                self.place_leaves_row(
                    region,
                    foliage_setter,
                    random,
                    config,
                    foliage_pos,
                    current_radius - 1,
                    foliage_height - 4,
                    double_trunk,
                );
                for y in (0..=foliage_height - 5).rev() {
                    self.place_leaves_row(
                        region,
                        foliage_setter,
                        random,
                        config,
                        foliage_pos,
                        current_radius,
                        y,
                        double_trunk,
                    );
                }
                self.place_leaves_row_with_hanging_leaves_below(
                    region,
                    foliage_setter,
                    random,
                    config,
                    foliage_pos,
                    current_radius,
                    -1,
                    double_trunk,
                    p.hanging_leaves_chance,
                    p.hanging_leaves_extension_chance,
                );
                self.place_leaves_row_with_hanging_leaves_below(
                    region,
                    foliage_setter,
                    random,
                    config,
                    foliage_pos,
                    current_radius - 1,
                    -2,
                    double_trunk,
                    p.hanging_leaves_chance,
                    p.hanging_leaves_extension_chance,
                );
            }
            Self::MegaPine(_) => {
                let foliage_pos = attachment.pos;
                let mut prev_radius = 0;
                for yy in foliage_pos.y() - foliage_height + offset..=foliage_pos.y() + offset {
                    let yo = foliage_pos.y() - yy;
                    let smooth_radius = leaf_radius
                        + attachment.radius_offset
                        + floor_f32(yo as f32 / foliage_height as f32 * 3.5);
                    let jagged_radius = if yo > 0 && smooth_radius == prev_radius && (yy & 1) == 0 {
                        smooth_radius + 1
                    } else {
                        smooth_radius
                    };
                    self.place_leaves_row(
                        region,
                        foliage_setter,
                        random,
                        config,
                        foliage_pos.at_y(yy),
                        jagged_radius,
                        0,
                        attachment.double_trunk,
                    );
                    prev_radius = smooth_radius;
                }
            }
            Self::Bush(_) => {
                for yo in (offset - foliage_height..=offset).rev() {
                    let current_radius = leaf_radius + attachment.radius_offset - 1 - yo;
                    self.place_leaves_row(
                        region,
                        foliage_setter,
                        random,
                        config,
                        attachment.pos,
                        current_radius,
                        yo,
                        attachment.double_trunk,
                    );
                }
            }
            Self::RandomSpread(p) => {
                let origin = attachment.pos;
                for _ in 0..p.leaf_placement_attempts {
                    let pos = BlockPos::new(
                        origin.x() + random.next_i32_bounded(leaf_radius)
                            - random.next_i32_bounded(leaf_radius),
                        origin.y() + random.next_i32_bounded(foliage_height)
                            - random.next_i32_bounded(foliage_height),
                        origin.z() + random.next_i32_bounded(leaf_radius)
                            - random.next_i32_bounded(leaf_radius),
                    );
                    try_place_leaf(region, foliage_setter, random, config, pos);
                }
            }
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors vanilla FoliagePlacer.placeLeavesRow"
    )]
    fn place_leaves_row(
        &self,
        region: &WorldGenRegion<'_>,
        foliage_setter: &mut FoliageSetter,
        random: &mut RandomSource,
        config: &TreeConfig,
        origin: BlockPos,
        current_radius: i32,
        y: i32,
        double_trunk: bool,
    ) {
        let offset = if double_trunk { 1 } else { 0 };
        for dx in -current_radius..=current_radius + offset {
            for dz in -current_radius..=current_radius + offset {
                if !self.should_skip_location_signed(
                    random,
                    dx,
                    y,
                    dz,
                    current_radius,
                    double_trunk,
                ) {
                    let pos = origin.offset(dx, y, dz);
                    try_place_leaf(region, foliage_setter, random, config, pos);
                }
            }
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors vanilla FoliagePlacer.placeLeavesRowWithHangingLeavesBelow"
    )]
    fn place_leaves_row_with_hanging_leaves_below(
        &self,
        region: &WorldGenRegion<'_>,
        foliage_setter: &mut FoliageSetter,
        random: &mut RandomSource,
        config: &TreeConfig,
        origin: BlockPos,
        current_radius: i32,
        y: i32,
        double_trunk: bool,
        hanging_leaves_chance: f32,
        hanging_leaves_extension_chance: f32,
    ) {
        self.place_leaves_row(
            region,
            foliage_setter,
            random,
            config,
            origin,
            current_radius,
            y,
            double_trunk,
        );
        let offset = if double_trunk { 1 } else { 0 };
        let log_pos = origin.below();

        for along_edge in Direction::HORIZONTAL {
            let to_edge = along_edge.rotate_y_clockwise();
            let offset_to_edge = if matches!(to_edge, Direction::East | Direction::South) {
                current_radius + offset
            } else {
                current_radius
            };
            let mut pos = BlockPos::new(
                origin.x() + to_edge.offset().0 * offset_to_edge
                    - along_edge.offset().0 * current_radius,
                origin.y() + y - 1,
                origin.z() + to_edge.offset().2 * offset_to_edge
                    - along_edge.offset().2 * current_radius,
            );
            let mut offset_along_edge = -current_radius;

            while offset_along_edge < current_radius + offset {
                let leaves_above = foliage_setter.is_set(pos.above());
                if leaves_above
                    && try_place_extension(
                        region,
                        foliage_setter,
                        random,
                        config,
                        hanging_leaves_chance,
                        log_pos,
                        pos,
                    )
                {
                    try_place_extension(
                        region,
                        foliage_setter,
                        random,
                        config,
                        hanging_leaves_extension_chance,
                        log_pos,
                        pos.below(),
                    );
                }

                offset_along_edge += 1;
                pos = pos.offset(along_edge.offset().0, 0, along_edge.offset().2);
            }
        }
    }

    fn should_skip_location_signed(
        &self,
        random: &mut RandomSource,
        dx: i32,
        y: i32,
        dz: i32,
        current_radius: i32,
        double_trunk: bool,
    ) -> bool {
        if let Self::DarkOak(_) = self
            && y == 0
            && double_trunk
            && (dx == -current_radius || dx >= current_radius)
            && (dz == -current_radius || dz >= current_radius)
        {
            return true;
        }

        let (min_dx, min_dz) = if double_trunk {
            (dx.abs().min((dx - 1).abs()), dz.abs().min((dz - 1).abs()))
        } else {
            (dx.abs(), dz.abs())
        };
        self.should_skip_location(random, min_dx, y, min_dz, current_radius, double_trunk)
    }

    fn should_skip_location(
        &self,
        random: &mut RandomSource,
        dx: i32,
        y: i32,
        dz: i32,
        current_radius: i32,
        double_trunk: bool,
    ) -> bool {
        match self {
            Self::DarkOak(p) => {
                p.should_skip_location(random, dx, y, dz, current_radius, double_trunk)
            }
            Self::Blob(p) => {
                p.should_skip_location(random, dx, y, dz, current_radius, double_trunk)
            }
            Self::Fancy(p) => {
                p.should_skip_location(random, dx, y, dz, current_radius, double_trunk)
            }
            Self::Acacia(p) => {
                p.should_skip_location(random, dx, y, dz, current_radius, double_trunk)
            }
            Self::Pine(p) => {
                p.should_skip_location(random, dx, y, dz, current_radius, double_trunk)
            }
            Self::Spruce(p) => {
                p.should_skip_location(random, dx, y, dz, current_radius, double_trunk)
            }
            Self::Jungle(p) => {
                p.should_skip_location(random, dx, y, dz, current_radius, double_trunk)
            }
            Self::Cherry(p) => {
                p.should_skip_location(random, dx, y, dz, current_radius, double_trunk)
            }
            Self::MegaPine(p) => {
                p.should_skip_location(random, dx, y, dz, current_radius, double_trunk)
            }
            Self::Bush(p) => {
                p.should_skip_location(random, dx, y, dz, current_radius, double_trunk)
            }
            Self::RandomSpread(p) => {
                p.should_skip_location(random, dx, y, dz, current_radius, double_trunk)
            }
        }
    }
}

pub struct FoliageSetter {
    pub positions: JavaHashSet,
}

impl FoliageSetter {
    pub fn new() -> FoliageSetter {
        FoliageSetter {
            positions: JavaHashSet::new(),
        }
    }

    pub fn set(&mut self, region: &WorldGenRegion<'_>, pos: BlockPos, state: BlockState) {
        self.positions.insert(pos);
        region.set_block(pos, state);
    }

    pub fn is_set(&self, pos: BlockPos) -> bool {
        self.positions.contains(pos)
    }
}

impl Default for FoliageSetter {
    fn default() -> Self {
        FoliageSetter::new()
    }
}

fn try_place_extension(
    region: &WorldGenRegion<'_>,
    foliage_setter: &mut FoliageSetter,
    random: &mut RandomSource,
    config: &TreeConfig,
    chance: f32,
    log_pos: BlockPos,
    pos: BlockPos,
) -> bool {
    let distance = (pos.x() - log_pos.x()).abs()
        + (pos.y() - log_pos.y()).abs()
        + (pos.z() - log_pos.z()).abs();
    #[expect(
        clippy::if_same_then_else,
        reason = "branches match but the second short-circuits an RNG draw vanilla only takes when the distance check passes"
    )]
    if distance >= 7 {
        false
    } else if random.next_f32() > chance {
        false
    } else {
        try_place_leaf(region, foliage_setter, random, config, pos)
    }
}

fn try_place_leaf(
    region: &WorldGenRegion<'_>,
    foliage_setter: &mut FoliageSetter,
    random: &mut RandomSource,
    config: &TreeConfig,
    pos: BlockPos,
) -> bool {
    let is_persistent = region.get_block(pos).try_get_value(&Properties::PERSISTENT) == Some(true);
    if !is_persistent && valid_tree_pos(region, pos) {
        let mut state = config.foliage_provider.get_state(region, random, pos);
        if state.try_get_value(&Properties::WATERLOGGED).is_some() {
            let fluid = region.get_block(pos).get_fluid_state();
            let waterlogged = fluid.is_source() && fluid.is_water();
            state = state.set_value(&Properties::WATERLOGGED, waterlogged);
        }
        foliage_setter.set(region, pos, state);
        true
    } else {
        false
    }
}
