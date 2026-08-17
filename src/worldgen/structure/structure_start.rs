use crate::util::geometry::BoundingBox;
use crate::util::pos::{BlockPos, ChunkPos};
use crate::util::rng::RandomSource;
use crate::worldgen::region::WorldGenRegion;
use crate::worldgen::structure::pieces::pieces_container::PiecesContainer;
use crate::worldgen::structure::structure::Structure;
use crate::worldgen::structure::structure_piece::StructurePiece;

#[derive(Debug)]
pub struct StructureStart {
    structure: Option<&'static Structure>,
    piece_container: PiecesContainer,
    chunk_pos: ChunkPos,
    references: i32,
    cached_bounding_box: Option<BoundingBox>,
}

impl StructureStart {
    pub const fn new(
        structure: Option<&'static Structure>,
        chunk_pos: ChunkPos,
        references: i32,
        piece_container: PiecesContainer,
    ) -> Self {
        Self {
            structure,
            piece_container,
            chunk_pos,
            references,
            cached_bounding_box: None,
        }
    }

    pub fn invalid() -> Self {
        Self::new(None, ChunkPos::new(0, 0), 0, PiecesContainer::default())
    }

    pub fn get_bounding_box(&mut self) -> BoundingBox {
        if let Some(bounding_box) = self.cached_bounding_box {
            return bounding_box;
        }
        let bounding_box = self
            .structure
            .expect("bounding box of an invalid structure start")
            .adjust_bounding_box(self.piece_container.calculate_bounding_box());
        self.cached_bounding_box = Some(bounding_box);
        bounding_box
    }

    pub fn bounding_box(&self) -> BoundingBox {
        self.cached_bounding_box.unwrap_or_else(|| {
            self.structure
                .expect("bounding box of an invalid structure start")
                .adjust_bounding_box(self.piece_container.calculate_bounding_box())
        })
    }

    pub fn place_in_chunk(
        &mut self,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        chunk_bb: BoundingBox,
    ) {
        let pieces = self.piece_container.pieces_mut();
        let Some(first) = pieces.first() else {
            return;
        };

        let center_bb = first.bounding_box();
        let center_pos = center_bb.center();
        let reference_pos = BlockPos::new(center_pos.x(), center_bb.min_y(), center_pos.z());

        for piece in pieces {
            if piece.bounding_box().intersects(chunk_bb) {
                piece.post_process(region, random, chunk_bb, reference_pos);
            }
        }

        if let Some(structure) = self.structure {
            structure.after_place(region, chunk_bb, &self.piece_container);
        }
    }

    pub const fn is_valid(&self) -> bool {
        !self.piece_container.is_empty()
    }

    pub const fn chunk_pos(&self) -> ChunkPos {
        self.chunk_pos
    }

    pub const fn structure(&self) -> Option<&'static Structure> {
        self.structure
    }

    pub const fn references(&self) -> i32 {
        self.references
    }

    pub const fn add_reference(&mut self) {
        self.references += 1;
    }

    pub const fn can_be_referenced(&self) -> bool {
        self.references < 1
    }

    pub fn pieces(&self) -> &[StructurePiece] {
        self.piece_container.pieces()
    }
}
