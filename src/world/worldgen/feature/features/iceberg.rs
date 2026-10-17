use shulkr::world::block::Block;
use std::f64::consts::PI;

use crate::util::math::ceil_f32;
use crate::util::rng::RandomSource;
use crate::world::worldgen::region::WorldGenRegion;
use serde::Deserialize;
use shulkr::util::BlockPosition;
use shulkr::world::block::BlockState;

#[derive(Debug, Deserialize)]
pub struct IcebergConfig {
    pub state: BlockState,
}

struct Shape {
    ellipse_a: i32,
    ellipse_c: i32,
    angle: f64,
    is_ellipse: bool,
    snow_on_top: bool,
    main_block_state: BlockState,
}

fn is_iceberg_state(state: BlockState) -> bool {
    let block = state.as_block();
    block == Block::PACKED_ICE || block == Block::SNOW_BLOCK || block == Block::BLUE_ICE
}

fn signed_distance_ellipse(
    xo: i32,
    zo: i32,
    origin: (i32, i32),
    a: i32,
    c: i32,
    angle: f64,
) -> f64 {
    let dx = f64::from(xo - origin.0);
    let dz = f64::from(zo - origin.1);
    ((dx * angle.cos() - dz * angle.sin()) / f64::from(a)).powi(2)
        + ((dx * angle.sin() + dz * angle.cos()) / f64::from(c)).powi(2)
        - 1.0
}

fn signed_distance_circle(
    xo: i32,
    zo: i32,
    origin: (i32, i32),
    radius: i32,
    random: &mut RandomSource,
) -> f64 {
    let off = 10.0 * random.next_f32().clamp(0.2, 0.8) / radius as f32;
    f64::from(off) + f64::from(xo - origin.0).powi(2) + f64::from(zo - origin.1).powi(2)
        - f64::from(radius).powi(2)
}

fn height_dependent_radius_round(
    random: &mut RandomSource,
    y_off: i32,
    height: i32,
    width: i32,
) -> i32 {
    let k = 3.5 - random.next_f32();
    let mut scale = (1.0 - (y_off as f32).powi(2) / (height as f32 * k)) * width as f32;
    if height > 15 + random.next_i32_bounded(5) {
        let temp_y_off = if y_off < 3 + random.next_i32_bounded(6) {
            y_off / 2
        } else {
            y_off
        };
        scale = (1.0 - temp_y_off as f32 / (height as f32 * k * 0.4)) * width as f32;
    }
    ceil_f32(scale / 2.0)
}

fn height_dependent_radius_ellipse(y_off: i32, height: i32, width: i32) -> i32 {
    let scale = (1.0 - (y_off as f32).powi(2) / height as f32) * width as f32;
    ceil_f32(scale / 2.0)
}

fn height_dependent_radius_steep(
    random: &mut RandomSource,
    y_off: i32,
    height: i32,
    width: i32,
) -> i32 {
    let k = 1.0 + random.next_f32() / 2.0;
    let scale = (1.0 - y_off as f32 / (height as f32 * k)) * width as f32;
    ceil_f32(scale / 2.0)
}

const fn get_ellipse_c(y_off: i32, height: i32, shape_ellipse_c: i32) -> i32 {
    let mut c = shape_ellipse_c;
    if y_off > 0 && height - y_off <= 3 {
        c -= 4 - (height - y_off);
    }
    c
}

fn below_is_air(region: &WorldGenRegion<'_>, pos: BlockPosition) -> bool {
    region.get_block(pos.below()).is_air()
}

fn set_iceberg_block(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    shape: &Shape,
    pos: BlockPosition,
    h_diff: i32,
    height: i32,
) {
    let state = region.get_block(pos);
    let block = state.as_block();
    if !(state.is_air()
        || block == Block::SNOW_BLOCK
        || block == Block::ICE
        || block == Block::WATER)
    {
        return;
    }

    let randomness = !shape.is_ellipse || random.next_f64() > 0.05;
    let divisor = if shape.is_ellipse { 3 } else { 2 };
    if shape.snow_on_top
        && block != Block::WATER
        && f64::from(h_diff)
            <= f64::from(random.next_i32_bounded(1.max(height / divisor))) + f64::from(height) * 0.6
        && randomness
    {
        region.set_block(pos, Block::SNOW_BLOCK.default_state());
    } else {
        region.set_block(pos, shape.main_block_state);
    }
}

