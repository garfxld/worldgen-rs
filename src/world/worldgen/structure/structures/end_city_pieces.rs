use shulkr::util::Key;
use shulkr::world::block::Block;

use shulkr::util::BoundingBox;

use crate::util::rng::RandomSource;
use crate::world::worldgen::block_predicate::HolderSet;
use crate::world::worldgen::region::WorldGenRegion;
use crate::world::worldgen::structure::structure_piece::{PieceBase, PieceKind, StructurePiece};
use crate::world::worldgen::structure::template_structure_piece::TemplateStructurePiece;
use crate::world::worldgen::structure::templatesystem::structure_place_settings::StructurePlaceSettings;
use crate::world::worldgen::structure::templatesystem::structure_processor::StructureProcessor;
use crate::world::worldgen::structure::templatesystem::structure_template::calculate_relative_position;
use shulkr::util::BlockPosition;
use shulkr::world::block::Rotation;

const MAX_GEN_DEPTH: i32 = 8;

const TOWER_BRIDGES: [(Rotation, BlockPosition); 4] = [
    (Rotation::None, BlockPosition::new(1, -1, 0)),
    (Rotation::Clockwise90, BlockPosition::new(6, -1, 1)),
    (Rotation::CounterClockwise90, BlockPosition::new(0, -1, 5)),
    (Rotation::Clockwise180, BlockPosition::new(5, -1, 6)),
];

const FAT_TOWER_BRIDGES: [(Rotation, BlockPosition); 4] = [
    (Rotation::None, BlockPosition::new(4, -1, 0)),
    (Rotation::Clockwise90, BlockPosition::new(12, -1, 4)),
    (Rotation::CounterClockwise90, BlockPosition::new(0, -1, 8)),
    (Rotation::Clockwise180, BlockPosition::new(8, -1, 12)),
];

#[derive(Debug, Clone, Copy)]
enum SectionGenerator {
    HouseTower,
    Tower,
    TowerBridge,
    FatTower,
}

#[derive(Clone)]
struct Anchor {
    place_settings: StructurePlaceSettings,
    template_position: BlockPosition,
    gen_depth: i32,
}

fn make_identifier(template_name: &str) -> Key {
    Key::of(format!("minecraft:end_city/{template_name}"))
}

fn make_settings(overwrite: bool, rotation: Rotation) -> StructurePlaceSettings {
    let ignored = if overwrite {
        vec![Key::const_vanilla("structure_block")]
    } else {
        vec![
            Key::const_vanilla("structure_block"),
            Key::const_vanilla("air"),
        ]
    };
    let mut settings = StructurePlaceSettings::new()
        .set_ignore_entities(true)
        .set_rotation(rotation);
    settings.add_processor(StructureProcessor::BlockIgnore {
        blocks: HolderSet::Entries(ignored),
    });
    settings
}

fn new_piece(
    template_name: &str,
    position: BlockPosition,
    rotation: Rotation,
    overwrite: bool,
) -> StructurePiece {
    let location = make_identifier(template_name);
    let (base, template) =
        TemplateStructurePiece::new(0, &location, make_settings(overwrite, rotation), position);
    StructurePiece::new(base, PieceKind::EndCity(EndCityPiece { template }))
}

fn add_piece(
    parent: &Anchor,
    offset: BlockPosition,
    template_name: &str,
    rotation: Rotation,
    overwrite: bool,
) -> StructurePiece {
    let mut child = new_piece(template_name, parent.template_position, rotation, overwrite);
    let child_settings = match &child.kind {
        PieceKind::EndCity(piece) => piece.template.place_settings.clone(),
        _ => unreachable!(),
    };
    let marker_1 = calculate_relative_position(&parent.place_settings, offset);
    let marker_2 = calculate_relative_position(&child_settings, BlockPosition::new(0, 0, 0));
    let origin = BlockPosition::new(
        marker_1.x() - marker_2.x(),
        marker_1.y() - marker_2.y(),
        marker_1.z() - marker_2.z(),
    );
    child.move_by(origin.x(), origin.y(), origin.z());
    child
}

fn anchor_of(piece: &StructurePiece) -> Anchor {
    let PieceKind::EndCity(end_city) = &piece.kind else {
        unreachable!()
    };
    Anchor {
        place_settings: end_city.template.place_settings.clone(),
        template_position: end_city.template.template_position,
        gen_depth: piece.base.gen_depth,
    }
}

