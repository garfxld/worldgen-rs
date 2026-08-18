use crate::registry::blocks::transform;
use crate::registry::blocks::vanilla_blocks;
use crate::util::geometry::BoundingBox;
use crate::util::ident::Identifier;
use crate::util::mirror::Mirror;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::util::rng::legacy_random::LegacyRandomSource;
use crate::util::rotation::Rotation;
use crate::worldgen::block_predicate::HolderSet;
use crate::worldgen::region::WorldGenRegion;
use crate::worldgen::structure::pieces::structure_pieces_builder::StructurePiecesBuilder;
use crate::worldgen::structure::structure_piece::{PieceBase, PieceKind, StructurePiece};
use crate::worldgen::structure::template_structure_piece::TemplateStructurePiece;
use crate::worldgen::structure::templatesystem::structure_place_settings::StructurePlaceSettings;
use crate::worldgen::structure::templatesystem::structure_processor::StructureProcessor;

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
            Identifier::const_vanilla("structure_block"),
            Identifier::const_vanilla("air"),
        ]),
    });
    settings
}

pub fn add_pieces(
    builder: &mut StructurePiecesBuilder,
    random: &mut RandomSource,
    position: BlockPos,
) {
    let rotation = Rotation::get_random(random);
    let index = random.next_i32_bounded(FOSSILS.len() as i32) as usize;
    let location: Identifier = format!("minecraft:{}", FOSSILS[index])
        .parse()
        .expect("nether fossil template path should be a valid identifier");
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
    fn new(location: &Identifier, position: BlockPos, rotation: Rotation) -> StructurePiece {
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
        reference_pos: BlockPos,
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
    let pos = BlockPos::new(x, y, z);
    if region.get_block(pos).is_air() && chunk_bb.is_inside(pos) {
        let rotation = Rotation::get_random(&mut positional_random);
        region.set_block(
            pos,
            transform::rotate(vanilla_blocks::DRIED_GHAST.default_state(), rotation),
        );
    }
}