#[expect(clippy::too_many_arguments, reason = "vanilla parameter list")]
fn generate_iceberg_block(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    shape: &Shape,
    origin: BlockPosition,
    height: i32,
    xo: i32,
    y_off: i32,
    zo: i32,
    radius: i32,
    a: i32,
) {
    let signed_dist = if shape.is_ellipse {
        signed_distance_ellipse(
            xo,
            zo,
            (0, 0),
            a,
            get_ellipse_c(y_off, height, shape.ellipse_c),
            shape.angle,
        )
    } else {
        signed_distance_circle(xo, zo, (0, 0), radius, random)
    };

    if signed_dist < 0.0 {
        let pos = origin.offset(xo, y_off, zo);
        let compare_val = if shape.is_ellipse {
            -0.5
        } else {
            f64::from(-6 - random.next_i32_bounded(3))
        };
        if signed_dist > compare_val && random.next_f64() > 0.9 {
            return;
        }
        set_iceberg_block(region, random, shape, pos, height - y_off, height);
    }
}

fn remove_floating_snow_layer(region: &WorldGenRegion<'_>, pos: BlockPosition) {
    let above = pos.above();
    if region.get_block(above).is(Block::SNOW) {
        region.set_block(above, Block::AIR.default_state());
    }
}

#[expect(clippy::too_many_arguments, reason = "vanilla parameter list")]
fn carve(
    region: &WorldGenRegion<'_>,
    shape: &Shape,
    radius: i32,
    y_off: i32,
    global_origin: BlockPosition,
    under_water: bool,
    angle: f64,
    local_origin: (i32, i32),
) {
    let a = radius + 1 + shape.ellipse_a / 3;
    let c = (radius - 3).min(3) + shape.ellipse_c / 2 - 1;

    for xo in -a..a {
        for zo in -a..a {
            let signed_dist = signed_distance_ellipse(xo, zo, local_origin, a, c, angle);
            if signed_dist < 0.0 {
                let pos = global_origin.offset(xo, y_off, zo);
                let state = region.get_block(pos);
                if is_iceberg_state(state) || state.is(Block::SNOW_BLOCK) {
                    if under_water {
                        region.set_block(pos, Block::WATER.default_state());
                    } else {
                        region.set_block(pos, Block::AIR.default_state());
                        remove_floating_snow_layer(region, pos);
                    }
                }
            }
        }
    }
}

fn generate_cut_out(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    shape: &Shape,
    width: i32,
    height: i32,
    global_origin: BlockPosition,
) {
    let random_sign_x = if random.next_bool() { -1 } else { 1 };
    let random_sign_z = if random.next_bool() { -1 } else { 1 };
    let mut x_off = random.next_i32_bounded(1.max(width / 2 - 2));
    if random.next_bool() {
        x_off = width / 2 + 1 - random.next_i32_bounded(1.max(width - width / 2 - 1));
    }
    let mut z_off = random.next_i32_bounded(1.max(width / 2 - 2));
    if random.next_bool() {
        z_off = width / 2 + 1 - random.next_i32_bounded(1.max(width - width / 2 - 1));
    }
    if shape.is_ellipse {
        x_off = random.next_i32_bounded(1.max(shape.ellipse_a - 5));
        z_off = x_off;
    }

    let local_origin = (random_sign_x * x_off, random_sign_z * z_off);
    let angle = if shape.is_ellipse {
        shape.angle + PI / 2.0
    } else {
        random.next_f64() * 2.0 * PI
    };

    for y_off in 0..height - 3 {
        let radius = height_dependent_radius_round(random, y_off, height, width);
        carve(
            region,
            shape,
            radius,
            y_off,
            global_origin,
            false,
            angle,
            local_origin,
        );
    }

    let mut y_off = -1;
    while y_off > -height + random.next_i32_bounded(5) {
        let radius = height_dependent_radius_steep(random, -y_off, height, width);
        carve(
            region,
            shape,
            radius,
            y_off,
            global_origin,
            true,
            angle,
            local_origin,
        );
        y_off -= 1;
    }
}

