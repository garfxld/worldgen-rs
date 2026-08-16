use crate::util::geometry::BoundingBox;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::util::rotation::Rotation;
use crate::worldgen::region::WorldGenRegion;
use crate::worldgen::structure::pools::jigsaw_junction::JigsawJunction;
use crate::worldgen::structure::pools::structure_pool_element::StructurePoolElement;
use crate::worldgen::structure::templatesystem::liquid_settings::LiquidSettings;

#[derive(Debug)]
pub struct PoolElementStructurePiece {
    pub element: &'static StructurePoolElement,
    pub position: BlockPos,
    pub ground_level_delta: i32,
    pub rotation: Rotation,
    pub bounding_box: BoundingBox,
    pub liquid_settings: LiquidSettings,
    pub junctions: Vec<JigsawJunction>,
}

impl PoolElementStructurePiece {
    pub const fn new(
        element: &'static StructurePoolElement,
        position: BlockPos,
        ground_level_delta: i32,
        rotation: Rotation,
        bounding_box: BoundingBox,
        liquid_settings: LiquidSettings,
    ) -> Self {
        Self {
            element,
            position,
            ground_level_delta,
            rotation,
            bounding_box,
            liquid_settings,
            junctions: Vec::new(),
        }
    }

    pub fn add_junction(&mut self, junction: JigsawJunction) {
        self.junctions.push(junction);
    }
}

#[derive(Debug)]
pub enum StructurePiece {
    PoolElement(PoolElementStructurePiece),
}

impl StructurePiece {
    pub const fn bounding_box(&self) -> BoundingBox {
        match self {
            Self::PoolElement(piece) => piece.bounding_box,
        }
    }

    pub const fn move_by(&mut self, dx: i32, dy: i32, dz: i32) {
        match self {
            Self::PoolElement(piece) => {
                piece.bounding_box = piece.bounding_box.moved(dx, dy, dz);
                piece.position = piece.position.offset(dx, dy, dz);
            }
        }
    }

    pub fn post_process(
        &self,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        chunk_bb: BoundingBox,
        reference_pos: BlockPos,
    ) {
        match self {
            Self::PoolElement(piece) => {
                piece.element.place(
                    region,
                    piece.position,
                    reference_pos,
                    piece.rotation,
                    chunk_bb,
                    random,
                    piece.liquid_settings,
                    false,
                );
            }
        }
    }
}

pub fn create_bounding_box<'a>(
    pieces: impl IntoIterator<Item = &'a StructurePiece>,
) -> Option<BoundingBox> {
    pieces
        .into_iter()
        .map(StructurePiece::bounding_box)
        .reduce(BoundingBox::encapsulate_box)
}
