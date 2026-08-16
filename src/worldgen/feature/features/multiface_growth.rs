use serde::Deserialize;

use crate::registry::REGISTRY;
use crate::registry::blocks::properties::{BoolProperty, Properties};
use crate::registry::blocks::shapes::is_offset_face_full;
use crate::registry::blocks::vanilla_blocks;
use crate::util::direction::Direction;
use crate::util::ident::{BlockState, Identifier};
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::util::shuffle;
use crate::worldgen::region::WorldGenRegion;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MultifaceGrowthConfig {
    pub block: Identifier,
    #[serde(default = "default_search_range")]
    pub search_range: i32,
    #[serde(default)]
    pub can_place_on_floor: bool,
    #[serde(default)]
    pub can_place_on_ceiling: bool,
    #[serde(default)]
    pub can_place_on_wall: bool,
    #[serde(default = "default_chance_of_spreading")]
    pub chance_of_spreading: f32,
    pub can_be_placed_on: Vec<Identifier>,
}

const fn default_search_range() -> i32 {
    10
}

const fn default_chance_of_spreading() -> f32 {
    0.5
}

const fn face_property(direction: Direction) -> &'static BoolProperty {
    match direction {
        Direction::Down => &Properties::DOWN,
        Direction::Up => &Properties::UP,
        Direction::North => &Properties::NORTH,
        Direction::South => &Properties::SOUTH,
        Direction::West => &Properties::WEST,
        Direction::East => &Properties::EAST,
    }
}

fn has_face(state: BlockState, direction: Direction) -> bool {
    state.try_get_value(face_property(direction)).unwrap_or(false)
}

fn with_face(state: BlockState, direction: Direction, value: bool) -> BlockState {
    state.set_value(face_property(direction), value)
}

fn is_air_or_water(state: BlockState) -> bool {
    state.is_air() || state.get_block() == &vanilla_blocks::WATER
}

fn is_replaceable_target(state: BlockState, block: &Identifier) -> bool {
    is_air_or_water(state) || &state.get_block().key == block
}

fn valid_directions(config: &MultifaceGrowthConfig) -> Vec<Direction> {
    let mut dirs = Vec::with_capacity(6);
    if config.can_place_on_ceiling {
        dirs.push(Direction::Up);
    }
    if config.can_place_on_floor {
        dirs.push(Direction::Down);
    }
    if config.can_place_on_wall {
        dirs.extend(Direction::HORIZONTAL);
    }
    dirs
}

fn shuffled_directions(config: &MultifaceGrowthConfig, random: &mut RandomSource) -> Vec<Direction> {
    let mut dirs = valid_directions(config);
    shuffle(&mut dirs, random);
    dirs
}

fn shuffled_directions_except(
    config: &MultifaceGrowthConfig,
    random: &mut RandomSource,
    except: Direction,
) -> Vec<Direction> {
    let mut dirs = valid_directions(config);
    dirs.retain(|&d| d != except);
    shuffle(&mut dirs, random);
    dirs
}

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPos,
    config: &MultifaceGrowthConfig,
) -> bool {
    if !is_air_or_water(region.get_block(origin)) {
        return false;
    }

    let search_dirs = shuffled_directions(config, random);
    if place_growth_if_possible(region, random, origin, config, &search_dirs) {
        return true;
    }

    for &search_dir in &search_dirs {
        let placement_dirs = shuffled_directions_except(config, random, search_dir.opposite());
        let pos = search_dir.relative(origin);
        for _ in 0..config.search_range {
            let state = region.get_block(pos);
            if !is_air_or_water(state) && &state.get_block().key != &config.block {
                break;
            }
            if place_growth_if_possible(region, random, pos, config, &placement_dirs) {
                return true;
            }
        }
    }

    false
}

fn place_growth_if_possible(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    pos: BlockPos,
    config: &MultifaceGrowthConfig,
    placement_dirs: &[Direction],
) -> bool {
    for &dir in placement_dirs {
        let neighbor = region.get_block(dir.relative(pos));
        if config.can_be_placed_on.iter().any(|id| id == &neighbor.get_block().key) {
            set_face(region, config, pos, dir);
            if random.next_f32() < config.chance_of_spreading {
                spread(region, config, random, pos, dir);
            }
            return true;
        }
    }
    false
}

fn spread(
    region: &WorldGenRegion<'_>,
    config: &MultifaceGrowthConfig,
    random: &mut RandomSource,
    pos: BlockPos,
    starting_face: Direction,
) {
    let mut dirs = Direction::ALL.to_vec();
    shuffle(&mut dirs, random);
    for spread_dir in dirs {
        if spread_dir.get_axis() == starting_face.get_axis() {
            continue;
        }
        if let Some((target, face)) = spread_pos(region, config, pos, starting_face, spread_dir) {
            set_face(region, config, target, face);
            return;
        }
    }
}

fn spread_pos(
    region: &WorldGenRegion<'_>,
    config: &MultifaceGrowthConfig,
    pos: BlockPos,
    starting_face: Direction,
    spread_dir: Direction,
) -> Option<(BlockPos, Direction)> {
    if can_spread_into(region, config, pos, spread_dir) {
        return Some((pos, spread_dir));
    }
    let same_plane = spread_dir.relative(pos);
    if can_spread_into(region, config, same_plane, starting_face) {
        return Some((same_plane, starting_face));
    }
    let wrap_around = starting_face.relative(spread_dir.relative(pos));
    if can_spread_into(region, config, wrap_around, spread_dir.opposite()) {
        return Some((wrap_around, spread_dir.opposite()));
    }
    None
}

fn can_spread_into(
    region: &WorldGenRegion<'_>,
    config: &MultifaceGrowthConfig,
    pos: BlockPos,
    face: Direction,
) -> bool {
    let existing = region.get_block(pos);
    if !is_replaceable_target(existing, &config.block) {
        return false;
    }
    if existing.get_block().key == config.block && has_face(existing, face) {
        return false;
    }
    let support_pos = face.relative(pos);
    let support = region.get_block(support_pos);
    is_offset_face_full(support.get_support_shape_at(support_pos), face.opposite())
}

fn set_face(region: &WorldGenRegion<'_>, config: &MultifaceGrowthConfig, pos: BlockPos, face: Direction) {
    let existing = region.get_block(pos);
    let base = if existing.get_block().key == config.block {
        existing
    } else {
        let block = REGISTRY
            .blocks
            .by_key(&config.block)
            .expect("multiface growth block must be registered");
        let mut state = REGISTRY.blocks.get_default_state_id(block);
        if existing.get_block() == &vanilla_blocks::WATER && state.try_get_value(&Properties::WATERLOGGED).is_some() {
            state = state.set_value(&Properties::WATERLOGGED, true);
        }
        state
    };
    region.set_block(pos, with_face(base, face, true));
}
