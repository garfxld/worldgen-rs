use crate::world::worldgen::structure::structure_piece::{StructurePiece, create_bounding_box};
use shulkr::util::BlockPosition;
use shulkr::util::BoundingBox;

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

    pub const fn pieces_mut(&mut self) -> &mut [StructurePiece] {
        self.pieces.as_mut_slice()
    }

    pub const fn is_empty(&self) -> bool {
        self.pieces.is_empty()
    }

    pub fn is_inside_piece(&self, start_pos: BlockPosition) -> bool {
        self.pieces
            .iter()
            .any(|piece| piece.bounding_box().is_inside(start_pos))
    }

    pub fn calculate_bounding_box(&self) -> BoundingBox {
        create_bounding_box(&self.pieces).expect("Unable to calculate boundingbox without pieces")
    }
}
