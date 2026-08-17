use crate::chunk::heightmap::HeightmapType;
use crate::registry::REGISTRY;
use crate::util::direction::Direction;
use crate::util::ident::Identifier;
use crate::worldgen::structure::structure::{
    GenerationContext, GenerationStub, on_top_of_chunk_center,
};
use crate::worldgen::structure::structures::ocean_monument_pieces::{
    self, BIOME_RANGE_CHECK,
};

fn required_surrounding_tag() -> Identifier {
    "minecraft:required_ocean_monument_surrounding"
        .parse()
        .expect("tag identifier should parse")
}

fn is_surrounded_by_required_biomes(
    context: &GenerationContext<'_>,
    block_x: i32,
    block_z: i32,
) -> bool {
    let tag = required_surrounding_tag();
    let sea_level = context.generator.sea_level();
    let mut sampler = context.generator.biome_source().chunk_sampler();

    let x0 = (block_x - BIOME_RANGE_CHECK) >> 2;
    let y0 = (sea_level - BIOME_RANGE_CHECK) >> 2;
    let z0 = (block_z - BIOME_RANGE_CHECK) >> 2;
    let x1 = (block_x + BIOME_RANGE_CHECK) >> 2;
    let y1 = (sea_level + BIOME_RANGE_CHECK) >> 2;
    let z1 = (block_z + BIOME_RANGE_CHECK) >> 2;

    for z in z0..=z1 {
        for x in x0..=x1 {
            for y in y0..=y1 {
                let biome = sampler.sample(x, y, z);
                if !REGISTRY.biomes.is_in_tag(&biome.key, &tag) {
                    return false;
                }
            }
        }
    }

    true
}

pub fn find_generation_point<'a>(
    context: &'a GenerationContext<'a>,
) -> Option<GenerationStub<'a>> {
    let chunk_pos = context.chunk_pos;
    let offset_x = chunk_pos.0.x * 16 + 9;
    let offset_z = chunk_pos.0.y * 16 + 9;

    if !is_surrounded_by_required_biomes(context, offset_x, offset_z) {
        return None;
    }

    Some(on_top_of_chunk_center(
        context,
        HeightmapType::OceanFloorWg,
        Box::new(move |builder| {
            let west = chunk_pos.0.x * 16 - BIOME_RANGE_CHECK;
            let north = chunk_pos.0.y * 16 - BIOME_RANGE_CHECK;
            context.with_random(|random| {
                let direction = Direction::get_random_horizontal(random);
                builder.add_piece(ocean_monument_pieces::create_top_piece(
                    random, west, north, direction,
                ));
            });
        }),
    ))
}
