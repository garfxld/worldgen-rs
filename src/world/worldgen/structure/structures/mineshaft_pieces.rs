use shulkr::util::Key;
use shulkr::util::SupportType;
use shulkr::world::block::{Block, BlockState, Properties, RailShape};

use shulkr::registry::registries;
use shulkr::util::Axis;
use shulkr::util::BoundingBox;
use shulkr::util::Direction;

use crate::util::rng::RandomSource;
use crate::world::worldgen::region::WorldGenRegion;
use crate::world::worldgen::structure::pieces::structure_pieces_builder::StructurePiecesBuilder;
use crate::world::worldgen::structure::structure_piece::{
    CanBeReplaced, PieceBase, PieceKind, StructurePiece, cave_air, is_replaceable_by_structures,
};
use shulkr::util::BlockPosition;

const MAX_PILLAR_HEIGHT: i32 = 20;
const MAX_CHAIN_HEIGHT: i32 = 50;
const MAX_DEPTH: i32 = 8;
pub const MAGIC_START_Y: i32 = 50;

const MINESHAFT_BLOCKING: Key = Key::const_vanilla("mineshaft_blocking");
const UNSTABLE_BOTTOM_CENTER: Key = Key::const_vanilla("unstable_bottom_center");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MineshaftType {
    Normal,
    Mesa,
}

impl MineshaftType {
    pub const fn parse(value: &str) -> Self {
        if value == "mesa" {
            Self::Mesa
        } else {
            Self::Normal
        }
    }

    pub const fn wood_state(self) -> BlockState {
        match self {
            Self::Normal => Block::OAK_LOG.default_state(),
            Self::Mesa => Block::DARK_OAK_LOG.default_state(),
        }
    }

    pub const fn planks_state(self) -> BlockState {
        match self {
            Self::Normal => Block::OAK_PLANKS.default_state(),
            Self::Mesa => Block::DARK_OAK_PLANKS.default_state(),
        }
    }

    pub const fn fence_state(self) -> BlockState {
        match self {
            Self::Normal => Block::OAK_FENCE.default_state(),
            Self::Mesa => Block::DARK_OAK_FENCE.default_state(),
        }
    }
}

#[derive(Debug)]
pub enum MineShaftPieceKind {
    Corridor {
        has_rails: bool,
        spider_corridor: bool,
        has_placed_spider: bool,
        num_sections: i32,
    },
    Crossing {
        direction: Direction,
        is_two_floored: bool,
    },
    Room {
        child_entrance_boxes: Vec<BoundingBox>,
    },
    Stairs,
}

#[derive(Debug)]
pub struct MineShaftPiece {
    pub kind: MineShaftPieceKind,
    pub mineshaft_type: MineshaftType,
}

const fn make_piece(
    kind: MineShaftPieceKind,
    mineshaft_type: MineshaftType,
    gen_depth: i32,
    bounding_box: BoundingBox,
    orientation: Option<Direction>,
) -> StructurePiece {
    let mut base = PieceBase::new(gen_depth, bounding_box);
    base.can_be_replaced = CanBeReplaced::MineshaftWood(mineshaft_type);
    if let Some(direction) = orientation {
        base.set_orientation(Some(direction));
    }
    StructurePiece::new(
        base,
        PieceKind::Mineshaft(MineShaftPiece {
            kind,
            mineshaft_type,
        }),
    )
}

pub fn new_room(
    gen_depth: i32,
    random: &mut RandomSource,
    west: i32,
    north: i32,
    mineshaft_type: MineshaftType,
) -> StructurePiece {
    let bounding_box = BoundingBox::new(
        west,
        MAGIC_START_Y,
        north,
        west + 7 + random.next_i32_bounded(6),
        54 + random.next_i32_bounded(6),
        north + 7 + random.next_i32_bounded(6),
    );
    make_piece(
        MineShaftPieceKind::Room {
            child_entrance_boxes: Vec::new(),
        },
        mineshaft_type,
        gen_depth,
        bounding_box,
        None,
    )
}

fn find_corridor_size(
    builder: &StructurePiecesBuilder,
    random: &mut RandomSource,
    foot_x: i32,
    foot_y: i32,
    foot_z: i32,
    direction: Direction,
) -> Option<BoundingBox> {
    let mut corridor_length = random.next_i32_bounded(3) + 2;
    while corridor_length > 0 {
        let block_length = corridor_length * 5;
        let box_ = match direction {
            Direction::South => BoundingBox::new(0, 0, 0, 2, 2, block_length - 1),
            Direction::West => BoundingBox::new(-(block_length - 1), 0, 0, 0, 2, 2),
            Direction::East => BoundingBox::new(0, 0, 0, block_length - 1, 2, 2),
            _ => BoundingBox::new(0, 0, -(block_length - 1), 2, 2, 0),
        }
        .moved(foot_x, foot_y, foot_z);
        if builder.find_collision_piece(box_).is_none() {
            return Some(box_);
        }
        corridor_length -= 1;
    }
    None
}

fn find_crossing(
    builder: &StructurePiecesBuilder,
    random: &mut RandomSource,
    foot_x: i32,
    foot_y: i32,
    foot_z: i32,
    direction: Direction,
) -> Option<BoundingBox> {
    let y1 = if random.next_i32_bounded(4) == 0 {
        6
    } else {
        2
    };
    let box_ = match direction {
        Direction::South => BoundingBox::new(-1, 0, 0, 3, y1, 4),
        Direction::West => BoundingBox::new(-4, 0, -1, 0, y1, 3),
        Direction::East => BoundingBox::new(0, 0, -1, 4, y1, 3),
        _ => BoundingBox::new(-1, 0, -4, 3, y1, 0),
    }
    .moved(foot_x, foot_y, foot_z);
    builder.find_collision_piece(box_).is_none().then_some(box_)
}

