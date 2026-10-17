use std::string::ToString;

use shulkr::util::Key;
use shulkr::world::block::{Block, BlockState};

use crate::util::rng::RandomSource;
use crate::world::worldgen::region::WorldGenRegion;
use crate::world::worldgen::structure::structure_piece::PieceBase;
use crate::world::worldgen::structure::templatesystem::structure_place_settings::StructurePlaceSettings;
use crate::world::worldgen::structure::templatesystem::structure_template::StructureTemplate;
use crate::world::worldgen::structure::templatesystem::structure_template_manager::templates;
use shulkr::util::BlockPosition;
use shulkr::util::BoundingBox;
use shulkr::util::Direction;

const STRUCTURE_BLOCK: Key = Key::const_vanilla("structure_block");
const JIGSAW: Key = Key::const_vanilla("jigsaw");

#[derive(Debug)]
pub struct TemplateStructurePiece {
    pub template_name: Key,
    pub template: &'static StructureTemplate,
    pub place_settings: StructurePlaceSettings,
    pub template_position: BlockPosition,
}

impl TemplateStructurePiece {
    pub fn new(
        gen_depth: i32,
        template_location: &Key,
        place_settings: StructurePlaceSettings,
        position: BlockPosition,
    ) -> (PieceBase, Self) {
        let template = templates().get_or_create(template_location);
        let mut base = PieceBase::new(
            gen_depth,
            template.get_bounding_box(&place_settings, position),
        );
        base.set_orientation(Some(Direction::North));
        (
            base,
            Self {
                template_name: template_location.clone(),
                template,
                place_settings,
                template_position: position,
            },
        )
    }

    pub const fn move_by(&mut self, dx: i32, dy: i32, dz: i32) {
        self.template_position = self.template_position.offset(dx, dy, dz);
    }

    pub fn post_process(
        &mut self,
        base: &mut PieceBase,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        chunk_bb: BoundingBox,
        reference_pos: BlockPosition,
        mut handle_data_marker: impl FnMut(
            &str,
            BlockPosition,
            &WorldGenRegion<'_>,
            &mut RandomSource,
            BoundingBox,
        ),
    ) {
        self.place_settings = self.place_settings.clone().set_bounding_box(chunk_bb);
        base.bounding_box = self
            .template
            .get_bounding_box(&self.place_settings, self.template_position);

        if !self.template.place_in_world(
            region,
            self.template_position,
            reference_pos,
            &self.place_settings,
            random,
        ) {
            return;
        }

        for marker in self.template.filter_blocks(
            self.template_position,
            &self.place_settings,
            &STRUCTURE_BLOCK,
            true,
            random,
        ) {
            let Some(nbt) = marker.nbt.as_ref() else {
                continue;
            };
            if nbt.string("mode").map(|mode| mode.to_string()) != Some("DATA".to_owned()) {
                continue;
            }
            let metadata = nbt
                .string("metadata")
                .map(ToString::to_string)
                .unwrap_or_default();
            handle_data_marker(&metadata, marker.pos, region, random, chunk_bb);
        }

        for jigsaw in self.template.filter_blocks(
            self.template_position,
            &self.place_settings,
            &JIGSAW,
            true,
            random,
        ) {
            let Some(nbt) = jigsaw.nbt.as_ref() else {
                continue;
            };
            let state_string = nbt
                .string("final_state")
                .map(ToString::to_string)
                .unwrap_or_else(|| "minecraft:air".to_owned());
            let target_state =
                BlockState::parse(&state_string).unwrap_or_else(|| Block::AIR.default_state());
            region.set_block(jigsaw.pos, target_state);
        }
    }
}
