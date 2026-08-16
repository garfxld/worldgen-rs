use crate::chunk::heightmap::HeightmapType;
use crate::registry::blocks::vanilla_blocks;
use crate::util::direction::Direction;
use crate::util::pos::{BlockPos, ChunkPos};
use crate::util::rng::RandomSource;
use crate::util::shuffle;
use crate::worldgen::block_survival;
use crate::worldgen::region::WorldGenRegion;

pub fn place(region: &WorldGenRegion<'_>, random: &mut RandomSource, origin: BlockPos) -> bool {
    let chunk_pos = ChunkPos::from_block_pos(origin);
    let min_x = chunk_pos.0.x * 16;
    let min_z = chunk_pos.0.y * 16;

    let mut x_positions: Vec<i32> = (min_x..=min_x + 15).collect();
    shuffle(&mut x_positions, random);
    let mut z_positions: Vec<i32> = (min_z..=min_z + 15).collect();
    shuffle(&mut z_positions, random);

    let torch = vanilla_blocks::TORCH.default_state();

    for x in x_positions {
        for &z in &z_positions {
            let y = region.get_height(HeightmapType::MotionBlockingNoLeaves, x, z);
            let chest_pos = BlockPos::new(x, y, z);
            let state = region.get_block(chest_pos);
            if state.is_air()
                || state.get_collision_shape_at(chest_pos).is_empty()
            {
                region.set_block(chest_pos, vanilla_blocks::CHEST.default_state());
                random.next_i64();

                for direction in Direction::HORIZONTAL {
                    let torch_pos = direction.relative(chest_pos);
                    if block_survival::can_survive(region, torch_pos, torch) {
                        region.set_block(torch_pos, torch);
                    }
                }

                return true;
            }
        }
    }

    false
}