fn find_stairs(
    builder: &StructurePiecesBuilder,
    foot_x: i32,
    foot_y: i32,
    foot_z: i32,
    direction: Direction,
) -> Option<BoundingBox> {
    let box_ = match direction {
        Direction::South => BoundingBox::new(0, -5, 0, 2, 2, 8),
        Direction::West => BoundingBox::new(-8, -5, 0, 0, 2, 2),
        Direction::East => BoundingBox::new(0, -5, 0, 8, 2, 2),
        _ => BoundingBox::new(0, -5, -8, 2, 2, 0),
    }
    .moved(foot_x, foot_y, foot_z);
    builder.find_collision_piece(box_).is_none().then_some(box_)
}

#[expect(
    clippy::too_many_arguments,
    reason = "mirrors vanilla createRandomShaftPiece"
)]
fn create_random_shaft_piece(
    builder: &StructurePiecesBuilder,
    random: &mut RandomSource,
    foot_x: i32,
    foot_y: i32,
    foot_z: i32,
    direction: Direction,
    gen_depth: i32,
    mineshaft_type: MineshaftType,
) -> Option<StructurePiece> {
    let random_selection = random.next_i32_bounded(100);
    if random_selection >= 80 {
        let box_ = find_crossing(builder, random, foot_x, foot_y, foot_z, direction)?;
        Some(make_piece(
            MineShaftPieceKind::Crossing {
                direction,
                is_two_floored: box_.y_span() > 3,
            },
            mineshaft_type,
            gen_depth,
            box_,
            None,
        ))
    } else if random_selection >= 70 {
        let box_ = find_stairs(builder, foot_x, foot_y, foot_z, direction)?;
        Some(make_piece(
            MineShaftPieceKind::Stairs,
            mineshaft_type,
            gen_depth,
            box_,
            Some(direction),
        ))
    } else {
        let box_ = find_corridor_size(builder, random, foot_x, foot_y, foot_z, direction)?;
        let has_rails = random.next_i32_bounded(3) == 0;
        let spider_corridor = !has_rails && random.next_i32_bounded(23) == 0;
        let num_sections = if direction.get_axis() == Axis::Z {
            box_.z_span() / 5
        } else {
            box_.x_span() / 5
        };
        Some(make_piece(
            MineShaftPieceKind::Corridor {
                has_rails,
                spider_corridor,
                has_placed_spider: false,
                num_sections,
            },
            mineshaft_type,
            gen_depth,
            box_,
            Some(direction),
        ))
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "mirrors vanilla generateAndAddPiece"
)]
fn generate_and_add_piece(
    start_bb: BoundingBox,
    mineshaft_type: MineshaftType,
    builder: &mut StructurePiecesBuilder,
    random: &mut RandomSource,
    foot_x: i32,
    foot_y: i32,
    foot_z: i32,
    direction: Direction,
    depth: i32,
) -> Option<BoundingBox> {
    if depth > MAX_DEPTH {
        return None;
    }
    if (foot_x - start_bb.min_x()).abs() > 80 || (foot_z - start_bb.min_z()).abs() > 80 {
        return None;
    }

    let piece = create_random_shaft_piece(
        builder,
        random,
        foot_x,
        foot_y,
        foot_z,
        direction,
        depth + 1,
        mineshaft_type,
    )?;
    let child_bb = piece.bounding_box();
    let index = builder.add_piece_indexed(piece);
    add_children(index, start_bb, mineshaft_type, builder, random);
    Some(child_bb)
}

pub fn add_children(
    index: usize,
    start_bb: BoundingBox,
    mineshaft_type: MineshaftType,
    builder: &mut StructurePiecesBuilder,
    random: &mut RandomSource,
) {
    let piece = builder.piece_at(index);
    let box_ = piece.base.bounding_box;
    let depth = piece.base.gen_depth;
    let orientation = piece.base.orientation();
    let PieceKind::Mineshaft(mineshaft) = &piece.kind else {
        return;
    };

    match &mineshaft.kind {
        MineShaftPieceKind::Corridor { .. } => {
            corridor_add_children(
                start_bb,
                mineshaft_type,
                builder,
                random,
                box_,
                depth,
                orientation,
            );
        }
        MineShaftPieceKind::Crossing {
            direction,
            is_two_floored,
        } => {
            let (direction, is_two_floored) = (*direction, *is_two_floored);
            crossing_add_children(
                start_bb,
                mineshaft_type,
                builder,
                random,
                box_,
                depth,
                direction,
                is_two_floored,
            );
        }
        MineShaftPieceKind::Room { .. } => {
            room_add_children(
                index,
                start_bb,
                mineshaft_type,
                builder,
                random,
                box_,
                depth,
            );
        }
        MineShaftPieceKind::Stairs => {
            stairs_add_children(
                start_bb,
                mineshaft_type,
                builder,
                random,
                box_,
                depth,
                orientation,
            );
        }
    }
}

