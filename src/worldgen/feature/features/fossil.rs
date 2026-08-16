use serde::Deserialize;

use crate::chunk::heightmap::HeightmapType;
use crate::registry::REGISTRY;
use crate::registry::blocks::vanilla_blocks;
use crate::util::geometry::BoundingBox;
use crate::util::ident::Identifier;
use crate::util::mirror::Mirror;
use crate::util::pos::{BlockPos, ChunkPos};
use crate::util::rng::RandomSource;
use crate::util::rotation::Rotation;
use crate::worldgen::region::WorldGenRegion;
use crate::worldgen::structure::templatesystem::structure_place_settings::StructurePlaceSettings;
use crate::worldgen::structure::templatesystem::structure_template_manager::{
    processor_list, templates,
};

#[derive(Debug, Clone, Deserialize)]
pub struct FossilConfig {
    pub fossil_structures: Vec<Identifier>,
    pub overlay_structures: Vec<Identifier>,
    pub fossil_processors: Identifier,
    pub overlay_processors: Identifier,
    pub max_empty_corners_allowed: i32,
}

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPos,
    config: &FossilConfig,
) -> bool {
    let rotation = Rotation::get_random(random);
    let fossil_index = random.next_i32_bounded(config.fossil_structures.len() as i32) as usize;
    let fossil_base = templates().get_or_create(&config.fossil_structures[fossil_index]);
    let fossil_overlay = templates().get_or_create(&config.overlay_structures[fossil_index]);

    let chunk_pos = ChunkPos::from_block_pos(origin);
    let bounding_box = BoundingBox::new(
        BlockPos::new(
            chunk_pos.0.x * 16 - 16,
            region.min_y(),
            chunk_pos.0.y * 16 - 16,
        )
        .0,
        BlockPos::new(
            chunk_pos.0.x * 16 + 15 + 16,
            region.max_y(),
            chunk_pos.0.y * 16 + 15 + 16,
        )
        .0,
    );

    let mut settings = StructurePlaceSettings::new()
        .set_rotation(rotation)
        .set_bounding_box(bounding_box)
        .use_shared_random(true);

    let size = fossil_base.size_rotated(rotation);
    let low_corner = origin.offset(-size.x / 2, 0, -size.z / 2);
    let mut lowest_surface_y = origin.y();
    for x_scan in 0..size.x {
        for z_scan in 0..size.z {
            lowest_surface_y = lowest_surface_y.min(region.get_height(
                HeightmapType::OceanFloorWg,
                low_corner.x() + x_scan,
                low_corner.z() + z_scan,
            ));
        }
    }

    let target_y = (lowest_surface_y - 15 - random.next_i32_bounded(10)).max(region.min_y() + 10);
    let target_pos =
        fossil_base.zero_position_with_transform(low_corner.at_y(target_y), Mirror::None, rotation);

    if count_empty_corners(region, fossil_base.bounding_box(&settings, target_pos))
        > config.max_empty_corners_allowed
    {
        return false;
    }

    settings.clear_processors();
    if let Some(list) = processor_list(&config.fossil_processors) {
        for processor in &list.processors {
            settings.add_processor(processor.clone());
        }
    }
    fossil_base.place_in_world(region, target_pos, target_pos, &settings, random);

    settings.clear_processors();
    if let Some(list) = processor_list(&config.overlay_processors) {
        for processor in &list.processors {
            settings.add_processor(processor.clone());
        }
    }
    fossil_overlay.place_in_world(region, target_pos, target_pos, &settings, random);
    true
}

fn count_empty_corners(region: &WorldGenRegion<'_>, structure_bounds: BoundingBox) -> i32 {
    let mut count = 0;
    structure_bounds.for_all_corners(|pos| {
        let state = region.get_block(pos);
        let block = REGISTRY.blocks.by_state_id(state);
        if state.is_air()
            || block.is_some_and(|block| {
                block.key == vanilla_blocks::LAVA.key || block.key == vanilla_blocks::WATER.key
            })
        {
            count += 1;
        }
    });
    count
}
