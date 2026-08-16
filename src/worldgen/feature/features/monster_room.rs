use crate::registry::blocks::properties::Properties;
use crate::registry::blocks::vanilla_blocks;
use crate::util::direction::Direction;
use crate::util::ident::{BlockState, Identifier};
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::worldgen::region::WorldGenRegion;

const FEATURES_CANNOT_REPLACE: Identifier =
    Identifier::const_vanilla("features_cannot_replace");

fn safe_set_block(region: &WorldGenRegion<'_>, pos: BlockPos, state: BlockState) {
    if !region
        .get_block(pos)
        .get_block()
        .has_tag(&FEATURES_CANNOT_REPLACE)
    {
        region.set_block(pos, state);
    }
}

fn reorient(region: &WorldGenRegion<'_>, pos: BlockPos, state: BlockState) -> BlockState {
    let mut solid_neighbor = None;

    for direction in Direction::HORIZONTAL {
        let neighbor = region.get_block(direction.relative(pos));
        if neighbor.get_block() == &vanilla_blocks::CHEST {
            return state;
        }

        if neighbor.is_solid_render() {
            if solid_neighbor.is_some() {
                solid_neighbor = None;
                break;
            }
            solid_neighbor = Some(direction);
        }
    }

    if let Some(direction) = solid_neighbor {
        return state.set_value(&Properties::HORIZONTAL_FACING, direction.opposite());
    }

    let mut lock_dir = state.get_value(&Properties::HORIZONTAL_FACING);
    if region.get_block(lock_dir.relative(pos)).is_solid_render() {
        lock_dir = lock_dir.opposite();
    }

    if region.get_block(lock_dir.relative(pos)).is_solid_render() {
        lock_dir = lock_dir.rotate_y_clockwise();
    }

    if region.get_block(lock_dir.relative(pos)).is_solid_render() {
        lock_dir = lock_dir.opposite();
    }

    state.set_value(&Properties::HORIZONTAL_FACING, lock_dir)
}

pub fn place(region: &WorldGenRegion<'_>, random: &mut RandomSource, origin: BlockPos) -> bool {
    let air = vanilla_blocks::CAVE_AIR.default_state();
    let xr = random.next_i32_bounded(2) + 2;
    let min_x = -xr - 1;
    let max_x = xr + 1;
    let zr = random.next_i32_bounded(2) + 2;
    let min_z = -zr - 1;
    let max_z = zr + 1;
    let mut hole_count = 0;

    for dx in min_x..=max_x {
        for dy in -1..=4 {
            for dz in min_z..=max_z {
                let hole_pos = origin.offset(dx, dy, dz);
                let solid = region.get_block(hole_pos).is_solid();
                if dy == -1 && !solid {
                    return false;
                }

                if dy == 4 && !solid {
                    return false;
                }

                if (dx == min_x || dx == max_x || dz == min_z || dz == max_z)
                    && dy == 0
                    && region.get_block(hole_pos).is_air()
                    && region.get_block(hole_pos.above()).is_air()
                {
                    hole_count += 1;
                }
            }
        }
    }

    if !(1..=5).contains(&hole_count) {
        return false;
    }

    for dx in min_x..=max_x {
        for dy in (-1..=3).rev() {
            for dz in min_z..=max_z {
                let wall_pos = origin.offset(dx, dy, dz);
                let wall_state = region.get_block(wall_pos);
                if dx == min_x || dy == -1 || dz == min_z || dx == max_x || dz == max_z {
                    if wall_pos.y() >= region.min_y()
                        && !region.get_block(wall_pos.below()).is_solid()
                    {
                        region.set_block(wall_pos, air);
                    } else if wall_state.is_solid()
                        && wall_state.get_block() != &vanilla_blocks::CHEST
                    {
                        if dy == -1 && random.next_i32_bounded(4) != 0 {
                            safe_set_block(
                                region,
                                wall_pos,
                                vanilla_blocks::MOSSY_COBBLESTONE.default_state(),
                            );
                        } else {
                            safe_set_block(
                                region,
                                wall_pos,
                                vanilla_blocks::COBBLESTONE.default_state(),
                            );
                        }
                    }
                } else if wall_state.get_block() != &vanilla_blocks::CHEST
                    && wall_state.get_block() != &vanilla_blocks::SPAWNER
                {
                    safe_set_block(region, wall_pos, air);
                }
            }
        }
    }

    for _ in 0..2 {
        for _ in 0..3 {
            let x = origin.x() + random.next_i32_bounded(xr * 2 + 1) - xr;
            let z = origin.z() + random.next_i32_bounded(zr * 2 + 1) - zr;
            let chest_pos = BlockPos::new(x, origin.y(), z);
            if region.get_block(chest_pos).is_air() {
                let mut wall_count = 0;
                for direction in Direction::HORIZONTAL {
                    if region.get_block(direction.relative(chest_pos)).is_solid() {
                        wall_count += 1;
                    }
                }

                if wall_count == 1 {
                    let chest = reorient(
                        region,
                        chest_pos,
                        vanilla_blocks::CHEST.default_state(),
                    );
                    safe_set_block(region, chest_pos, chest);
                    random.next_i64();
                    break;
                }
            }
        }
    }

    safe_set_block(region, origin, vanilla_blocks::SPAWNER.default_state());
    random.next_i32_bounded(4);
    true
}
