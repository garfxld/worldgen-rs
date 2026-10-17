use crate::util::rng::RandomSource;
use crate::world::worldgen::block_predicate::BlockPredicate;
use crate::world::worldgen::feature::features::weeping_vines;
use crate::world::worldgen::region::WorldGenRegion;
use serde::Deserialize;
use shulkr::util::BlockPosition;
use shulkr::world::block::Block;
use shulkr::world::block::BlockState;

#[derive(Debug, Deserialize)]
pub struct HugeFungusConfig {
    pub valid_base_block: BlockState,
    pub stem_state: BlockState,
    pub hat_state: BlockState,
    pub decor_state: BlockState,
    pub replaceable_blocks: BlockPredicate,
    #[serde(default)]
    pub planted: bool,
}

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPosition,
    config: &HugeFungusConfig,
) -> bool {
    if region.get_block(origin.below()).as_block() != config.valid_base_block.as_block() {
        return false;
    }

    let mut total_height = random.next_i32_between(4, 13);
    if random.next_i32_bounded(12) == 0 {
        total_height *= 2;
    }

    if !config.planted {
        let gen_depth = region.max_y() - region.min_y() + 1;
        if origin.y() + total_height + 1 >= gen_depth {
            return false;
        }
    }

    let is_huge = !config.planted && random.next_f32() < 0.06;
    region.set_block(origin, Block::AIR.default_state());
    place_stem(region, random, config, origin, total_height, is_huge);
    place_hat(region, random, config, origin, total_height, is_huge);
    true
}

fn is_replaceable(
    region: &WorldGenRegion<'_>,
    pos: BlockPosition,
    config: &HugeFungusConfig,
    check_non_replaceable_plants: bool,
) -> bool {
    if region.get_block(pos).is_replaceable() {
        return true;
    }
    check_non_replaceable_plants && config.replaceable_blocks.test(region, pos)
}

fn place_stem(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    config: &HugeFungusConfig,
    surface_origin: BlockPosition,
    total_height: i32,
    is_huge: bool,
) {
    let stem = config.stem_state;
    let stem_radius = i32::from(is_huge);

    for dx in -stem_radius..=stem_radius {
        for dz in -stem_radius..=stem_radius {
            let corner_of_huge_stem = is_huge && dx.abs() == stem_radius && dz.abs() == stem_radius;

            for dy in 0..total_height {
                let pos = surface_origin.offset(dx, dy, dz);
                if is_replaceable(region, pos, config, true) {
                    if config.planted {
                        region.set_block(pos, stem);
                    } else if corner_of_huge_stem {
                        if random.next_f32() < 0.1 {
                            region.set_block(pos, stem);
                        }
                    } else {
                        region.set_block(pos, stem);
                    }
                }
            }
        }
    }
}

fn place_hat(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    config: &HugeFungusConfig,
    surface_origin: BlockPosition,
    total_height: i32,
    is_huge: bool,
) {
    let place_vines = config.hat_state.is(Block::NETHER_WART_BLOCK);
    let hat_height = (random.next_i32_bounded(1 + total_height / 3) + 5).min(total_height);
    let hat_start_y = total_height - hat_height;

    for dy in hat_start_y..=total_height {
        let mut radius = if dy < total_height - random.next_i32_bounded(3) {
            2
        } else {
            1
        };
        if hat_height > 8 && dy < hat_start_y + 4 {
            radius = 3;
        }
        if is_huge {
            radius += 1;
        }

        for dx in -radius..=radius {
            for dz in -radius..=radius {
                let is_edge_x = dx == -radius || dx == radius;
                let is_edge_z = dz == -radius || dz == radius;
                let inside = !is_edge_x && !is_edge_z && dy != total_height;
                let corner = is_edge_x && is_edge_z;
                let is_hat_bottom = dy < hat_start_y + 3;
                let pos = surface_origin.offset(dx, dy, dz);
                if is_replaceable(region, pos, config, false) {
                    if is_hat_bottom {
                        if !inside {
                            place_hat_drop_block(
                                region,
                                random,
                                pos,
                                config.hat_state,
                                place_vines,
                            );
                        }
                    } else if inside {
                        place_hat_block(
                            region,
                            random,
                            config,
                            pos,
                            0.1,
                            0.2,
                            if place_vines { 0.1 } else { 0.0 },
                        );
                    } else if corner {
                        place_hat_block(
                            region,
                            random,
                            config,
                            pos,
                            0.01,
                            0.7,
                            if place_vines { 0.083 } else { 0.0 },
                        );
                    } else {
                        place_hat_block(
                            region,
                            random,
                            config,
                            pos,
                            5.0e-4,
                            0.98,
                            if place_vines { 0.07 } else { 0.0 },
                        );
                    }
                }
            }
        }
    }
}

fn place_hat_block(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    config: &HugeFungusConfig,
    pos: BlockPosition,
    decor_block_probability: f32,
    hat_block_probability: f32,
    vines_probability: f32,
) {
    if random.next_f32() < decor_block_probability {
        region.set_block(pos, config.decor_state);
    } else if random.next_f32() < hat_block_probability {
        region.set_block(pos, config.hat_state);
        if random.next_f32() < vines_probability {
            try_place_weeping_vines(region, random, pos);
        }
    }
}

fn place_hat_drop_block(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    pos: BlockPosition,
    hat_state: BlockState,
    place_vines: bool,
) {
    if region.get_block(pos.below()).as_block() == hat_state.as_block() {
        region.set_block(pos, hat_state);
    } else if random.next_f32() < 0.15 {
        region.set_block(pos, hat_state);
        if place_vines && random.next_i32_bounded(11) == 0 {
            try_place_weeping_vines(region, random, pos);
        }
    }
}

fn try_place_weeping_vines(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    hat_block_pos: BlockPosition,
) {
    let place_pos = hat_block_pos.below();
    if region.get_block(place_pos).is_air() {
        let mut goal_vine_height = random.next_i32_between(1, 5);
        if random.next_i32_bounded(7) == 0 {
            goal_vine_height *= 2;
        }

        weeping_vines::place_weeping_vines_column(
            region,
            random,
            place_pos,
            goal_vine_height,
            23,
            25,
        );
    }
}