fn corridor_add_children(
    start_bb: BoundingBox,
    mineshaft_type: MineshaftType,
    builder: &mut StructurePiecesBuilder,
    random: &mut RandomSource,
    box_: BoundingBox,
    depth: i32,
    orientation: Option<Direction>,
) {
    let end_selection = random.next_i32_bounded(4);
    let Some(orientation) = orientation else {
        return;
    };

    let generate = |builder: &mut StructurePiecesBuilder,
                    random: &mut RandomSource,
                    x: i32,
                    y: i32,
                    z: i32,
                    direction: Direction,
                    depth: i32| {
        generate_and_add_piece(
            start_bb,
            mineshaft_type,
            builder,
            random,
            x,
            y,
            z,
            direction,
            depth,
        );
    };

    match orientation {
        Direction::South => {
            if end_selection <= 1 {
                let y = box_.min_y() - 1 + random.next_i32_bounded(3);
                generate(
                    builder,
                    random,
                    box_.min_x(),
                    y,
                    box_.max_z() + 1,
                    orientation,
                    depth,
                );
            } else if end_selection == 2 {
                let y = box_.min_y() - 1 + random.next_i32_bounded(3);
                generate(
                    builder,
                    random,
                    box_.min_x() - 1,
                    y,
                    box_.max_z() - 3,
                    Direction::West,
                    depth,
                );
            } else {
                let y = box_.min_y() - 1 + random.next_i32_bounded(3);
                generate(
                    builder,
                    random,
                    box_.max_x() + 1,
                    y,
                    box_.max_z() - 3,
                    Direction::East,
                    depth,
                );
            }
        }
        Direction::West => {
            if end_selection <= 1 {
                let y = box_.min_y() - 1 + random.next_i32_bounded(3);
                generate(
                    builder,
                    random,
                    box_.min_x() - 1,
                    y,
                    box_.min_z(),
                    orientation,
                    depth,
                );
            } else if end_selection == 2 {
                let y = box_.min_y() - 1 + random.next_i32_bounded(3);
                generate(
                    builder,
                    random,
                    box_.min_x(),
                    y,
                    box_.min_z() - 1,
                    Direction::North,
                    depth,
                );
            } else {
                let y = box_.min_y() - 1 + random.next_i32_bounded(3);
                generate(
                    builder,
                    random,
                    box_.min_x(),
                    y,
                    box_.max_z() + 1,
                    Direction::South,
                    depth,
                );
            }
        }
        Direction::East => {
            if end_selection <= 1 {
                let y = box_.min_y() - 1 + random.next_i32_bounded(3);
                generate(
                    builder,
                    random,
                    box_.max_x() + 1,
                    y,
                    box_.min_z(),
                    orientation,
                    depth,
                );
            } else if end_selection == 2 {
                let y = box_.min_y() - 1 + random.next_i32_bounded(3);
                generate(
                    builder,
                    random,
                    box_.max_x() - 3,
                    y,
                    box_.min_z() - 1,
                    Direction::North,
                    depth,
                );
            } else {
                let y = box_.min_y() - 1 + random.next_i32_bounded(3);
                generate(
                    builder,
                    random,
                    box_.max_x() - 3,
                    y,
                    box_.max_z() + 1,
                    Direction::South,
                    depth,
                );
            }
        }
        _ => {
            if end_selection <= 1 {
                let y = box_.min_y() - 1 + random.next_i32_bounded(3);
                generate(
                    builder,
                    random,
                    box_.min_x(),
                    y,
                    box_.min_z() - 1,
                    orientation,
                    depth,
                );
            } else if end_selection == 2 {
                let y = box_.min_y() - 1 + random.next_i32_bounded(3);
                generate(
                    builder,
                    random,
                    box_.min_x() - 1,
                    y,
                    box_.min_z(),
                    Direction::West,
                    depth,
                );
            } else {
                let y = box_.min_y() - 1 + random.next_i32_bounded(3);
                generate(
                    builder,
                    random,
                    box_.max_x() + 1,
                    y,
                    box_.min_z(),
                    Direction::East,
                    depth,
                );
            }
        }
    }

    if depth >= MAX_DEPTH {
        return;
    }

    if orientation != Direction::North && orientation != Direction::South {
        let mut x = box_.min_x() + 3;
        while x + 3 <= box_.max_x() {
            let selection = random.next_i32_bounded(5);
            if selection == 0 {
                generate(
                    builder,
                    random,
                    x,
                    box_.min_y(),
                    box_.min_z() - 1,
                    Direction::North,
                    depth + 1,
                );
            } else if selection == 1 {
                generate(
                    builder,
                    random,
                    x,
                    box_.min_y(),
                    box_.max_z() + 1,
                    Direction::South,
                    depth + 1,
                );
            }
            x += 5;
        }
    } else {
        let mut z = box_.min_z() + 3;
        while z + 3 <= box_.max_z() {
            let selection = random.next_i32_bounded(5);
            if selection == 0 {
                generate(
                    builder,
                    random,
                    box_.min_x() - 1,
                    box_.min_y(),
                    z,
                    Direction::West,
                    depth + 1,
                );
            } else if selection == 1 {
                generate(
                    builder,
                    random,
                    box_.max_x() + 1,
                    box_.min_y(),
                    z,
                    Direction::East,
                    depth + 1,
                );
            }
            z += 5;
        }
    }
}

