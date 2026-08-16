use crate::util::geometry::BoundingBox;
use crate::util::rng::RandomSource;
use crate::worldgen::structure::pieces::pieces_container::PiecesContainer;
use crate::worldgen::structure::structure_piece::{StructurePiece, create_bounding_box};

#[derive(Debug, Default)]
pub struct StructurePiecesBuilder {
    pieces: Vec<StructurePiece>,
}

impl StructurePiecesBuilder {
    pub const fn new() -> Self {
        Self { pieces: Vec::new() }
    }

    pub fn add_piece(&mut self, piece: StructurePiece) {
        self.pieces.push(piece);
    }

    pub fn find_collision_piece(&self, box_: BoundingBox) -> Option<&StructurePiece> {
        self.pieces
            .iter()
            .find(|piece| piece.bounding_box().intersects(box_))
    }

    pub fn offset_pieces_vertically(&mut self, dy: i32) {
        for piece in &mut self.pieces {
            piece.move_by(0, dy, 0);
        }
    }

    pub fn move_inside_heights(
        &mut self,
        random: &mut RandomSource,
        lowest_allowed: i32,
        highest_allowed: i32,
    ) {
        let bounding_box = self.get_bounding_box();
        let height_span = highest_allowed - lowest_allowed + 1 - bounding_box.y_span();
        let y0_pos = if height_span > 1 {
            lowest_allowed + random.next_i32_bounded(height_span)
        } else {
            lowest_allowed
        };
        let dy = y0_pos - bounding_box.min_y();
        self.offset_pieces_vertically(dy);
    }

    pub fn build(self) -> PiecesContainer {
        PiecesContainer::new(self.pieces)
    }

    pub fn clear(&mut self) {
        self.pieces.clear();
    }

    pub const fn is_empty(&self) -> bool {
        self.pieces.is_empty()
    }

    pub fn get_bounding_box(&self) -> BoundingBox {
        create_bounding_box(&self.pieces).expect("Unable to calculate boundingbox without pieces")
    }
}