fn add_helper(pieces: &mut Vec<StructurePiece>, piece: StructurePiece) -> (usize, Anchor) {
    let anchor = anchor_of(&piece);
    pieces.push(piece);
    (pieces.len() - 1, anchor)
}

pub fn start_house_tower(
    origin: BlockPosition,
    rotation: Rotation,
    pieces: &mut Vec<StructurePiece>,
    random: &mut RandomSource,
) {
    let mut ship_created = false;
    let (_, mut last) = add_helper(pieces, new_piece("base_floor", origin, rotation, true));
    let piece = add_piece(
        &last,
        BlockPosition::new(-1, 0, -1),
        "second_floor_1",
        rotation,
        false,
    );
    (_, last) = add_helper(pieces, piece);
    let piece = add_piece(
        &last,
        BlockPosition::new(-1, 4, -1),
        "third_floor_1",
        rotation,
        false,
    );
    (_, last) = add_helper(pieces, piece);
    let piece = add_piece(
        &last,
        BlockPosition::new(-1, 8, -1),
        "third_roof",
        rotation,
        true,
    );
    (_, last) = add_helper(pieces, piece);
    recursive_children(
        SectionGenerator::Tower,
        1,
        &last,
        None,
        pieces,
        random,
        &mut ship_created,
    );
}

fn recursive_children(
    generator: SectionGenerator,
    gen_depth: i32,
    parent: &Anchor,
    offset: Option<BlockPosition>,
    pieces: &mut Vec<StructurePiece>,
    random: &mut RandomSource,
    ship_created: &mut bool,
) -> bool {
    if gen_depth > MAX_GEN_DEPTH {
        return false;
    }

    let mut child_pieces = Vec::new();
    if !generate(
        generator,
        gen_depth,
        parent,
        offset,
        &mut child_pieces,
        random,
        ship_created,
    ) {
        return false;
    }

    let child_tag = random.next_i32();
    let mut collision = false;
    for child in &mut child_pieces {
        child.base.gen_depth = child_tag;
        let child_box = child.bounding_box();
        if let Some(existing) = pieces
            .iter()
            .find(|piece| piece.bounding_box().intersects(child_box))
            && existing.base.gen_depth != parent.gen_depth
        {
            collision = true;
            break;
        }
    }

    if collision {
        return false;
    }
    pieces.append(&mut child_pieces);
    true
}

fn generate(
    generator: SectionGenerator,
    gen_depth: i32,
    parent: &Anchor,
    offset: Option<BlockPosition>,
    pieces: &mut Vec<StructurePiece>,
    random: &mut RandomSource,
    ship_created: &mut bool,
) -> bool {
    match generator {
        SectionGenerator::HouseTower => {
            generate_house_tower(gen_depth, parent, offset, pieces, random, ship_created)
        }
        SectionGenerator::Tower => generate_tower(gen_depth, parent, pieces, random, ship_created),
        SectionGenerator::TowerBridge => {
            generate_tower_bridge(gen_depth, parent, pieces, random, ship_created)
        }
        SectionGenerator::FatTower => {
            generate_fat_tower(gen_depth, parent, pieces, random, ship_created)
        }
    }
}