#[expect(clippy::too_many_arguments, reason = "mirrors vanilla addChildren")]
fn crossing_add_children(
    start_bb: BoundingBox,
    mineshaft_type: MineshaftType,
    builder: &mut StructurePiecesBuilder,
    random: &mut RandomSource,
    box_: BoundingBox,
    depth: i32,
    direction: Direction,
    is_two_floored: bool,
) {
    let generate = |builder: &mut StructurePiecesBuilder,
                    random: &mut RandomSource,
                    x: i32,
                    y: i32,
                    z: i32,
                    dir: Direction| {
        generate_and_add_piece(
            start_bb,
            mineshaft_type,
            builder,
            random,
            x,
            y,
            z,
            dir,
            depth,
        );
    };

    match direction {
        Direction::South => {
            generate(
                builder,
                random,
                box_.min_x() + 1,
                box_.min_y(),
                box_.max_z() + 1,
                Direction::South,
            );
            generate(
                builder,
                random,
                box_.min_x() - 1,
                box_.min_y(),
                box_.min_z() + 1,
                Direction::West,
            );
            generate(
                builder,
                random,
                box_.max_x() + 1,
                box_.min_y(),
                box_.min_z() + 1,
                Direction::East,
            );
        }
        Direction::West => {
            generate(
                builder,
                random,
                box_.min_x() + 1,
                box_.min_y(),
                box_.min_z() - 1,
                Direction::North,
            );
            generate(
                builder,
                random,
                box_.min_x() + 1,
                box_.min_y(),
                box_.max_z() + 1,
                Direction::South,
            );
            generate(
                builder,
                random,
                box_.min_x() - 1,
                box_.min_y(),
                box_.min_z() + 1,
                Direction::West,
            );
        }
        Direction::East => {
            generate(
                builder,
                random,
                box_.min_x() + 1,
                box_.min_y(),
                box_.min_z() - 1,
                Direction::North,
            );
            generate(
                builder,
                random,
                box_.min_x() + 1,
                box_.min_y(),
                box_.max_z() + 1,
                Direction::South,
            );
            generate(
                builder,
                random,
                box_.max_x() + 1,
                box_.min_y(),
                box_.min_z() + 1,
                Direction::East,
            );
        }
        _ => {
            generate(
                builder,
                random,
                box_.min_x() + 1,
                box_.min_y(),
                box_.min_z() - 1,
                Direction::North,
            );
            generate(
                builder,
                random,
                box_.min_x() - 1,
                box_.min_y(),
                box_.min_z() + 1,
                Direction::West,
            );
            generate(
                builder,
                random,
                box_.max_x() + 1,
                box_.min_y(),
                box_.min_z() + 1,
                Direction::East,
            );
        }
    }

    if !is_two_floored {
        return;
    }

    if random.next_bool() {
        generate(
            builder,
            random,
            box_.min_x() + 1,
            box_.min_y() + 4,
            box_.min_z() - 1,
            Direction::North,
        );
    }
    if random.next_bool() {
        generate(
            builder,
            random,
            box_.min_x() - 1,
            box_.min_y() + 4,
            box_.min_z() + 1,
            Direction::West,
        );
    }
    if random.next_bool() {
        generate(
            builder,
            random,
            box_.max_x() + 1,
            box_.min_y() + 4,
            box_.min_z() + 1,
            Direction::East,
        );
    }
    if random.next_bool() {
        generate(
            builder,
            random,
            box_.min_x() + 1,
            box_.min_y() + 4,
            box_.max_z() + 1,
            Direction::South,
        );
    }
}

fn stairs_add_children(
    start_bb: BoundingBox,
    mineshaft_type: MineshaftType,
    builder: &mut StructurePiecesBuilder,
    random: &mut RandomSource,
    box_: BoundingBox,
    depth: i32,
    orientation: Option<Direction>,
) {
    let Some(orientation) = orientation else {
        return;
    };
    let (x, y, z, direction) = match orientation {
        Direction::South => (
            box_.min_x(),
            box_.min_y(),
            box_.max_z() + 1,
            Direction::South,
        ),
        Direction::West => (
            box_.min_x() - 1,
            box_.min_y(),
            box_.min_z(),
            Direction::West,
        ),
        Direction::East => (
            box_.max_x() + 1,
            box_.min_y(),
            box_.min_z(),
            Direction::East,
        ),
        _ => (
            box_.min_x(),
            box_.min_y(),
            box_.min_z() - 1,
            Direction::North,
        ),
    };
    generate_and_add_piece(
        start_bb,
        mineshaft_type,
        builder,
        random,
        x,
        y,
        z,
        direction,
        depth,
    );
}

