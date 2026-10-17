use serde::Deserialize;

use crate::world::chunk::heightmap::HeightmapType;

use shulkr::world::block::BlockState;
use std::f32::consts::PI;

use crate::util::math;
use crate::util::rng::RandomSource;
use crate::world::worldgen::feature::features::{is_adjacent_to_air, should_skip_air_check};
use crate::world::worldgen::feature::rule_test::RuleTest;
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util::BlockPosition;

#[derive(Debug, Deserialize)]
pub struct OreConfig {
    pub size: i32,
    pub discard_chance_on_air_exposure: f32,
    pub targets: Vec<BlockReplacement>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BlockReplacement {
    pub target: RuleTest,
    pub state: BlockState,
}

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPosition,
    config: &OreConfig,
) -> bool {
    let dir = random.next_f32() * PI;
    let spread_xy = config.size as f32 / 8.0;
    let max_radius = math::ceil_f32((config.size as f32 / 16.0 * 2.0 + 1.0) / 2.0);
    let sin_dir = f64::from(dir).sin();
    let cos_dir = f64::from(dir).cos();
    let x0 = f64::from(origin.x()) + sin_dir * f64::from(spread_xy);
    let x1 = f64::from(origin.x()) - sin_dir * f64::from(spread_xy);
    let z0 = f64::from(origin.z()) + cos_dir * f64::from(spread_xy);
    let z1 = f64::from(origin.z()) - cos_dir * f64::from(spread_xy);
    let y0 = f64::from(origin.y() + random.next_i32_bounded(3) - 2);
    let y1 = f64::from(origin.y() + random.next_i32_bounded(3) - 2);
    let x_start = origin.x() - math::ceil_f32(spread_xy) - max_radius;
    let y_start = origin.y() - 2 - max_radius;
    let z_start = origin.z() - math::ceil_f32(spread_xy) - max_radius;
    let size_xz = 2 * (math::ceil_f32(spread_xy) + max_radius);
    let size_y = 2 * (2 + max_radius);

    for x_probe in x_start..=(x_start + size_xz) {
        for z_probe in z_start..=(z_start + size_xz) {
            if y_start <= region.get_height(HeightmapType::OceanFloorWg, x_probe, z_probe) {
                return do_place(
                    region, random, config, x0, x1, z0, z1, y0, y1, x_start, y_start, z_start,
                    size_xz, size_y,
                );
            }
        }
    }

    false
}

#[expect(
    clippy::too_many_arguments,
    reason = "mirrors vanilla OreFeature.doPlace"
)]
fn do_place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    config: &OreConfig,
    x0: f64,
    x1: f64,
    z0: f64,
    z1: f64,
    y0: f64,
    y1: f64,
    x_start: i32,
    y_start: i32,
    z_start: i32,
    size_xz: i32,
    size_y: i32,
) -> bool {
    let mut placed = 0;
    let size = config.size;
    let mut tested = vec![false; (size_xz * size_y * size_xz).max(0) as usize];
    let mut data = vec![0.0f64; size as usize * 4];

    for i in 0..size as usize {
        let step = i as f32 / size as f32;
        let xx = math::lerp(f64::from(step), x0, x1);
        let yy = math::lerp(f64::from(step), y0, y1);
        let zz = math::lerp(f64::from(step), z0, z1);
        let ss = random.next_f64() * f64::from(size) / 16.0;
        let r = (f64::from(math::sin(f64::from(PI * step)) + 1.0) * ss + 1.0) / 2.0;
        data[i * 4] = xx;
        data[i * 4 + 1] = yy;
        data[i * 4 + 2] = zz;
        data[i * 4 + 3] = r;
    }

    for i1 in 0..(size as usize).saturating_sub(1) {
        if data[i1 * 4 + 3] <= 0.0 {
            continue;
        }
        for i2 in (i1 + 1)..size as usize {
            if data[i2 * 4 + 3] <= 0.0 {
                continue;
            }
            let dx = data[i1 * 4] - data[i2 * 4];
            let dy = data[i1 * 4 + 1] - data[i2 * 4 + 1];
            let dz = data[i1 * 4 + 2] - data[i2 * 4 + 2];
            let dr = data[i1 * 4 + 3] - data[i2 * 4 + 3];
            if dr * dr > dx * dx + dy * dy + dz * dz {
                if dr > 0.0 {
                    data[i2 * 4 + 3] = -1.0;
                } else {
                    data[i1 * 4 + 3] = -1.0;
                }
            }
        }
    }

    for i in 0..size as usize {
        let r = data[i * 4 + 3];
        if r < 0.0 {
            continue;
        }
        let xx = data[i * 4];
        let yy = data[i * 4 + 1];
        let zz = data[i * 4 + 2];
        let x_min = math::fast_floor(xx - r).max(x_start);
        let y_min = math::fast_floor(yy - r).max(y_start);
        let z_min = math::fast_floor(zz - r).max(z_start);
        let x_max = math::fast_floor(xx + r).max(x_min);
        let y_max = math::fast_floor(yy + r).max(y_min);
        let z_max = math::fast_floor(zz + r).max(z_min);

        for x in x_min..=x_max {
            let xd = (f64::from(x) + 0.5 - xx) / r;
            if xd * xd >= 1.0 {
                continue;
            }
            for y in y_min..=y_max {
                let yd = (f64::from(y) + 0.5 - yy) / r;
                if xd * xd + yd * yd >= 1.0 {
                    continue;
                }
                for z in z_min..=z_max {
                    let zd = (f64::from(z) + 0.5 - zz) / r;
                    if xd * xd + yd * yd + zd * zd >= 1.0 || !region.is_inside_build_height(y) {
                        continue;
                    }
                    let index = ((x - x_start)
                        + (y - y_start) * size_xz
                        + (z - z_start) * size_xz * size_y)
                        as usize;
                    if index >= tested.len() {
                        tested.resize(index + 1, false);
                    }
                    if tested[index] {
                        continue;
                    }
                    tested[index] = true;
                    let ore_pos = BlockPosition::new(x, y, z);
                    if !region.ensure_can_write(ore_pos) {
                        continue;
                    }
                    let existing = region.get_block(ore_pos);
                    for target in &config.targets {
                        if can_place_ore(existing, region, random, config, target, ore_pos) {
                            region.set_block(ore_pos, target.state);
                            placed += 1;
                            break;
                        }
                    }
                }
            }
        }
    }

    placed > 0
}

pub fn can_place_ore(
    ore_pos_state: BlockState,
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    config: &OreConfig,
    target: &BlockReplacement,
    ore_pos: BlockPosition,
) -> bool {
    if !target.target.test(ore_pos_state, ore_pos, random) {
        return false;
    }
    if should_skip_air_check(random, config.discard_chance_on_air_exposure) {
        return true;
    }
    !is_adjacent_to_air(region, ore_pos)
}