fn generate_house_tower(
    gen_depth: i32,
    parent: &Anchor,
    offset: Option<BlockPosition>,
    pieces: &mut Vec<StructurePiece>,
    random: &mut RandomSource,
    ship_created: &mut bool,
) -> bool {
    if gen_depth > MAX_GEN_DEPTH {
        return false;
    }

    let rotation = parent.place_settings.rotation();
    let offset = offset.unwrap_or(BlockPosition::new(0, 0, 0));
    let piece = add_piece(parent, offset, "base_floor", rotation, true);
    let (_, mut last) = add_helper(pieces, piece);

    let num_floors = random.next_i32_bounded(3);
    if num_floors == 0 {
        let piece = add_piece(
            &last,
            BlockPosition::new(-1, 4, -1),
            "base_roof",
            rotation,
            true,
        );
        (_, last) = add_helper(pieces, piece);
        let _ = last;
    } else if num_floors == 1 {
        let piece = add_piece(
            &last,
            BlockPosition::new(-1, 0, -1),
            "second_floor_2",
            rotation,
            false,
        );
        (_, last) = add_helper(pieces, piece);
        let piece = add_piece(
            &last,
            BlockPosition::new(-1, 8, -1),
            "second_roof",
            rotation,
            false,
        );
        (_, last) = add_helper(pieces, piece);
        recursive_children(
            SectionGenerator::Tower,
            gen_depth + 1,
            &last,
            None,
            pieces,
            random,
            ship_created,
        );
    } else if num_floors == 2 {
        let piece = add_piece(
            &last,
            BlockPosition::new(-1, 0, -1),
            "second_floor_2",
            rotation,
            false,
        );
        (_, last) = add_helper(pieces, piece);
        let piece = add_piece(
            &last,
            BlockPosition::new(-1, 4, -1),
            "third_floor_2",
            rotation,
            false,
        );
        (_, last) = add_helper(pieces, piece);
        let piece = add_piece(
            &last,
            BlockPosition::new(-1, 8, -1),
            "third_roof",
            rotation,
            true,
        );
        (_, last) = add_helper(pieces, piece);
        recursive_children(
            SectionGenerator::Tower,
            gen_depth + 1,
            &last,
            None,
            pieces,
            random,
            ship_created,
        );
    }

    true
}

fn generate_tower(
    gen_depth: i32,
    parent: &Anchor,
    pieces: &mut Vec<StructurePiece>,
    random: &mut RandomSource,
    ship_created: &mut bool,
) -> bool {
    let rotation = parent.place_settings.rotation();
    let base_offset = BlockPosition::new(
        3 + random.next_i32_bounded(2),
        -3,
        3 + random.next_i32_bounded(2),
    );
    let piece = add_piece(parent, base_offset, "tower_base", rotation, true);
    let (_, mut last) = add_helper(pieces, piece);
    let piece = add_piece(
        &last,
        BlockPosition::new(0, 7, 0),
        "tower_piece",
        rotation,
        true,
    );
    (_, last) = add_helper(pieces, piece);

    let mut bridge_piece = (random.next_i32_bounded(3) == 0).then(|| last.clone());
    let tower_height = 1 + random.next_i32_bounded(3);

    for i in 0..tower_height {
        let piece = add_piece(
            &last,
            BlockPosition::new(0, 4, 0),
            "tower_piece",
            rotation,
            true,
        );
        (_, last) = add_helper(pieces, piece);
        if i < tower_height - 1 && random.next_bool() {
            bridge_piece = Some(last.clone());
        }
    }

    if let Some(bridge_piece) = bridge_piece {
        for (bridge_rotation, bridge_offset) in TOWER_BRIDGES {
            if random.next_bool() {
                let piece = add_piece(
                    &bridge_piece,
                    bridge_offset,
                    "bridge_end",
                    rotation.then(bridge_rotation),
                    true,
                );
                let (_, bridge_start) = add_helper(pieces, piece);
                recursive_children(
                    SectionGenerator::TowerBridge,
                    gen_depth + 1,
                    &bridge_start,
                    None,
                    pieces,
                    random,
                    ship_created,
                );
            }
        }
        let piece = add_piece(
            &last,
            BlockPosition::new(-1, 4, -1),
            "tower_top",
            rotation,
            true,
        );
        add_helper(pieces, piece);
    } else {
        if gen_depth != 7 {
            return recursive_children(
                SectionGenerator::FatTower,
                gen_depth + 1,
                &last,
                None,
                pieces,
                random,
                ship_created,
            );
        }
        let piece = add_piece(
            &last,
            BlockPosition::new(-1, 4, -1),
            "tower_top",
            rotation,
            true,
        );
        add_helper(pieces, piece);
    }

    true
}

