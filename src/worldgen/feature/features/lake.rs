use serde::Deserialize;

use crate::registry::blocks::vanilla_blocks;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::worldgen::block_predicate::BlockPredicate;
use crate::worldgen::feature::features::mark_above_for_post_processing;
use crate::worldgen::feature::state_provider::BlockStateProvider;
use crate::worldgen::region::WorldGenRegion;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LakeConfig {
    pub fluid: BlockStateProvider,
    pub barrier: BlockStateProvider,
    pub can_place_feature: BlockPredicate,
    pub can_replace_with_air_or_fluid: BlockPredicate,
    pub can_replace_with_barrier: BlockPredicate,
}

const GRID_LEN: usize = 2048;

const fn index(xx: usize, zz: usize, yy: usize) -> usize {
    (xx * 16 + zz) * 8 + yy
}

fn borders_grid(grid: &[bool; GRID_LEN], xx: usize, zz: usize, yy: usize) -> bool {
    !grid[index(xx, zz, yy)]
        && (xx < 15 && grid[index(xx + 1, zz, yy)]
            || xx > 0 && grid[index(xx - 1, zz, yy)]
            || zz < 15 && grid[index(xx, zz + 1, yy)]
            || zz > 0 && grid[index(xx, zz - 1, yy)]
            || yy < 7 && grid[index(xx, zz, yy + 1)]
            || yy > 0 && grid[index(xx, zz, yy - 1)])
}

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPos,
    config: &LakeConfig,
) -> bool {
    if origin.y() <= region.min_y() + 4 {
        return false;
    }

    let origin = origin.offset(-8, -4, -8);
    let mut grid = [false; GRID_LEN];
    let spots = random.next_i32_bounded(4) + 4;

    for _ in 0..spots {
        let xr = random.next_f64() * 6.0 + 3.0;
        let yr = random.next_f64() * 4.0 + 2.0;
        let zr = random.next_f64() * 6.0 + 3.0;
        let xp = random.next_f64() * (16.0 - xr - 2.0) + 1.0 + xr / 2.0;
        let yp = random.next_f64() * (8.0 - yr - 4.0) + 2.0 + yr / 2.0;
        let zp = random.next_f64() * (16.0 - zr - 2.0) + 1.0 + zr / 2.0;

        for xx in 1..15usize {
            for zz in 1..15usize {
                for yy in 1..7usize {
                    let xd = (xx as f64 - xp) / (xr / 2.0);
                    let yd = (yy as f64 - yp) / (yr / 2.0);
                    let zd = (zz as f64 - zp) / (zr / 2.0);
                    if xd * xd + yd * yd + zd * zd < 1.0 {
                        grid[index(xx, zz, yy)] = true;
                    }
                }
            }
        }
    }

    let fluid = config.fluid.get_state(region, random, origin);

    for xx in 0..16usize {
        for zz in 0..16usize {
            for yy in 0..8usize {
                if !borders_grid(&grid, xx, zz, yy) {
                    continue;
                }
                let pos = origin.offset(xx as i32, yy as i32, zz as i32);
                let state = region.get_block(pos);
                if yy >= 4 && state.get_block().config.liquid {
                    return false;
                }
                if yy < 4 && !state.is_solid() && state != fluid {
                    return false;
                }
                if !config.can_place_feature.test(region, pos) {
                    return false;
                }
            }
        }
    }

    let cave_air = vanilla_blocks::CAVE_AIR.default_state();
    for xx in 0..16usize {
        for zz in 0..16usize {
            for yy in 0..8usize {
                if !grid[index(xx, zz, yy)] {
                    continue;
                }
                let pos = origin.offset(xx as i32, yy as i32, zz as i32);
                if config.can_replace_with_air_or_fluid.test(region, pos) {
                    let place_air = yy >= 4;
                    region.set_block(pos, if place_air { cave_air } else { fluid });
                    if place_air {
                        mark_above_for_post_processing(region, pos);
                    }
                }
            }
        }
    }

    let barrier = config.barrier.get_state(region, random, origin);
    if !barrier.is_air() {
        for xx in 0..16usize {
            for zz in 0..16usize {
                for yy in 0..8usize {
                    if !borders_grid(&grid, xx, zz, yy) {
                        continue;
                    }
                    if yy >= 4 && random.next_i32_bounded(2) == 0 {
                        continue;
                    }
                    let pos = origin.offset(xx as i32, yy as i32, zz as i32);
                    let state = region.get_block(pos);
                    if state.is_solid() && config.can_replace_with_barrier.test(region, pos) {
                        region.set_block(pos, barrier);
                        mark_above_for_post_processing(region, pos);
                    }
                }
            }
        }
    }

    true
}

