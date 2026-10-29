use std::sync::LazyLock;

use shulkr::util::BlockPosition;
use shulkr::world::block::{Block, BlockState};

use crate::world::worldgen::region::WorldGenRegion;

const HEIGHT: i32 = 70;
const BARRIER_HEIGHT: i32 = 60;

struct Grid {
    states: Vec<BlockState>,
    width: i32,
    height: i32,
}

fn grid() -> &'static Grid {
    static GRID: LazyLock<Grid> = LazyLock::new(|| {
        let mut states = BlockState::all();
        states.sort_by_key(BlockState::state_id);
        let count = states.len() as f32;
        let width = count.sqrt().ceil() as i32;
        let height = (count / width as f32).ceil() as i32;
        Grid {
            states,
            width,
            height,
        }
    });
    &GRID
}

// DebugLevelSource.getBlockStateFor
fn block_state_for(world_x: i32, world_z: i32) -> BlockState {
    let grid = grid();
    if world_x > 0 && world_z > 0 && world_x % 2 != 0 && world_z % 2 != 0 {
        let (x, z) = (world_x / 2, world_z / 2);
        if x <= grid.width && z <= grid.height {
            let index = (x * grid.width + z).unsigned_abs() as usize;
            if let Some(&state) = grid.states.get(index) {
                return state;
            }
        }
    }
    Block::AIR.default_state()
}

// DebugLevelSource.applyBiomeDecoration
pub fn decorate(region: &WorldGenRegion<'_>) {
    let center = region.center();
    let barrier = Block::BARRIER.default_state();
    for x in 0..16 {
        for z in 0..16 {
            let world_x = center.0.x * 16 + x;
            let world_z = center.0.y * 16 + z;
            region.set_block_with_flags(
                BlockPosition::new(world_x, BARRIER_HEIGHT, world_z),
                barrier,
                2,
            );
            region.set_block_with_flags(
                BlockPosition::new(world_x, HEIGHT, world_z),
                block_state_for(world_x, world_z),
                2,
            );
        }
    }
}