fn smooth(
    region: &WorldGenRegion<'_>,
    shape: &Shape,
    origin: BlockPosition,
    width: i32,
    height: i32,
) {
    let a = if shape.is_ellipse {
        shape.ellipse_a
    } else {
        width / 2
    };

    for x in -a..=a {
        for z in -a..=a {
            for y_off in 0..=height {
                let pos = origin.offset(x, y_off, z);
                let state = region.get_block(pos);
                if is_iceberg_state(state) || state.is(Block::SNOW) {
                    if below_is_air(region, pos) {
                        region.set_block(pos, Block::AIR.default_state());
                        region.set_block(pos.above(), Block::AIR.default_state());
                    } else if is_iceberg_state(state) {
                        let sides = [
                            region.get_block(pos.west()),
                            region.get_block(pos.east()),
                            region.get_block(pos.north()),
                            region.get_block(pos.south()),
                        ];
                        let non_iceberg = sides
                            .iter()
                            .filter(|&&side| !is_iceberg_state(side))
                            .count();
                        if non_iceberg >= 3 {
                            region.set_block(pos, Block::AIR.default_state());
                        }
                    }
                }
            }
        }
    }
}

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPosition,
    config: &IcebergConfig,
) -> bool {
    let origin = origin.at_y(region.sea_level());
    let snow_on_top = random.next_f64() > 0.7;
    let shape_angle = random.next_f64() * 2.0 * PI;
    let shape_ellipse_a = 11 - random.next_i32_bounded(5);
    let shape_ellipse_c = 3 + random.next_i32_bounded(3);
    let is_ellipse = random.next_f64() > 0.7;
    let shape = Shape {
        ellipse_a: shape_ellipse_a,
        ellipse_c: shape_ellipse_c,
        angle: shape_angle,
        is_ellipse,
        snow_on_top,
        main_block_state: config.state,
    };

    let mut over_water_height = if is_ellipse {
        random.next_i32_bounded(6) + 6
    } else {
        random.next_i32_bounded(15) + 3
    };
    if !is_ellipse && random.next_f64() > 0.9 {
        over_water_height += random.next_i32_bounded(19) + 7;
    }

    let under_water_height = (over_water_height + random.next_i32_bounded(11)).min(18);
    let width =
        (over_water_height + random.next_i32_bounded(7) - random.next_i32_bounded(5)).min(11);
    let a = if is_ellipse { shape_ellipse_a } else { 11 };

    for xo in -a..a {
        for zo in -a..a {
            for y_off in 0..over_water_height {
                let radius = if is_ellipse {
                    height_dependent_radius_ellipse(y_off, over_water_height, width)
                } else {
                    height_dependent_radius_round(random, y_off, over_water_height, width)
                };
                if is_ellipse || xo < radius {
                    generate_iceberg_block(
                        region,
                        random,
                        &shape,
                        origin,
                        over_water_height,
                        xo,
                        y_off,
                        zo,
                        radius,
                        a,
                    );
                }
            }
        }
    }

    smooth(region, &shape, origin, width, over_water_height);

    for xo in -a..a {
        for zo in -a..a {
            let mut y_off = -1;
            while y_off > -under_water_height {
                let new_a = if is_ellipse {
                    ceil_f32(
                        a as f32
                            * (1.0 - (y_off as f32).powi(2) / (under_water_height as f32 * 8.0)),
                    )
                } else {
                    a
                };
                let radius =
                    height_dependent_radius_steep(random, -y_off, under_water_height, width);
                if xo < radius {
                    generate_iceberg_block(
                        region,
                        random,
                        &shape,
                        origin,
                        under_water_height,
                        xo,
                        y_off,
                        zo,
                        radius,
                        new_a,
                    );
                }
                y_off -= 1;
            }
        }
    }

    let do_cut_out = if is_ellipse {
        random.next_f64() > 0.1
    } else {
        random.next_f64() > 0.7
    };
    if do_cut_out {
        generate_cut_out(region, random, &shape, width, over_water_height, origin);
    }

    true
}
