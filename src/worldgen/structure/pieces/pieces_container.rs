use crate::util::geometry::BoundingBox;
use crate::util::pos::BlockPos;
use crate::worldgen::structure::structure_piece::{StructurePiece, create_bounding_box};

#[derive(Debug, Default)]
pub struct PiecesContainer {
    pieces: Vec<StructurePiece>,
}

impl PiecesContainer {
    pub const fn new(pieces: Vec<StructurePiece>) -> Self {
        Self { pieces }
    }

    pub fn pieces(&self) -> &[StructurePiece] {
        &self.pieces
    }

    pub const fn is_empty(&self) -> bool {
        self.pieces.is_empty()
    }

    pub fn is_inside_piece(&self, start_pos: BlockPos) -> bool {
        self.pieces
            .iter()
            .any(|piece| piece.bounding_box().is_inside(start_pos))
    }

    pub fn calculate_bounding_box(&self) -> BoundingBox {
        create_bounding_box(&self.pieces)
            .expect("Unable to calculate boundingbox without pieces")
    }
}