fn generate_tower_bridge(
    gen_depth: i32,
    parent: &Anchor,
    pieces: &mut Vec<StructurePiece>,
    random: &mut RandomSource,
    ship_created: &mut bool,
) -> bool {
    let rotation = parent.place_settings.rotation();
    let bridge_length = random.next_i32_bounded(4) + 1;
    let piece = add_piece(
        parent,
        BlockPosition::new(0, 0, -4),
        "bridge_piece",
        rotation,
        true,
    );
    let (mut index, mut last) = add_helper(pieces, piece);
    pieces[index].base.gen_depth = -1;
    let mut next_y = 0;

    for _ in 0..bridge_length {
        if random.next_bool() {
            let piece = add_piece(
                &last,
                BlockPosition::new(0, next_y, -4),
                "bridge_piece",
                rotation,
                true,
            );
            (index, last) = add_helper(pieces, piece);
            next_y = 0;
        } else {
            if random.next_bool() {
                let piece = add_piece(
                    &last,
                    BlockPosition::new(0, next_y, -4),
                    "bridge_steep_stairs",
                    rotation,
                    true,
                );
                (index, last) = add_helper(pieces, piece);
            } else {
                let piece = add_piece(
                    &last,
                    BlockPosition::new(0, next_y, -8),
                    "bridge_gentle_stairs",
                    rotation,
                    true,
                );
                (index, last) = add_helper(pieces, piece);
            }
            next_y = 4;
        }
    }
    let _ = index;

    if !*ship_created && random.next_i32_bounded(10 - gen_depth) == 0 {
        let ship_offset = BlockPosition::new(
            -8 + random.next_i32_bounded(8),
            next_y,
            -70 + random.next_i32_bounded(10),
        );
        let piece = add_piece(&last, ship_offset, "ship", rotation, true);
        add_helper(pieces, piece);
        *ship_created = true;
    } else if !recursive_children(
        SectionGenerator::HouseTower,
        gen_depth + 1,
        &last,
        Some(BlockPosition::new(-3, next_y + 1, -11)),
        pieces,
        random,
        ship_created,
    ) {
        return false;
    }

    let piece = add_piece(
        &last,
        BlockPosition::new(4, next_y, 0),
        "bridge_end",
        rotation.then(Rotation::Clockwise180),
        true,
    );
    let (end_index, _) = add_helper(pieces, piece);
    pieces[end_index].base.gen_depth = -1;
    true
}

fn generate_fat_tower(
    gen_depth: i32,
    parent: &Anchor,
    pieces: &mut Vec<StructurePiece>,
    random: &mut RandomSource,
    ship_created: &mut bool,
) -> bool {
    let rotation = parent.place_settings.rotation();
    let piece = add_piece(
        parent,
        BlockPosition::new(-3, 4, -3),
        "fat_tower_base",
        rotation,
        true,
    );
    let (_, mut last) = add_helper(pieces, piece);
    let piece = add_piece(
        &last,
        BlockPosition::new(0, 4, 0),
        "fat_tower_middle",
        rotation,
        true,
    );
    (_, last) = add_helper(pieces, piece);

    let mut i = 0;
    while i < 2 && random.next_i32_bounded(3) != 0 {
        let piece = add_piece(
            &last,
            BlockPosition::new(0, 8, 0),
            "fat_tower_middle",
            rotation,
            true,
        );
        (_, last) = add_helper(pieces, piece);

        for (bridge_rotation, bridge_offset) in FAT_TOWER_BRIDGES {
            if random.next_bool() {
                let piece = add_piece(
                    &last,
                    bridge_offset,
                    "bridge_end",
                    rotation.then(bridge_rotation),
                    true,
                );
                let (_, bridge_start) = add_helper(pieces, piece);
                recursive_children(
                    SectionGenerator::TowerBridge,
                    gen_depth + 1,
                    &bridge_start,
                    None,
                    pieces,
                    random,
                    ship_created,
                );
            }
        }
        i += 1;
    }

    let piece = add_piece(
        &last,
        BlockPosition::new(-2, 8, -2),
        "fat_tower_top",
        rotation,
        true,
    );
    add_helper(pieces, piece);
    true
}

#[derive(Debug)]
pub struct EndCityPiece {
    pub template: TemplateStructurePiece,
}

impl EndCityPiece {
    pub fn post_process(
        &mut self,
        base: &mut PieceBase,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        chunk_bb: BoundingBox,
        reference_pos: BlockPosition,
    ) {
        self.template.post_process(
            base,
            region,
            random,
            chunk_bb,
            reference_pos,
            |marker_id, position, region, random, chunk_bb| {
                if marker_id.starts_with("Chest") {
                    let chest_position = position.below();
                    if chunk_bb.is_inside(chest_position)
                        && region.get_block(chest_position).is(Block::CHEST)
                    {
                        random.next_i64();
                    }
                }
            },
        );
    }
}
