use shulkr::util::BoundingBox;
use shulkr::util::Key;
use shulkr::world::block::Block;

use crate::util::rng::RandomSource;
use crate::util::rng::legacy_random::LegacyRandomSource;
use crate::util::rotation::RotationExt;
use crate::world::worldgen::block_predicate::HolderSet;
use crate::world::worldgen::region::WorldGenRegion;
use crate::world::worldgen::structure::pieces::structure_pieces_builder::StructurePiecesBuilder;
use crate::world::worldgen::structure::structure_piece::{PieceBase, PieceKind, StructurePiece};
use crate::world::worldgen::structure::template_structure_piece::TemplateStructurePiece;
use crate::world::worldgen::structure::templatesystem::structure_place_settings::StructurePlaceSettings;
use crate::world::worldgen::structure::templatesystem::structure_processor::StructureProcessor;
use shulkr::util::BlockPosition;
use shulkr::world::block::Mirror;
use shulkr::world::block::Rotation;

const FOSSILS: &[&str] = &[
    "nether_fossils/fossil_1",
    "nether_fossils/fossil_2",
    "nether_fossils/fossil_3",
    "nether_fossils/fossil_4",
    "nether_fossils/fossil_5",
    "nether_fossils/fossil_6",
    "nether_fossils/fossil_7",
    "nether_fossils/fossil_8",
    "nether_fossils/fossil_9",
    "nether_fossils/fossil_10",
    "nether_fossils/fossil_11",
    "nether_fossils/fossil_12",
    "nether_fossils/fossil_13",
    "nether_fossils/fossil_14",
];

fn make_settings(rotation: Rotation) -> StructurePlaceSettings {
    let mut settings = StructurePlaceSettings::new()
        .set_rotation(rotation)
        .set_mirror(Mirror::None);
    settings.add_processor(StructureProcessor::BlockIgnore {
        blocks: HolderSet::Entries(vec![
            Key::const_vanilla("structure_block"),
            Key::const_vanilla("air"),
        ]),
    });
    settings
}

pub fn add_pieces(
    builder: &mut StructurePiecesBuilder,
    random: &mut RandomSource,
    position: BlockPosition,
) {
    let rotation = Rotation::get_random(random);
    let index = random.next_i32_bounded(FOSSILS.len() as i32) as usize;
    let location = Key::of(format!("minecraft:{}", FOSSILS[index]));
    builder.add_piece(NetherFossilPiece::new(&location, position, rotation));
}

#[derive(Debug)]
pub struct NetherFossilPiece {
    pub template: TemplateStructurePiece,
}

impl NetherFossilPiece {
    #[expect(
        clippy::new_ret_no_self,
        reason = "mirrors the vanilla piece constructor"
    )]
    fn new(location: &Key, position: BlockPosition, rotation: Rotation) -> StructurePiece {
        let (base, template) =
            TemplateStructurePiece::new(0, location, make_settings(rotation), position);
        StructurePiece::new(base, PieceKind::NetherFossil(Self { template }))
    }

    pub fn post_process(
        &mut self,
        base: &mut PieceBase,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        chunk_bb: BoundingBox,
        reference_pos: BlockPosition,
    ) {
        let fossil_bb = self.template.template.get_bounding_box(
            &self.template.place_settings,
            self.template.template_position,
        );
        let chunk_bb = chunk_bb.encapsulate_box(fossil_bb);
        self.template.post_process(
            base,
            region,
            random,
            chunk_bb,
            reference_pos,
            |_, _, _, _, _| {},
        );
        place_dried_ghast(region, fossil_bb, chunk_bb);
    }
}

fn place_dried_ghast(region: &WorldGenRegion<'_>, fossil_bb: BoundingBox, chunk_bb: BoundingBox) {
    let center = fossil_bb.center();
    let mut positional_random = LegacyRandomSource::from_seed(region.seed() as u64)
        .next_positional()
        .at(center.x(), center.y(), center.z());
    if positional_random.next_f32() >= 0.5 {
        return;
    }
    let x = fossil_bb.min_x() + positional_random.next_i32_bounded(fossil_bb.x_span());
    let y = fossil_bb.min_y();
    let z = fossil_bb.min_z() + positional_random.next_i32_bounded(fossil_bb.z_span());
    let pos = BlockPosition::new(x, y, z);
    if region.get_block(pos).is_air() && chunk_bb.is_inside(pos) {
        let rotation = Rotation::get_random(&mut positional_random);
        region.set_block(pos, Block::DRIED_GHAST.default_state().rotate(rotation));
    }
}
