use crate::world::chunk::heightmap::HeightmapType;
use shulkr::registry::registries;
use shulkr::util::Key;

use crate::world::worldgen::structure::structure::{
    GenerationContext, GenerationStub, on_top_of_chunk_center,
};
use crate::world::worldgen::structure::structures::ocean_monument_pieces::{
    self, BIOME_RANGE_CHECK,
};

fn required_surrounding_tag() -> Key {
    Key::of("minecraft:required_ocean_monument_surrounding")
}

fn is_surrounded_by_required_biomes(
    cx: &GenerationContext<'_>,
    block_x: i32,
    block_z: i32,
) -> bool {
    let tag = required_surrounding_tag();
    let sea_level = cx.generator.sea_level();
    cx.biome_resolver
        .borrow_mut()
        .get_biomes_within(block_x, sea_level, block_z, BIOME_RANGE_CHECK)
        .into_iter()
        .all(|biome| registries::BIOME.has_tag(biome.key(), &tag))
}

pub fn find_generation_point<'a>(cx: &'a GenerationContext<'a>) -> Option<GenerationStub<'a>> {
    let chunk_pos = cx.chunk_pos;
    let offset_x = chunk_pos.0.x * 16 + 9;
    let offset_z = chunk_pos.0.y * 16 + 9;

    if !is_surrounded_by_required_biomes(cx, offset_x, offset_z) {
        return None;
    }

    on_top_of_chunk_center(
        cx,
        HeightmapType::OceanFloorWg,
        Box::new(move |builder| {
            let west = chunk_pos.0.x * 16 - BIOME_RANGE_CHECK;
            let north = chunk_pos.0.y * 16 - BIOME_RANGE_CHECK;
            cx.with_random(|random| {
                let direction = random.next_horizontal_direction();
                builder.add_piece(ocean_monument_pieces::create_top_piece(
                    random, west, north, direction,
                ));
            });
        }),
    )
}
