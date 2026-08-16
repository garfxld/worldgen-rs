use crate::chunk::heightmap::HeightmapType;
use crate::registry::REGISTRY;
use crate::registry::blocks::properties::Properties;
use crate::registry::blocks::vanilla_blocks;
use crate::util::pos::BlockPos;
use crate::worldgen::region::WorldGenRegion;

pub fn place(region: &WorldGenRegion<'_>, origin: BlockPos) -> bool {
    let ice = vanilla_blocks::ICE.default_state();
    let snow = vanilla_blocks::SNOW.default_state();

    for dx in 0..16 {
        for dz in 0..16 {
            let x = origin.x() + dx;
            let z = origin.z() + dz;
            let y = region.get_height(HeightmapType::MotionBlocking, x, z);
            let top_pos = BlockPos::new(x, y, z);
            let below_pos = top_pos.below();
            let biome = REGISTRY
                .biomes
                .by_id(region.get_biome(top_pos) as usize)
                .expect("biome id should be registered");

            if biome.should_freeze(region, below_pos, false) {
                region.set_block(below_pos, ice);
            }

            if biome.should_snow(region, top_pos) {
                region.set_block(top_pos, snow);
                let below_state = region.get_block(below_pos);
                if below_state.try_get_value(&Properties::SNOWY).is_some() {
                    region.set_block(below_pos, below_state.set_value(&Properties::SNOWY, true));
                }
            }
        }
    }

    true
}
