use serde::Deserialize;
use shulkr::util::Key;
use shulkr::world::block::Block;

use crate::util::rng::RandomSource;
use crate::util::shuffle;
use crate::world::worldgen::block_predicate::HolderSet;
use crate::world::worldgen::feature::multiface::{self, Spreader};
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util::BlockPosition;
use shulkr::util::Direction;
use shulkr::world::block::BlockState;

#[derive(Debug, Deserialize)]
pub struct MultifaceGrowthConfig {
    pub block: Key,
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
    pub can_be_placed_on: HolderSet,
}

const fn default_search_range() -> i32 {
    10
}

const fn default_chance_of_spreading() -> f32 {
    0.5
}

fn is_air_or_water(state: BlockState) -> bool {
    state.is_air() || state.is(Block::WATER)
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

fn shuffled_directions(
    config: &MultifaceGrowthConfig,
    random: &mut RandomSource,
) -> Vec<Direction> {
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
    origin: BlockPosition,
    config: &MultifaceGrowthConfig,
) -> bool {
    if !is_air_or_water(region.get_block(origin)) {
        return false;
    }
    let Some(placer_block) = Block::from_key(config.block.clone()).copied() else {
        return false;
    };

    let search_dirs = shuffled_directions(config, random);
    if place_growth_if_possible(
        region,
        random,
        placer_block,
        origin,
        region.get_block(origin),
        config,
        &search_dirs,
    ) {
        return true;
    }

    for &search_dir in &search_dirs {
        let placement_dirs = shuffled_directions_except(config, random, search_dir.opposite());
        let pos = origin.relative(search_dir);
        for _ in 0..config.search_range {
            let state = region.get_block(pos);
            if !is_air_or_water(state) && !state.is(placer_block) {
                break;
            }
            if place_growth_if_possible(
                region,
                random,
                placer_block,
                pos,
                state,
                config,
                &placement_dirs,
            ) {
                return true;
            }
        }
    }

    false
}

fn place_growth_if_possible(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    placer_block: Block,
    pos: BlockPosition,
    old_state: BlockState,
    config: &MultifaceGrowthConfig,
    placement_dirs: &[Direction],
) -> bool {
    for &dir in placement_dirs {
        let neighbor = region.get_block(pos.relative(dir));
        if config.can_be_placed_on.contains_block(neighbor) {
            let Some(new_state) =
                multiface::state_for_placement(region, placer_block, old_state, pos, dir)
            else {
                return false;
            };
            region.set_block(pos, new_state);
            region.mark_pos_for_postprocessing(pos);
            if random.next_f32() < config.chance_of_spreading {
                Spreader::Default(placer_block).spread_from_face_toward_random_direction(
                    new_state, region, pos, dir, random, true,
                );
            }
            return true;
        }
    }
    false
}
