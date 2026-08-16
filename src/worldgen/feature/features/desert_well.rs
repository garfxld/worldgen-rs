use crate::registry::blocks::vanilla_blocks;
use crate::util::direction::Direction;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::worldgen::region::WorldGenRegion;

pub fn place(region: &WorldGenRegion<'_>, random: &mut RandomSource, origin: BlockPos) -> bool {
    let mut origin = origin.above();
    while region.get_block(origin).is_air() && origin.y() > region.min_y() + 2 {
        origin = origin.below();
    }

    if region.get_block(origin).get_block() != &vanilla_blocks::SAND {
        return false;
    }

    for ox in -2..=2 {
        for oz in -2..=2 {
            if region.get_block(origin.offset(ox, -1, oz)).is_air()
                && region.get_block(origin.offset(ox, -2, oz)).is_air()
            {
                return false;
            }
        }
    }

    let sandstone = vanilla_blocks::SANDSTONE.default_state();
    let sand_slab = vanilla_blocks::SANDSTONE_SLAB.default_state();
    let sand = vanilla_blocks::SAND.default_state();
    let water = vanilla_blocks::WATER.default_state();

    for oy in -2..=0 {
        for ox in -2..=2 {
            for oz in -2..=2 {
                region.set_block(origin.offset(ox, oy, oz), sandstone);
            }
        }
    }

    region.set_block(origin, water);
    for direction in Direction::HORIZONTAL {
        region.set_block(direction.relative(origin), water);
    }

    let sand_center = origin.below();
    region.set_block(sand_center, sand);
    for direction in Direction::HORIZONTAL {
        region.set_block(direction.relative(sand_center), sand);
    }

    for ox in -2..=2 {
        for oz in -2..=2 {
            if ox == -2 || ox == 2 || oz == -2 || oz == 2 {
                region.set_block(origin.offset(ox, 1, oz), sandstone);
            }
        }
    }

    region.set_block(origin.offset(2, 1, 0), sand_slab);
    region.set_block(origin.offset(-2, 1, 0), sand_slab);
    region.set_block(origin.offset(0, 1, 2), sand_slab);
    region.set_block(origin.offset(0, 1, -2), sand_slab);

    for ox in -1..=1 {
        for oz in -1..=1 {
            if ox == 0 && oz == 0 {
                region.set_block(origin.offset(ox, 4, oz), sandstone);
            } else {
                region.set_block(origin.offset(ox, 4, oz), sand_slab);
            }
        }
    }

    for oy in 1..=3 {
        region.set_block(origin.offset(-1, oy, -1), sandstone);
        region.set_block(origin.offset(-1, oy, 1), sandstone);
        region.set_block(origin.offset(1, oy, -1), sandstone);
        region.set_block(origin.offset(1, oy, 1), sandstone);
    }

    let water_positions = [
        origin,
        origin.east(),
        origin.south(),
        origin.west(),
        origin.north(),
    ];
    let sus_sand = vanilla_blocks::SUSPICIOUS_SAND.default_state();
    region.set_block(pick(random, &water_positions).below_n(1), sus_sand);
    region.set_block(pick(random, &water_positions).below_n(2), sus_sand);
    true
}

fn pick(random: &mut RandomSource, positions: &[BlockPos]) -> BlockPos {
    positions[random.next_i32_bounded(positions.len() as i32) as usize]
}