fn room_add_children(
    index: usize,
    start_bb: BoundingBox,
    mineshaft_type: MineshaftType,
    builder: &mut StructurePiecesBuilder,
    random: &mut RandomSource,
    box_: BoundingBox,
    depth: i32,
) {
    let mut height_space = box_.y_span() - 3 - 1;
    if height_space <= 0 {
        height_space = 1;
    }

    let push_entrance = |builder: &mut StructurePiecesBuilder, entrance: BoundingBox| {
        if let PieceKind::Mineshaft(MineShaftPiece {
            kind: MineShaftPieceKind::Room {
                child_entrance_boxes,
            },
            ..
        }) = &mut builder.piece_at_mut(index).kind
        {
            child_entrance_boxes.push(entrance);
        }
    };

    let mut pos = 0;
    while pos < box_.x_span() {
        pos += random.next_i32_bounded(box_.x_span());
        if pos + 3 > box_.x_span() {
            break;
        }
        let y = box_.min_y() + random.next_i32_bounded(height_space) + 1;
        let child = generate_and_add_piece(
            start_bb,
            mineshaft_type,
            builder,
            random,
            box_.min_x() + pos,
            y,
            box_.min_z() - 1,
            Direction::North,
            depth,
        );
        if let Some(child_box) = child {
            push_entrance(
                builder,
                BoundingBox::new(
                    child_box.min_x(),
                    child_box.min_y(),
                    box_.min_z(),
                    child_box.max_x(),
                    child_box.max_y(),
                    box_.min_z() + 1,
                ),
            );
        }
        pos += 4;
    }

    let mut pos = 0;
    while pos < box_.x_span() {
        pos += random.next_i32_bounded(box_.x_span());
        if pos + 3 > box_.x_span() {
            break;
        }
        let y = box_.min_y() + random.next_i32_bounded(height_space) + 1;
        let child = generate_and_add_piece(
            start_bb,
            mineshaft_type,
            builder,
            random,
            box_.min_x() + pos,
            y,
            box_.max_z() + 1,
            Direction::South,
            depth,
        );
        if let Some(child_box) = child {
            push_entrance(
                builder,
                BoundingBox::new(
                    child_box.min_x(),
                    child_box.min_y(),
                    box_.max_z() - 1,
                    child_box.max_x(),
                    child_box.max_y(),
                    box_.max_z(),
                ),
            );
        }
        pos += 4;
    }

    let mut pos = 0;
    while pos < box_.z_span() {
        pos += random.next_i32_bounded(box_.z_span());
        if pos + 3 > box_.z_span() {
            break;
        }
        let y = box_.min_y() + random.next_i32_bounded(height_space) + 1;
        let child = generate_and_add_piece(
            start_bb,
            mineshaft_type,
            builder,
            random,
            box_.min_x() - 1,
            y,
            box_.min_z() + pos,
            Direction::West,
            depth,
        );
        if let Some(child_box) = child {
            push_entrance(
                builder,
                BoundingBox::new(
                    box_.min_x(),
                    child_box.min_y(),
                    child_box.min_z(),
                    box_.min_x() + 1,
                    child_box.max_y(),
                    child_box.max_z(),
                ),
            );
        }
        pos += 4;
    }

    let mut pos = 0;
    while pos < box_.z_span() {
        pos += random.next_i32_bounded(box_.z_span());
        if pos + 3 > box_.z_span() {
            break;
        }
        let y = box_.min_y() + random.next_i32_bounded(height_space) + 1;
        let child = generate_and_add_piece(
            start_bb,
            mineshaft_type,
            builder,
            random,
            box_.max_x() + 1,
            y,
            box_.min_z() + pos,
            Direction::East,
            depth,
        );
        if let Some(child_box) = child {
            push_entrance(
                builder,
                BoundingBox::new(
                    box_.max_x() - 1,
                    child_box.min_y(),
                    child_box.min_z(),
                    box_.max_x(),
                    child_box.max_y(),
                    child_box.max_z(),
                ),
            );
        }
        pos += 4;
    }
}

impl MineShaftPiece {
    pub fn move_by(&mut self, dx: i32, dy: i32, dz: i32) {
        if let MineShaftPieceKind::Room {
            child_entrance_boxes,
        } = &mut self.kind
        {
            for box_ in child_entrance_boxes.iter_mut() {
                *box_ = box_.moved(dx, dy, dz);
            }
        }
    }

    pub fn post_process(
        &mut self,
        base: &mut PieceBase,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        chunk_bb: BoundingBox,
    ) {
        if is_in_invalid_location(base, region, chunk_bb) {
            return;
        }
        let mineshaft_type = self.mineshaft_type;
        match &mut self.kind {
            MineShaftPieceKind::Corridor {
                has_rails,
                spider_corridor,
                has_placed_spider,
                num_sections,
            } => corridor_post_process(
                base,
                region,
                random,
                chunk_bb,
                mineshaft_type,
                *has_rails,
                *spider_corridor,
                has_placed_spider,
                *num_sections,
            ),
            MineShaftPieceKind::Crossing { is_two_floored, .. } => {
                crossing_post_process(base, region, chunk_bb, mineshaft_type, *is_two_floored)
            }
            MineShaftPieceKind::Room {
                child_entrance_boxes,
            } => room_post_process(base, region, chunk_bb, child_entrance_boxes),
            MineShaftPieceKind::Stairs => stairs_post_process(base, region, chunk_bb),
        }
    }
}

fn is_in_invalid_location(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    chunk_bb: BoundingBox,
) -> bool {
    let box_ = base.bounding_box;
    let x0 = (box_.min_x() - 1).max(chunk_bb.min_x());
    let y0 = (box_.min_y() - 1).max(chunk_bb.min_y());
    let z0 = (box_.min_z() - 1).max(chunk_bb.min_z());
    let x1 = (box_.max_x() + 1).min(chunk_bb.max_x());
    let y1 = (box_.max_y() + 1).min(chunk_bb.max_y());
    let z1 = (box_.max_z() + 1).min(chunk_bb.max_z());

    let center = BlockPosition::new((x0 + x1) / 2, (y0 + y1) / 2, (z0 + z1) / 2);
    let biome_id = region.get_biome(center);
    if let Some(biome) = registries::BIOME.by_id(biome_id)
        && registries::BIOME.has_tag(biome.key(), &MINESHAFT_BLOCKING)
    {
        return true;
    }

    let liquid = |x: i32, y: i32, z: i32| region.get_block(BlockPosition::new(x, y, z)).liquid();

    for x in x0..=x1 {
        for z in z0..=z1 {
            if liquid(x, y0, z) || liquid(x, y1, z) {
                return true;
            }
        }
    }
    for x in x0..=x1 {
        for y in y0..=y1 {
            if liquid(x, y, z0) || liquid(x, y, z1) {
                return true;
            }
        }
    }
    for z in z0..=z1 {
        for y in y0..=y1 {
            if liquid(x0, y, z) || liquid(x1, y, z) {
                return true;
            }
        }
    }
    false
}

fn set_planks_block(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    chunk_bb: BoundingBox,
    planks: BlockState,
    x: i32,
    y: i32,
    z: i32,
) {
    if !base.is_interior(region, x, y, z, chunk_bb) {
        return;
    }
    let pos = base.world_pos(x, y, z);
    let existing = region.get_block(pos);
    if !existing.is_side_solid_at(pos, Direction::Up) {
        region.set_block(pos, planks);
    }
}

