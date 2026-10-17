use shulkr::util::Key;
use shulkr::world::block::Block;

use crate::world::chunk::heightmap::HeightmapType;
use shulkr::util::BoundingBox;

use crate::util::rng::RandomSource;
use crate::world::worldgen::block_predicate::HolderSet;
use crate::world::worldgen::region::WorldGenRegion;
use crate::world::worldgen::structure::pieces::structure_pieces_builder::StructurePiecesBuilder;
use crate::world::worldgen::structure::structure_piece::{
    PieceBase, PieceKind, StructurePiece, air,
};
use crate::world::worldgen::structure::template_structure_piece::TemplateStructurePiece;
use crate::world::worldgen::structure::templatesystem::liquid_settings::LiquidSettings;
use crate::world::worldgen::structure::templatesystem::structure_place_settings::StructurePlaceSettings;
use crate::world::worldgen::structure::templatesystem::structure_processor::StructureProcessor;
use crate::world::worldgen::structure::templatesystem::structure_template::calculate_relative_position;
use shulkr::util::BlockPosition;
use shulkr::world::block::Mirror;
use shulkr::world::block::Rotation;

pub const GENERATION_HEIGHT: i32 = 90;

const STRUCTURE_LOCATION_IGLOO: Key = Key::const_vanilla("igloo/top");
const STRUCTURE_LOCATION_LADDER: Key = Key::const_vanilla("igloo/middle");
const STRUCTURE_LOCATION_LABORATORY: Key = Key::const_vanilla("igloo/bottom");

fn pivot(location: &Key) -> BlockPosition {
    match location.path() {
        "igloo/middle" => BlockPosition::new(1, 3, 1),
        "igloo/bottom" => BlockPosition::new(3, 6, 7),
        _ => BlockPosition::new(3, 5, 5),
    }
}

fn offset(location: &Key) -> BlockPosition {
    match location.path() {
        "igloo/middle" => BlockPosition::new(2, -3, 4),
        "igloo/bottom" => BlockPosition::new(0, -3, -2),
        _ => BlockPosition::new(0, 0, 0),
    }
}

fn make_settings(rotation: Rotation, location: &Key) -> StructurePlaceSettings {
    let mut settings = StructurePlaceSettings::new()
        .set_rotation(rotation)
        .set_mirror(Mirror::None)
        .set_rotation_pivot(pivot(location))
        .set_liquid_settings(LiquidSettings::IgnoreWaterlogging);
    settings.add_processor(StructureProcessor::BlockIgnore {
        blocks: HolderSet::Entries(vec![Key::const_vanilla("structure_block")]),
    });
    settings
}

pub fn add_pieces(
    position: BlockPosition,
    rotation: Rotation,
    builder: &mut StructurePiecesBuilder,
    random: &mut RandomSource,
) {
    if random.next_f64() < 0.5 {
        let depth = random.next_i32_bounded(8) + 4;
        builder.add_piece(IglooPiece::new(
            &STRUCTURE_LOCATION_LABORATORY,
            position,
            rotation,
            depth * 3,
        ));
        for i in 0..depth - 1 {
            builder.add_piece(IglooPiece::new(
                &STRUCTURE_LOCATION_LADDER,
                position,
                rotation,
                i * 3,
            ));
        }
    }

    builder.add_piece(IglooPiece::new(
        &STRUCTURE_LOCATION_IGLOO,
        position,
        rotation,
        0,
    ));
}

#[derive(Debug)]
pub struct IglooPiece {
    pub template: TemplateStructurePiece,
}

impl IglooPiece {
    #[expect(
        clippy::new_ret_no_self,
        reason = "mirrors the vanilla piece constructor"
    )]
    fn new(
        location: &Key,
        position: BlockPosition,
        rotation: Rotation,
        depth: i32,
    ) -> StructurePiece {
        let template_offset = offset(location);
        let template_position = position
            .offset(
                template_offset.x(),
                template_offset.y(),
                template_offset.z(),
            )
            .below_n(depth);
        let (base, template) = TemplateStructurePiece::new(
            0,
            location,
            make_settings(rotation, location),
            template_position,
        );
        StructurePiece::new(base, PieceKind::Igloo(Self { template }))
    }

    pub fn post_process(
        &mut self,
        base: &mut PieceBase,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        chunk_bb: BoundingBox,
        reference_pos: BlockPosition,
    ) {
        let location = self.template.template_name.clone();
        let settings = make_settings(self.template.place_settings.rotation(), &location);
        let template_offset = offset(&location);
        let entrance_offset = calculate_relative_position(
            &settings,
            BlockPosition::new(3 - template_offset.x(), 0, -template_offset.z()),
        );
        let entrance_pos = self.template.template_position.offset(
            entrance_offset.x(),
            entrance_offset.y(),
            entrance_offset.z(),
        );
        let height = region.get_height(
            HeightmapType::WorldSurfaceWg,
            entrance_pos.x(),
            entrance_pos.z(),
        );

        let old_template_pos = self.template.template_position;
        self.template.template_position =
            self.template
                .template_position
                .offset(0, height - GENERATION_HEIGHT - 1, 0);

        self.template.post_process(
            base,
            region,
            random,
            chunk_bb,
            reference_pos,
            |marker_id, position, region, random, _chunk_bb| {
                if marker_id == "chest" {
                    region.set_block(position, air());
                    random.next_i64();
                }
            },
        );

        if location == STRUCTURE_LOCATION_IGLOO {
            let trap_door_offset =
                calculate_relative_position(&settings, BlockPosition::new(3, 0, 5));
            let trap_door_pos = self.template.template_position.offset(
                trap_door_offset.x(),
                trap_door_offset.y(),
                trap_door_offset.z(),
            );
            let below_state = region.get_block(trap_door_pos.below());
            if !below_state.is_air() && below_state.is(Block::LADDER) {
                region.set_block(trap_door_pos, Block::SNOW_BLOCK.default_state());
            }
        }

        self.template.template_position = old_template_pos;
    }
}
