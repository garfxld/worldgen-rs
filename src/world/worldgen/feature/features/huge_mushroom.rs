use serde::Deserialize;
use shulkr::util::Key;

use shulkr::world::block::{BoolProperty, Properties};

use crate::util::rng::RandomSource;
use crate::world::worldgen::block_predicate::BlockPredicate;
use crate::world::worldgen::feature::state_provider::ProviderHolder;
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util::BlockPosition;
use shulkr::world::block::BlockState;

const LEAVES: Key = Key::const_vanilla("leaves");
const REPLACEABLE_BY_MUSHROOMS: Key = Key::const_vanilla("replaceable_by_mushrooms");

#[derive(Debug, Deserialize)]
pub struct HugeMushroomConfig {
    pub can_place_on: BlockPredicate,
    pub cap_provider: ProviderHolder,
    pub stem_provider: ProviderHolder,
    #[serde(default = "default_foliage_radius")]
    pub foliage_radius: i32,
}

const fn default_foliage_radius() -> i32 {
    2
}

#[derive(Debug, Clone, Copy)]
pub enum MushroomVariant {
    Red,
    Brown,
}

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPosition,
    config: &HugeMushroomConfig,
    variant: MushroomVariant,
) -> bool {
    let mut height = 4 + random.next_i32_bounded(3);
    if random.next_i32_bounded(12) == 0 {
        height *= 2;
    }

    if !is_valid_position(region, origin, height, config, variant) {
        return false;
    }

    make_cap(region, random, origin, height, config, variant);
    place_trunk(region, random, origin, height, config);
    true
}

const fn tree_radius_for_height(
    variant: MushroomVariant,
    tree_height: i32,
    leaf_radius: i32,
    yo: i32,
) -> i32 {
    match variant {
        MushroomVariant::Brown => {
            if yo <= 3 {
                0
            } else {
                leaf_radius
            }
        }
        MushroomVariant::Red => {
            if yo >= tree_height - 3 && yo <= tree_height {
                leaf_radius
            } else {
                0
            }
        }
    }
}

fn is_valid_position(
    region: &WorldGenRegion<'_>,
    origin: BlockPosition,
    height: i32,
    config: &HugeMushroomConfig,
    variant: MushroomVariant,
) -> bool {
    let y = origin.y();
    if y < region.min_y() + 1 || y + height + 1 > region.max_y() - 1 {
        return false;
    }
    if !config.can_place_on.test(region, origin.offset(0, -1, 0)) {
        return false;
    }
    for dy in 0..=height {
        let radius = tree_radius_for_height(variant, -1, config.foliage_radius, dy);
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                let state = region.get_block(origin.offset(dx, dy, dz));
                if !state.is_air() && !state.as_block().has_tag(&LEAVES) {
                    return false;
                }
            }
        }
    }
    true
}

fn place_mushroom_block(region: &WorldGenRegion<'_>, pos: BlockPosition, state: BlockState) {
    let current = region.get_block(pos);
    if current.is_air() || current.as_block().has_tag(&REPLACEABLE_BY_MUSHROOMS) {
        region.set_block(pos, state);
    }
}

fn place_trunk(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPosition,
    height: i32,
    config: &HugeMushroomConfig,
) {
    for dy in 0..height {
        let pos = origin.offset(0, dy, 0);
        let state = config.stem_provider.get_state(region, random, origin);
        place_mushroom_block(region, pos, state);
    }
}

const HAS_ALL_FACES: [BoolProperty; 5] = [
    Properties::WEST,
    Properties::EAST,
    Properties::NORTH,
    Properties::SOUTH,
    Properties::UP,
];

fn make_cap(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPosition,
    height: i32,
    config: &HugeMushroomConfig,
    variant: MushroomVariant,
) {
    match variant {
        MushroomVariant::Red => {
            for dy in (height - 3)..=height {
                let radius = if dy < height {
                    config.foliage_radius
                } else {
                    config.foliage_radius - 1
                };
                let center = config.foliage_radius - 2;
                for dx in -radius..=radius {
                    for dz in -radius..=radius {
                        let min_x = dx == -radius;
                        let max_x = dx == radius;
                        let min_z = dz == -radius;
                        let max_z = dz == radius;
                        let x_edge = min_x || max_x;
                        let z_edge = min_z || max_z;
                        if dy < height && x_edge == z_edge {
                            continue;
                        }
                        let pos = origin.offset(dx, dy, dz);
                        let mut state = config.cap_provider.get_state(region, random, origin);
                        if HAS_ALL_FACES.iter().all(|p| state.get(*p).is_some()) {
                            state = state
                                .with(Properties::UP, dy >= height - 1)
                                .with(Properties::WEST, dx < -center)
                                .with(Properties::EAST, dx > center)
                                .with(Properties::NORTH, dz < -center)
                                .with(Properties::SOUTH, dz > center);
                        }
                        place_mushroom_block(region, pos, state);
                    }
                }
            }
        }
        MushroomVariant::Brown => {
            let radius = config.foliage_radius;
            for dx in -radius..=radius {
                for dz in -radius..=radius {
                    let min_x = dx == -radius;
                    let max_x = dx == radius;
                    let min_z = dz == -radius;
                    let max_z = dz == radius;
                    let x_edge = min_x || max_x;
                    let z_edge = min_z || max_z;
                    if x_edge && z_edge {
                        continue;
                    }
                    let pos = origin.offset(dx, height, dz);
                    let west = min_x || (z_edge && dx == 1 - radius);
                    let east = max_x || (z_edge && dx == radius - 1);
                    let north = min_z || (x_edge && dz == 1 - radius);
                    let south = max_z || (x_edge && dz == radius - 1);
                    let mut state = config.cap_provider.get_state(region, random, origin);
                    if [
                        Properties::WEST,
                        Properties::EAST,
                        Properties::NORTH,
                        Properties::SOUTH,
                    ]
                    .iter()
                    .all(|p| state.get(*p).is_some())
                    {
                        state = state
                            .with(Properties::WEST, west)
                            .with(Properties::EAST, east)
                            .with(Properties::NORTH, north)
                            .with(Properties::SOUTH, south);
                    }
                    place_mushroom_block(region, pos, state);
                }
            }
        }
    }
}