fn is_supporting_box(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    chunk_bb: BoundingBox,
    x0: i32,
    x1: i32,
    y1: i32,
    z0: i32,
) -> bool {
    for x in x0..=x1 {
        if base.get_block(region, x, y1 + 1, z0, chunk_bb).is_air() {
            return false;
        }
    }
    true
}

fn can_place_column_on_top_of(pos: BlockPosition, state: BlockState) -> bool {
    state.is_side_solid_at(pos, Direction::Up)
}

fn can_hang_chain_below(pos: BlockPosition, state: BlockState) -> bool {
    !state.as_block().has_tag(&UNSTABLE_BOTTOM_CENTER)
        && state.is_side_solid_for_at(pos, Direction::Down, SupportType::Center)
        && !is_falling_block(state.as_block())
}

fn is_falling_block(block: Block) -> bool {
    matches!(
        block,
        Block::SAND
            | Block::RED_SAND
            | Block::GRAVEL
            | Block::DRAGON_EGG
            | Block::ANVIL
            | Block::CHIPPED_ANVIL
            | Block::DAMAGED_ANVIL
    ) || block.key().path().ends_with("_concrete_powder")
}

fn fill_column_between(
    region: &WorldGenRegion<'_>,
    state: BlockState,
    x: i32,
    z: i32,
    bottom_inclusive: i32,
    top_exclusive: i32,
) {
    for y in bottom_inclusive..top_exclusive {
        region.set_block(BlockPosition::new(x, y, z), state);
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "mirrors vanilla fillPillarDownOrChainUp"
)]
fn fill_pillar_down_or_chain_up(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    mineshaft_type: MineshaftType,
    pillar_state: BlockState,
    x: i32,
    y: i32,
    z: i32,
    chunk_bb: BoundingBox,
) {
    let origin = base.world_pos(x, y, z);
    if !chunk_bb.is_inside(origin) {
        return;
    }
    let world_y = origin.y();
    let (world_x, world_z) = (origin.x(), origin.z());
    let mut distance = 1;
    let mut check_below = true;
    let mut check_above = true;

    while check_below || check_above {
        if check_below {
            let pos = BlockPosition::new(world_x, world_y - distance, world_z);
            let below_state = region.get_block(pos);
            let empty_below =
                is_replaceable_by_structures(below_state) && !below_state.is(Block::LAVA);
            if !empty_below && can_place_column_on_top_of(pos, below_state) {
                fill_column_between(
                    region,
                    pillar_state,
                    world_x,
                    world_z,
                    world_y - distance + 1,
                    world_y,
                );
                return;
            }
            check_below =
                distance <= MAX_PILLAR_HEIGHT && empty_below && pos.y() > region.min_y() + 1;
        }

        if check_above {
            let pos = BlockPosition::new(world_x, world_y + distance, world_z);
            let above_state = region.get_block(pos);
            let empty_above = is_replaceable_by_structures(above_state);
            if !empty_above && can_hang_chain_below(pos, above_state) {
                region.set_block(
                    BlockPosition::new(world_x, world_y + 1, world_z),
                    mineshaft_type.fence_state(),
                );
                fill_column_between(
                    region,
                    Block::IRON_CHAIN.default_state(),
                    world_x,
                    world_z,
                    world_y + 2,
                    world_y + distance,
                );
                return;
            }
            check_above =
                distance <= MAX_CHAIN_HEIGHT && empty_above && pos.y() < region.max_y() - 1;
        }

        distance += 1;
    }
}

fn has_sturdy_neighbours(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    chunk_bb: BoundingBox,
    x: i32,
    y: i32,
    z: i32,
    count: i32,
) -> bool {
    let origin = base.world_pos(x, y, z);
    let mut sturdy = 0;
    for direction in Direction::ALL {
        let pos = origin.relative(direction);
        if chunk_bb.is_inside(pos)
            && region
                .get_block(pos)
                .is_side_solid_at(pos, direction.opposite())
        {
            sturdy += 1;
            if sturdy >= count {
                return true;
            }
        }
    }
    false
}

#[expect(
    clippy::too_many_arguments,
    reason = "mirrors vanilla maybePlaceCobWeb"
)]
fn maybe_place_cob_web(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    chunk_bb: BoundingBox,
    random: &mut RandomSource,
    probability: f32,
    x: i32,
    y: i32,
    z: i32,
) {
    if base.is_interior(region, x, y, z, chunk_bb)
        && random.next_f32() < probability
        && has_sturdy_neighbours(base, region, chunk_bb, x, y, z, 2)
    {
        base.place_block(region, Block::COBWEB.default_state(), x, y, z, chunk_bb);
    }
}

#[expect(clippy::too_many_arguments, reason = "mirrors vanilla placeSupport")]
fn place_support(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    chunk_bb: BoundingBox,
    mineshaft_type: MineshaftType,
    x0: i32,
    y0: i32,
    z: i32,
    y1: i32,
    x1: i32,
    random: &mut RandomSource,
) {
    if !is_supporting_box(base, region, chunk_bb, x0, x1, y1, z) {
        return;
    }
    let planks = mineshaft_type.planks_state();
    let fence = mineshaft_type.fence_state();
    let cave_air = cave_air();
    base.generate_box(
        region,
        chunk_bb,
        x0,
        y0,
        z,
        x0,
        y1 - 1,
        z,
        fence.with(Properties::WEST, true),
        cave_air,
        false,
    );
    base.generate_box(
        region,
        chunk_bb,
        x1,
        y0,
        z,
        x1,
        y1 - 1,
        z,
        fence.with(Properties::EAST, true),
        cave_air,
        false,
    );
    if random.next_i32_bounded(4) == 0 {
        base.generate_box(
            region, chunk_bb, x0, y1, z, x0, y1, z, planks, cave_air, false,
        );
        base.generate_box(
            region, chunk_bb, x1, y1, z, x1, y1, z, planks, cave_air, false,
        );
    } else {
        base.generate_box(
            region, chunk_bb, x0, y1, z, x1, y1, z, planks, cave_air, false,
        );
        base.maybe_generate_block(
            region,
            chunk_bb,
            random,
            0.05,
            x0 + 1,
            y1,
            z - 1,
            Block::WALL_TORCH
                .default_state()
                .with(Properties::HORIZONTAL_FACING, Direction::South),
        );
        base.maybe_generate_block(
            region,
            chunk_bb,
            random,
            0.05,
            x0 + 1,
            y1,
            z + 1,
            Block::WALL_TORCH
                .default_state()
                .with(Properties::HORIZONTAL_FACING, Direction::North),
        );
    }
}

