pub mod canyon;
pub mod cave;

use serde::Deserialize;

use crate::util::pos::ChunkPos;
use crate::util::rng::RandomSource;
use crate::world::chunk::carving_mask::CarvingMask;
use crate::world::worldgen::worldgen_context::WorldGenContext;

use canyon::CanyonWorldCarver;
use cave::CaveWorldCarver;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum WorldCarver {
    #[serde(rename = "minecraft:cave")]
    Cave(CaveWorldCarver),
    #[serde(rename = "minecraft:canyon")]
    Canyon(CanyonWorldCarver),
}

const fn middle_block_x(chunk_pos: ChunkPos) -> i32 {
    chunk_pos.0.x * 16 + 8
}

const fn middle_block_z(chunk_pos: ChunkPos) -> i32 {
    chunk_pos.0.y * 16 + 8
}

impl WorldCarver {
    pub const fn get_range() -> i32 {
        4
    }

    pub fn is_start_chunk(&self, random: &mut RandomSource) -> bool {
        let probability = match self {
            Self::Cave(cave) => cave.probability,
            Self::Canyon(canyon) => canyon.probability,
        };
        random.next_f32() <= probability
    }

    pub fn carve(
        &self,
        context: &WorldGenContext,
        random: &mut RandomSource,
        chunk_pos: ChunkPos,
        source_chunk_pos: ChunkPos,
        output: &mut CarvingMask,
    ) -> bool {
        match self {
            Self::Cave(cave) => cave.carve(context, random, chunk_pos, source_chunk_pos, output),
            Self::Canyon(canyon) => {
                canyon.carve(context, random, chunk_pos, source_chunk_pos, output)
            }
        }
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "mirrors WorldCarver.carveEllipsoid"
)]
pub fn carve_ellipsoid(
    chunk_pos: ChunkPos,
    x: f64,
    y: f64,
    z: f64,
    horizontal_radius: f64,
    vertical_radius: f64,
    output: &mut CarvingMask,
    skip_checker: &dyn Fn(f64, f64, f64, i32) -> bool,
) {
    let center_x = f64::from(middle_block_x(chunk_pos));
    let center_z = f64::from(middle_block_z(chunk_pos));
    let max_delta = 16.0 + horizontal_radius * 2.0;
    if (x - center_x).abs() > max_delta || (z - center_z).abs() > max_delta {
        return;
    }
    let chunk_min_x = chunk_pos.0.x * 16;
    let chunk_min_z = chunk_pos.0.y * 16;
    let min_x_index = ((x - horizontal_radius).floor() as i32 - chunk_min_x - 1).max(0);
    let max_x_index = ((x + horizontal_radius).floor() as i32 - chunk_min_x).min(15);
    let min_y = ((y - vertical_radius).floor() as i32 - 1).max(output.min_y());
    let max_y = ((y + vertical_radius).floor() as i32 + 1).min(output.max_y());
    let min_z_index = ((z - horizontal_radius).floor() as i32 - chunk_min_z - 1).max(0);
    let max_z_index = ((z + horizontal_radius).floor() as i32 - chunk_min_z).min(15);
    for x_index in min_x_index..=max_x_index {
        let world_x = chunk_min_x + x_index;
        let xd = (f64::from(world_x) + 0.5 - x) / horizontal_radius;
        for z_index in min_z_index..=max_z_index {
            let world_z = chunk_min_z + z_index;
            let zd = (f64::from(world_z) + 0.5 - z) / horizontal_radius;
            if xd * xd + zd * zd >= 1.0 {
                continue;
            }
            let mut world_y = max_y;
            while world_y > min_y {
                let yd = (f64::from(world_y) - 0.5 - y) / vertical_radius;
                if !skip_checker(xd, yd, zd, world_y) {
                    output.carve(x_index, world_y, z_index);
                }
                world_y -= 1;
            }
        }
    }
}

pub fn can_reach(
    chunk_pos: ChunkPos,
    x: f64,
    z: f64,
    current_step: i32,
    total_steps: i32,
    thickness: f32,
) -> bool {
    let x_mid = f64::from(middle_block_x(chunk_pos));
    let z_mid = f64::from(middle_block_z(chunk_pos));
    let xd = x - x_mid;
    let zd = z - z_mid;
    let remaining = f64::from(total_steps - current_step);
    let rr = f64::from(thickness + 2.0 + 16.0);
    xd * xd + zd * zd - remaining * remaining <= rr * rr
}
