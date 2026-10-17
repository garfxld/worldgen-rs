use shulkr::world::block::Block;

use crate::util::rng::legacy_random::LegacyRandomSource;
use crate::world::worldgen::region::WorldGenRegion;
use crate::world::worldgen::structure::pieces::pieces_container::PiecesContainer;
use crate::world::worldgen::structure::structure_piece::PieceKind;
use shulkr::util::BlockPosition;
use shulkr::util::BoundingBox;
use std::cmp::Ordering;

pub fn after_place(region: &WorldGenRegion<'_>, chunk_bb: BoundingBox, pieces: &PiecesContainer) {
    let mut unique_sand_placements: Vec<BlockPosition> = Vec::new();

    for piece in pieces.pieces() {
        let PieceKind::DesertPyramid(pyramid) = &piece.kind else {
            continue;
        };
        for pos in pyramid.potential_suspicious_sand_world_positions() {
            insert_sorted(&mut unique_sand_placements, *pos);
        }
        place_suspicious_sand(chunk_bb, region, pyramid.random_collapsed_roof_pos());
    }

    let mut shuffled = unique_sand_placements.clone();
    let center = pieces.calculate_bounding_box().center();
    let mut positional_random = LegacyRandomSource::from_seed(region.seed() as u64)
        .next_positional()
        .at(center.x(), center.y(), center.z());

    for i in (2..=shuffled.len()).rev() {
        let swap_to = positional_random.next_i32_bounded(i as i32) as usize;
        shuffled.swap(i - 1, swap_to);
    }

    let mut suspicious_sand_to_place = unique_sand_placements
        .len()
        .min(positional_random.next_i32_between_exclusive(5, 8) as usize);

    for pos in shuffled {
        if suspicious_sand_to_place > 0 {
            suspicious_sand_to_place -= 1;
            place_suspicious_sand(chunk_bb, region, pos);
        } else if chunk_bb.is_inside(pos) {
            region.set_block(pos, Block::SAND.default_state());
        }
    }
}

fn place_suspicious_sand(chunk_bb: BoundingBox, region: &WorldGenRegion<'_>, pos: BlockPosition) {
    if chunk_bb.is_inside(pos) {
        region.set_block(pos, Block::SUSPICIOUS_SAND.default_state());
    }
}

fn insert_sorted(positions: &mut Vec<BlockPosition>, pos: BlockPosition) {
    match positions.binary_search_by(|entry| compare_vec3i(*entry, pos)) {
        Ok(_) => {}
        Err(index) => positions.insert(index, pos),
    }
}

const fn compare_vec3i(a: BlockPosition, b: BlockPosition) -> Ordering {
    a.y()
        .cmp(&b.y())
        .then(a.z().cmp(&b.z()))
        .then(a.x().cmp(&b.x()))
}