fn corridor_create_chest(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    chunk_bb: BoundingBox,
    random: &mut RandomSource,
    x: i32,
    y: i32,
    z: i32,
) -> bool {
    let pos = base.world_pos(x, y, z);
    if !chunk_bb.is_inside(pos)
        || !region.get_block(pos).is_air()
        || region.get_block(pos.below()).is_air()
    {
        return false;
    }
    let shape = if random.next_bool() {
        RailShape::NorthSouth
    } else {
        RailShape::EastWest
    };
    let state = Block::RAIL
        .default_state()
        .with(Properties::RAIL_SHAPE, shape);
    base.place_block(region, state, x, y, z, chunk_bb);
    random.next_i64();
    true
}

#[expect(
    clippy::too_many_arguments,
    reason = "mirrors vanilla MineShaftCorridor::postProcess"
)]
fn corridor_post_process(
    base: &mut PieceBase,
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    chunk_bb: BoundingBox,
    mineshaft_type: MineshaftType,
    has_rails: bool,
    spider_corridor: bool,
    has_placed_spider: &mut bool,
    num_sections: i32,
) {
    let piece = &*base;
    let cave_air = cave_air();
    let length = num_sections * 5 - 1;
    let planks = mineshaft_type.planks_state();

    piece.generate_box(
        region, chunk_bb, 0, 0, 0, 2, 1, length, cave_air, cave_air, false,
    );
    piece.generate_maybe_box(
        region, chunk_bb, random, 0.8, 0, 2, 0, 2, 2, length, cave_air, cave_air, false, false,
    );
    if spider_corridor {
        piece.generate_maybe_box(
            region,
            chunk_bb,
            random,
            0.6,
            0,
            0,
            0,
            2,
            1,
            length,
            Block::COBWEB.default_state(),
            cave_air,
            false,
            true,
        );
    }

    for section in 0..num_sections {
        let z = 2 + section * 5;
        place_support(
            piece,
            region,
            chunk_bb,
            mineshaft_type,
            0,
            0,
            z,
            2,
            2,
            random,
        );
        maybe_place_cob_web(piece, region, chunk_bb, random, 0.1, 0, 2, z - 1);
        maybe_place_cob_web(piece, region, chunk_bb, random, 0.1, 2, 2, z - 1);
        maybe_place_cob_web(piece, region, chunk_bb, random, 0.1, 0, 2, z + 1);
        maybe_place_cob_web(piece, region, chunk_bb, random, 0.1, 2, 2, z + 1);
        maybe_place_cob_web(piece, region, chunk_bb, random, 0.05, 0, 2, z - 2);
        maybe_place_cob_web(piece, region, chunk_bb, random, 0.05, 2, 2, z - 2);
        maybe_place_cob_web(piece, region, chunk_bb, random, 0.05, 0, 2, z + 2);
        maybe_place_cob_web(piece, region, chunk_bb, random, 0.05, 2, 2, z + 2);
        if random.next_i32_bounded(100) == 0 {
            corridor_create_chest(piece, region, chunk_bb, random, 2, 0, z - 1);
        }
        if random.next_i32_bounded(100) == 0 {
            corridor_create_chest(piece, region, chunk_bb, random, 0, 0, z + 1);
        }
        if spider_corridor && !*has_placed_spider {
            let new_z = z - 1 + random.next_i32_bounded(3);
            let pos = piece.world_pos(1, 0, new_z);
            if chunk_bb.is_inside(pos) && piece.is_interior(region, 1, 0, new_z, chunk_bb) {
                *has_placed_spider = true;
                region.set_block(pos, Block::SPAWNER.default_state());
            }
        }
    }

    for x in 0..=2 {
        for z in 0..=length {
            set_planks_block(piece, region, chunk_bb, planks, x, -1, z);
        }
    }

    place_double_lower_or_upper_support(piece, region, chunk_bb, mineshaft_type, 0, -1, 2);
    if num_sections > 1 {
        let last_support_pillar = length - 2;
        place_double_lower_or_upper_support(
            piece,
            region,
            chunk_bb,
            mineshaft_type,
            0,
            -1,
            last_support_pillar,
        );
    }

    if has_rails {
        let state = Block::RAIL
            .default_state()
            .with(Properties::RAIL_SHAPE, RailShape::NorthSouth);
        for z in 0..=length {
            let floor = piece.get_block(region, 1, -1, z, chunk_bb);
            if !floor.is_air() && floor.is_solid_render() {
                let probability = if piece.is_interior(region, 1, 0, z, chunk_bb) {
                    0.7
                } else {
                    0.9
                };
                piece.maybe_generate_block(region, chunk_bb, random, probability, 1, 0, z, state);
            }
        }
    }
}

