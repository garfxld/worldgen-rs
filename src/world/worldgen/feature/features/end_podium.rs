use serde::Deserialize;
use shulkr::util::{BlockPosition, Direction};
use shulkr::world::block::{Block, BlockState};

use crate::world::worldgen::feature::features::blockpos_iter::between_closed;
use crate::world::worldgen::region::WorldGenRegion;

#[derive(Debug, Clone, Deserialize)]
pub struct EndPodiumFeature {
    #[serde(default)]
    pub active: bool,
}

fn closer_than(a: BlockPosition, b: BlockPosition, distance: f64) -> bool {
    let dx = f64::from(a.x() - b.x());
    let dy = f64::from(a.y() - b.y());
    let dz = f64::from(a.z() - b.z());
    dx * dx + dy * dy + dz * dz < distance * distance
}

impl EndPodiumFeature {
    fn drop_previous_and_set_block(region: &WorldGenRegion<'_>, pos: BlockPosition, block: Block) {
        if !region.get_block(pos).is(block) {
            if !region.get_block(pos).is_air() {
                region.set_block(pos, Block::AIR.default_state());
            }
            region.set_block(pos, block.default_state());
        }
    }

    pub fn place(&self, region: &WorldGenRegion<'_>, origin: BlockPosition) -> bool {
        let bedrock = Block::BEDROCK.default_state();
        let air = Block::AIR.default_state();
        for pos in between_closed(
            BlockPosition::new(origin.x() - 4, origin.y() - 1, origin.z() - 4),
            BlockPosition::new(origin.x() + 4, origin.y() + 32, origin.z() + 4),
        ) {
            let inside_rim = closer_than(pos, origin, 2.5);
            if !inside_rim && !closer_than(pos, origin, 3.5) {
                continue;
            }
            if pos.y() < origin.y() {
                if inside_rim {
                    region.set_block(pos, bedrock);
                } else if self.active {
                    Self::drop_previous_and_set_block(region, pos, Block::END_STONE);
                } else {
                    region.set_block(pos, Block::END_STONE.default_state());
                }
            } else if pos.y() > origin.y() {
                if self.active {
                    Self::drop_previous_and_set_block(region, pos, Block::AIR);
                } else {
                    region.set_block(pos, air);
                }
            } else if !inside_rim {
                region.set_block(pos, bedrock);
            } else if self.active {
                Self::drop_previous_and_set_block(region, pos, Block::END_PORTAL);
            } else {
                region.set_block(pos, air);
            }
        }
        for y in 0..4 {
            region.set_block(origin.offset(0, y, 0), bedrock);
        }
        let center_of_pillar = origin.offset(0, 2, 0);
        for face in Direction::HORIZONTAL {
            let torch: BlockState = Block::WALL_TORCH.default_state();
            let torch = torch
                .with_by_name("facing", horizontal_name(face))
                .unwrap_or(torch);
            region.set_block(center_of_pillar.relative(face), torch);
        }
        true
    }
}

const fn horizontal_name(direction: Direction) -> &'static str {
    match direction {
        Direction::North => "north",
        Direction::South => "south",
        Direction::West => "west",
        Direction::East => "east",
        Direction::Up => "up",
        Direction::Down => "down",
    }
}
