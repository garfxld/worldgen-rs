use crate::registry::biome::{should_freeze, should_snow};
use crate::world::chunk::heightmap::HeightmapType;
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::registry::registries;
use shulkr::util::BlockPosition;
use shulkr::world::block::Block;
use shulkr::world::block::Properties;

pub fn place(region: &WorldGenRegion<'_>, origin: BlockPosition) -> bool {
    let ice = Block::ICE.default_state();
    let snow = Block::SNOW.default_state();

    for dx in 0..16 {
        for dz in 0..16 {
            let x = origin.x() + dx;
            let z = origin.z() + dz;
            let y = region.get_height(HeightmapType::MotionBlocking, x, z);
            let top_pos = BlockPosition::new(x, y, z);
            let below_pos = top_pos.below();
            let biome = registries::BIOME
                .by_id(region.get_biome(top_pos))
                .expect("biome id should be registered");

            if should_freeze(biome, region, below_pos, false) {
                region.set_block(below_pos, ice);
            }

            if should_snow(biome, region, top_pos) {
                region.set_block(top_pos, snow);
                let below_state = region.get_block(below_pos);
                if below_state.get(Properties::SNOWY).is_some() {
                    region.set_block(below_pos, below_state.with(Properties::SNOWY, true));
                }
            }
        }
    }

    true
}
