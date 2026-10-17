use crate::world::chunk::heightmap::HeightmapType;
use shulkr::util::BoundingBox;
use shulkr::util::Key;
use shulkr::world::block::Block;

use crate::util::rng::RandomSource;
use crate::world::worldgen::block_predicate::HolderSet;
use crate::world::worldgen::region::WorldGenRegion;
use crate::world::worldgen::structure::structure_piece::{PieceBase, PieceKind, StructurePiece};
use crate::world::worldgen::structure::template_structure_piece::TemplateStructurePiece;
use crate::world::worldgen::structure::templatesystem::structure_place_settings::StructurePlaceSettings;
use crate::world::worldgen::structure::templatesystem::structure_processor::StructureProcessor;
use shulkr::util::BlockPosition;
use shulkr::world::block::Mirror;
use shulkr::world::block::Rotation;

const NUMBER_OF_BLOCKS_ALLOWED_IN_WORLD_GEN_REGION: i32 = 32;
const PIVOT: BlockPosition = BlockPosition::new(4, 0, 15);

const STRUCTURE_LOCATION_BEACHED: &[&str] = &[
    "shipwreck/with_mast",
    "shipwreck/sideways_full",
    "shipwreck/sideways_fronthalf",
    "shipwreck/sideways_backhalf",
    "shipwreck/rightsideup_full",
    "shipwreck/rightsideup_fronthalf",
    "shipwreck/rightsideup_backhalf",
    "shipwreck/with_mast_degraded",
    "shipwreck/rightsideup_full_degraded",
    "shipwreck/rightsideup_fronthalf_degraded",
    "shipwreck/rightsideup_backhalf_degraded",
];

const STRUCTURE_LOCATION_OCEAN: &[&str] = &[
    "shipwreck/with_mast",
    "shipwreck/upsidedown_full",
    "shipwreck/upsidedown_fronthalf",
    "shipwreck/upsidedown_backhalf",
    "shipwreck/sideways_full",
    "shipwreck/sideways_fronthalf",
    "shipwreck/sideways_backhalf",
    "shipwreck/rightsideup_full",
    "shipwreck/rightsideup_fronthalf",
    "shipwreck/rightsideup_backhalf",
    "shipwreck/with_mast_degraded",
    "shipwreck/upsidedown_full_degraded",
    "shipwreck/upsidedown_fronthalf_degraded",
    "shipwreck/upsidedown_backhalf_degraded",
    "shipwreck/sideways_full_degraded",
    "shipwreck/sideways_fronthalf_degraded",
    "shipwreck/sideways_backhalf_degraded",
    "shipwreck/rightsideup_full_degraded",
    "shipwreck/rightsideup_fronthalf_degraded",
    "shipwreck/rightsideup_backhalf_degraded",
];

fn make_settings(rotation: Rotation) -> StructurePlaceSettings {
    let mut settings = StructurePlaceSettings::new()
        .set_rotation(rotation)
        .set_mirror(Mirror::None)
        .set_rotation_pivot(PIVOT);
    settings.add_processor(StructureProcessor::BlockIgnore {
        blocks: HolderSet::Entries(vec![
            Key::const_vanilla("structure_block"),
            Key::const_vanilla("air"),
        ]),
    });
    settings
}

pub fn random_piece(
    position: BlockPosition,
    rotation: Rotation,
    random: &mut RandomSource,
    is_beached: bool,
) -> StructurePiece {
    let pool = if is_beached {
        STRUCTURE_LOCATION_BEACHED
    } else {
        STRUCTURE_LOCATION_OCEAN
    };
    let index = random.next_i32_bounded(pool.len() as i32) as usize;
    let location = Key::of(format!("minecraft:{}", pool[index]));
    ShipwreckPiece::new(&location, position, rotation, is_beached)
}

#[derive(Debug)]
pub struct ShipwreckPiece {
    pub template: TemplateStructurePiece,
    is_beached: bool,
    height_adjusted: bool,
}

impl ShipwreckPiece {
    #[expect(
        clippy::new_ret_no_self,
        reason = "mirrors the vanilla piece constructor"
    )]
    fn new(
        location: &Key,
        position: BlockPosition,
        rotation: Rotation,
        is_beached: bool,
    ) -> StructurePiece {
        let (base, template) =
            TemplateStructurePiece::new(0, location, make_settings(rotation), position);
        StructurePiece::new(
            base,
            PieceKind::Shipwreck(Self {
                template,
                is_beached,
                height_adjusted: false,
            }),
        )
    }

    pub const fn is_too_big_to_fit_in_world_gen_region(&self) -> bool {
        let size = self.template.template.size();
        size.x > NUMBER_OF_BLOCKS_ALLOWED_IN_WORLD_GEN_REGION
            || size.y > NUMBER_OF_BLOCKS_ALLOWED_IN_WORLD_GEN_REGION
    }

    pub fn calculate_beached_position(&self, min_y: i32, random: &mut RandomSource) -> i32 {
        min_y - self.template.template.size().y / 2 - random.next_i32_bounded(3)
    }

    pub const fn adjust_position_height(&mut self, new_height: i32) {
        self.height_adjusted = true;
        self.template.template_position = BlockPosition::new(
            self.template.template_position.x(),
            new_height,
            self.template.template_position.z(),
        );
    }

    pub fn post_process(
        &mut self,
        base: &mut PieceBase,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        chunk_bb: BoundingBox,
        reference_pos: BlockPosition,
    ) {
        if !self.height_adjusted && !self.is_too_big_to_fit_in_world_gen_region() {
            let mut min_y = region.max_y() + 1;
            let mut mean = 0;
            let template_size = self.template.template.size();
            let heightmap_type = if self.is_beached {
                HeightmapType::WorldSurfaceWg
            } else {
                HeightmapType::OceanFloorWg
            };
            let base_size = template_size.x * template_size.z;
            if base_size == 0 {
                mean = region.get_height(
                    heightmap_type,
                    self.template.template_position.x(),
                    self.template.template_position.z(),
                );
            } else {
                let origin = self.template.template_position;
                for x in origin.x()..origin.x() + template_size.x {
                    for z in origin.z()..origin.z() + template_size.z {
                        let heightmap = region.get_height(heightmap_type, x, z);
                        mean += heightmap;
                        min_y = min_y.min(heightmap);
                    }
                }
                mean /= base_size;
            }

            let new_height = if self.is_beached {
                self.calculate_beached_position(min_y, random)
            } else {
                mean
            };
            self.adjust_position_height(new_height);
        }

        self.template.post_process(
            base,
            region,
            random,
            chunk_bb,
            reference_pos,
            |marker_id, position, region, random, _chunk_bb| {
                if matches!(marker_id, "map_chest" | "treasure_chest" | "supply_chest")
                    && is_randomizable_container(region, position.below())
                {
                    random.next_i64();
                }
            },
        );
    }
}

fn is_randomizable_container(region: &WorldGenRegion<'_>, pos: BlockPosition) -> bool {
    let block = region.get_block(pos).as_block();
    block == Block::CHEST
        || block == Block::TRAPPED_CHEST
        || block == Block::BARREL
        || block == Block::DISPENSER
        || block == Block::DROPPER
        || block == Block::HOPPER
}