fn place_double_lower_or_upper_support(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    chunk_bb: BoundingBox,
    mineshaft_type: MineshaftType,
    x: i32,
    y: i32,
    z: i32,
) {
    let wood = mineshaft_type.wood_state();
    let planks_block = mineshaft_type.planks_state().as_block();
    if base.get_block(region, x, y, z, chunk_bb).as_block() == planks_block {
        fill_pillar_down_or_chain_up(base, region, mineshaft_type, wood, x, y, z, chunk_bb);
    }
    if base.get_block(region, x + 2, y, z, chunk_bb).as_block() == planks_block {
        fill_pillar_down_or_chain_up(base, region, mineshaft_type, wood, x + 2, y, z, chunk_bb);
    }
}

fn crossing_post_process(
    base: &mut PieceBase,
    region: &WorldGenRegion<'_>,
    chunk_bb: BoundingBox,
    mineshaft_type: MineshaftType,
    is_two_floored: bool,
) {
    let piece = &*base;
    let box_ = piece.bounding_box;
    let cave_air = cave_air();
    let planks = mineshaft_type.planks_state();

    let fill = |x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32| {
        piece.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, cave_air, cave_air, false,
        );
    };

    if is_two_floored {
        fill(
            box_.min_x() + 1,
            box_.min_y(),
            box_.min_z(),
            box_.max_x() - 1,
            box_.min_y() + 2,
            box_.max_z(),
        );
        fill(
            box_.min_x(),
            box_.min_y(),
            box_.min_z() + 1,
            box_.max_x(),
            box_.min_y() + 2,
            box_.max_z() - 1,
        );
        fill(
            box_.min_x() + 1,
            box_.max_y() - 2,
            box_.min_z(),
            box_.max_x() - 1,
            box_.max_y(),
            box_.max_z(),
        );
        fill(
            box_.min_x(),
            box_.max_y() - 2,
            box_.min_z() + 1,
            box_.max_x(),
            box_.max_y(),
            box_.max_z() - 1,
        );
        fill(
            box_.min_x() + 1,
            box_.min_y() + 3,
            box_.min_z() + 1,
            box_.max_x() - 1,
            box_.min_y() + 3,
            box_.max_z() - 1,
        );
    } else {
        fill(
            box_.min_x() + 1,
            box_.min_y(),
            box_.min_z(),
            box_.max_x() - 1,
            box_.max_y(),
            box_.max_z(),
        );
        fill(
            box_.min_x(),
            box_.min_y(),
            box_.min_z() + 1,
            box_.max_x(),
            box_.max_y(),
            box_.max_z() - 1,
        );
    }

    let place_support_pillar = |x: i32, y0: i32, z: i32, y1: i32| {
        if !piece.get_block(region, x, y1 + 1, z, chunk_bb).is_air() {
            piece.generate_box(
                region, chunk_bb, x, y0, z, x, y1, z, planks, cave_air, false,
            );
        }
    };
    place_support_pillar(
        box_.min_x() + 1,
        box_.min_y(),
        box_.min_z() + 1,
        box_.max_y(),
    );
    place_support_pillar(
        box_.min_x() + 1,
        box_.min_y(),
        box_.max_z() - 1,
        box_.max_y(),
    );
    place_support_pillar(
        box_.max_x() - 1,
        box_.min_y(),
        box_.min_z() + 1,
        box_.max_y(),
    );
    place_support_pillar(
        box_.max_x() - 1,
        box_.min_y(),
        box_.max_z() - 1,
        box_.max_y(),
    );

    let y = box_.min_y() - 1;
    for x in box_.min_x()..=box_.max_x() {
        for z in box_.min_z()..=box_.max_z() {
            set_planks_block(piece, region, chunk_bb, planks, x, y, z);
        }
    }
}

fn room_post_process(
    base: &mut PieceBase,
    region: &WorldGenRegion<'_>,
    chunk_bb: BoundingBox,
    child_entrance_boxes: &[BoundingBox],
) {
    let piece = &*base;
    let box_ = piece.bounding_box;
    let cave_air = cave_air();

    piece.generate_box(
        region,
        chunk_bb,
        box_.min_x(),
        box_.min_y() + 1,
        box_.min_z(),
        box_.max_x(),
        (box_.min_y() + 3).min(box_.max_y()),
        box_.max_z(),
        cave_air,
        cave_air,
        false,
    );

    for entrance in child_entrance_boxes {
        piece.generate_box(
            region,
            chunk_bb,
            entrance.min_x(),
            entrance.max_y() - 2,
            entrance.min_z(),
            entrance.max_x(),
            entrance.max_y(),
            entrance.max_z(),
            cave_air,
            cave_air,
            false,
        );
    }

    piece.generate_upper_half_sphere(
        region,
        chunk_bb,
        box_.min_x(),
        box_.min_y() + 4,
        box_.min_z(),
        box_.max_x(),
        box_.max_y(),
        box_.max_z(),
        cave_air,
        false,
    );
}

fn stairs_post_process(base: &mut PieceBase, region: &WorldGenRegion<'_>, chunk_bb: BoundingBox) {
    let piece = &*base;
    let cave_air = cave_air();
    piece.generate_box(
        region, chunk_bb, 0, 5, 0, 2, 7, 1, cave_air, cave_air, false,
    );
    piece.generate_box(
        region, chunk_bb, 0, 0, 7, 2, 2, 8, cave_air, cave_air, false,
    );
    for i in 0..5 {
        piece.generate_box(
            region,
            chunk_bb,
            0,
            5 - i - i32::from(i < 4),
            2 + i,
            2,
            7 - i,
            2 + i,
            cave_air,
            cave_air,
            false,
        );
    }
}
