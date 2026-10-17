use crate::util::rng::RandomSource;
use crate::world::worldgen::region::WorldGenRegion;
use crate::world::worldgen::structure::pieces::structure_pieces_builder::StructurePiecesBuilder;
use crate::world::worldgen::structure::structure_piece::{
    PieceBase, PieceKind, StructurePiece, cave_air,
};
use shulkr::util::BoundingBox;
use shulkr::util::Direction;
use shulkr::world::block::Block;
use shulkr::world::block::BlockState;
use shulkr::world::block::{DoubleBlockHalf, Properties, SlabType};

const MAX_DEPTH: i32 = 50;
const LOWEST_Y_POSITION: i32 = 10;
pub const MAGIC_START_Y: i32 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmallDoorType {
    Opening,
    WoodDoor,
    Grates,
    IronDoor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StrongholdPieceType {
    Straight,
    PrisonHall,
    LeftTurn,
    RightTurn,
    RoomCrossing,
    StraightStairsDown,
    StairsDown,
    FiveCrossing,
    ChestCorridor,
    Library,
    PortalRoom,
}

#[derive(Debug, Clone, Copy)]
struct PieceWeight {
    piece_type: StrongholdPieceType,
    weight: i32,
    place_count: i32,
    max_place_count: i32,
}

impl PieceWeight {
    const fn is_valid(self) -> bool {
        self.max_place_count == 0 || self.place_count < self.max_place_count
    }

    const fn do_place(self, depth: i32) -> bool {
        match self.piece_type {
            StrongholdPieceType::Library => self.is_valid() && depth > 4,
            StrongholdPieceType::PortalRoom => self.is_valid() && depth > 5,
            _ => self.is_valid(),
        }
    }
}

fn stronghold_piece_weights() -> Vec<PieceWeight> {
    let entry = |piece_type, weight, max_place_count| PieceWeight {
        piece_type,
        weight,
        place_count: 0,
        max_place_count,
    };
    vec![
        entry(StrongholdPieceType::Straight, 40, 0),
        entry(StrongholdPieceType::PrisonHall, 5, 5),
        entry(StrongholdPieceType::LeftTurn, 20, 0),
        entry(StrongholdPieceType::RightTurn, 20, 0),
        entry(StrongholdPieceType::RoomCrossing, 10, 6),
        entry(StrongholdPieceType::StraightStairsDown, 5, 5),
        entry(StrongholdPieceType::StairsDown, 5, 5),
        entry(StrongholdPieceType::FiveCrossing, 5, 4),
        entry(StrongholdPieceType::ChestCorridor, 5, 4),
        entry(StrongholdPieceType::Library, 10, 2),
        entry(StrongholdPieceType::PortalRoom, 20, 1),
    ]
}

#[derive(Debug)]
pub enum StrongholdKind {
    ChestCorridor {
        has_placed_chest: bool,
    },
    FillerCorridor {
        steps: i32,
    },
    FiveCrossing {
        left_low: bool,
        left_high: bool,
        right_low: bool,
        right_high: bool,
    },
    LeftTurn,
    RightTurn,
    Library {
        is_tall: bool,
    },
    PortalRoom {
        has_placed_spawner: bool,
    },
    PrisonHall,
    RoomCrossing {
        room_type: i32,
    },
    StairsDown {
        is_source: bool,
    },
    Straight {
        left_child: bool,
        right_child: bool,
    },
    StraightStairsDown,
}

#[derive(Debug)]
pub struct StrongholdPiece {
    pub kind: StrongholdKind,
    pub entry_door: SmallDoorType,
}

struct StrongholdGen {
    start_bb: BoundingBox,
    current_pieces: Vec<PieceWeight>,
    imposed_piece: Option<StrongholdPieceType>,
    previous_piece: Option<StrongholdPieceType>,
    pending_children: Vec<usize>,
    portal_room_placed: bool,
}

const fn make_piece(
    kind: StrongholdKind,
    entry_door: SmallDoorType,
    gen_depth: i32,
    bounding_box: BoundingBox,
    direction: Direction,
) -> StructurePiece {
    let mut base = PieceBase::new(gen_depth, bounding_box);
    base.set_orientation(Some(direction));
    StructurePiece::new(
        base,
        PieceKind::Stronghold(StrongholdPiece { kind, entry_door }),
    )
}

const fn is_ok_box(box_: BoundingBox) -> bool {
    box_.min_y() > LOWEST_Y_POSITION
}

fn try_box(builder: &StructurePiecesBuilder, box_: BoundingBox) -> Option<BoundingBox> {
    (is_ok_box(box_) && builder.find_collision_piece(box_).is_none()).then_some(box_)
}

fn random_small_door(random: &mut RandomSource) -> SmallDoorType {
    match random.next_i32_bounded(5) {
        2 => SmallDoorType::WoodDoor,
        3 => SmallDoorType::Grates,
        4 => SmallDoorType::IronDoor,
        _ => SmallDoorType::Opening,
    }
}

pub fn new_start_piece(random: &mut RandomSource, west: i32, north: i32) -> StructurePiece {
    let direction = random.next_horizontal_direction();
    let box_ = PieceBase::make_bounding_box(west, MAGIC_START_Y, north, direction, 5, 11, 5);
    make_piece(
        StrongholdKind::StairsDown { is_source: true },
        SmallDoorType::Opening,
        0,
        box_,
        direction,
    )
}

pub fn generate_pieces(
    builder: &mut StructurePiecesBuilder,
    random: &mut RandomSource,
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
    sea_level: i32,
    min_y: i32,
) {
    let mut tries = 0;
    loop {
        builder.clear();
        random.set_large_feature_seed(seed + tries, chunk_x, chunk_z);
        tries += 1;

        let start = new_start_piece(random, chunk_x * 16 + 2, chunk_z * 16 + 2);
        let start_bb = start.bounding_box();
        let start_index = builder.add_piece_indexed(start);
        let mut state = StrongholdGen {
            start_bb,
            current_pieces: stronghold_piece_weights(),
            imposed_piece: None,
            previous_piece: None,
            pending_children: Vec::new(),
            portal_room_placed: false,
        };

        add_children(start_index, &mut state, builder, random);

        while !state.pending_children.is_empty() {
            let index = random.next_i32_bounded(state.pending_children.len() as i32) as usize;
            let piece_index = state.pending_children.remove(index);
            add_children(piece_index, &mut state, builder, random);
        }

        builder.move_below_sea_level(sea_level, min_y, random, 10);

        if !builder.is_empty() && state.portal_room_placed {
            return;
        }
    }
}

#[expect(clippy::too_many_arguments, reason = "mirrors vanilla piece factories")]
fn create_stronghold_piece(
    piece_type: StrongholdPieceType,
    builder: &StructurePiecesBuilder,
    random: &mut RandomSource,
    foot_x: i32,
    foot_y: i32,
    foot_z: i32,
    direction: Direction,
    gen_depth: i32,
) -> Option<StructurePiece> {
    let orient = |off_x, off_y, off_z, width, height, depth| {
        BoundingBox::orient_box(
            foot_x, foot_y, foot_z, off_x, off_y, off_z, width, height, depth, direction,
        )
    };

    match piece_type {
        StrongholdPieceType::Straight => {
            let box_ = try_box(builder, orient(-1, -1, 0, 5, 5, 7))?;
            let entry_door = random_small_door(random);
            let left_child = random.next_i32_bounded(2) == 0;
            let right_child = random.next_i32_bounded(2) == 0;
            Some(make_piece(
                StrongholdKind::Straight {
                    left_child,
                    right_child,
                },
                entry_door,
                gen_depth,
                box_,
                direction,
            ))
        }
        StrongholdPieceType::PrisonHall => {
            let box_ = try_box(builder, orient(-1, -1, 0, 9, 5, 11))?;
            let entry_door = random_small_door(random);
            Some(make_piece(
                StrongholdKind::PrisonHall,
                entry_door,
                gen_depth,
                box_,
                direction,
            ))
        }
        StrongholdPieceType::LeftTurn => {
            let box_ = try_box(builder, orient(-1, -1, 0, 5, 5, 5))?;
            let entry_door = random_small_door(random);
            Some(make_piece(
                StrongholdKind::LeftTurn,
                entry_door,
                gen_depth,
                box_,
                direction,
            ))
        }
        StrongholdPieceType::RightTurn => {
            let box_ = try_box(builder, orient(-1, -1, 0, 5, 5, 5))?;
            let entry_door = random_small_door(random);
            Some(make_piece(
                StrongholdKind::RightTurn,
                entry_door,
                gen_depth,
                box_,
                direction,
            ))
        }
        StrongholdPieceType::RoomCrossing => {
            let box_ = try_box(builder, orient(-4, -1, 0, 11, 7, 11))?;
            let entry_door = random_small_door(random);
            let room_type = random.next_i32_bounded(5);
            Some(make_piece(
                StrongholdKind::RoomCrossing { room_type },
                entry_door,
                gen_depth,
                box_,
                direction,
            ))
        }
        StrongholdPieceType::StraightStairsDown => {
            let box_ = try_box(builder, orient(-1, -7, 0, 5, 11, 8))?;
            let entry_door = random_small_door(random);
            Some(make_piece(
                StrongholdKind::StraightStairsDown,
                entry_door,
                gen_depth,
                box_,
                direction,
            ))
        }
        StrongholdPieceType::StairsDown => {
            let box_ = try_box(builder, orient(-1, -7, 0, 5, 11, 5))?;
            let entry_door = random_small_door(random);
            Some(make_piece(
                StrongholdKind::StairsDown { is_source: false },
                entry_door,
                gen_depth,
                box_,
                direction,
            ))
        }
        StrongholdPieceType::FiveCrossing => {
            let box_ = try_box(builder, orient(-4, -3, 0, 10, 9, 11))?;
            let entry_door = random_small_door(random);
            let left_low = random.next_bool();
            let left_high = random.next_bool();
            let right_low = random.next_bool();
            let right_high = random.next_i32_bounded(3) > 0;
            Some(make_piece(
                StrongholdKind::FiveCrossing {
                    left_low,
                    left_high,
                    right_low,
                    right_high,
                },
                entry_door,
                gen_depth,
                box_,
                direction,
            ))
        }
        StrongholdPieceType::ChestCorridor => {
            let box_ = try_box(builder, orient(-1, -1, 0, 5, 5, 7))?;
            let entry_door = random_small_door(random);
            Some(make_piece(
                StrongholdKind::ChestCorridor {
                    has_placed_chest: false,
                },
                entry_door,
                gen_depth,
                box_,
                direction,
            ))
        }
        StrongholdPieceType::Library => {
            let tall = orient(-4, -1, 0, 14, 11, 15);
            let box_ = if is_ok_box(tall) && builder.find_collision_piece(tall).is_none() {
                tall
            } else {
                let short = orient(-4, -1, 0, 14, 6, 15);
                if !is_ok_box(short) || builder.find_collision_piece(short).is_some() {
                    return None;
                }
                short
            };
            let entry_door = random_small_door(random);
            let is_tall = box_.y_span() > 6;
            Some(make_piece(
                StrongholdKind::Library { is_tall },
                entry_door,
                gen_depth,
                box_,
                direction,
            ))
        }
        StrongholdPieceType::PortalRoom => {
            let box_ = try_box(builder, orient(-4, -1, 0, 11, 8, 16))?;
            Some(make_piece(
                StrongholdKind::PortalRoom {
                    has_placed_spawner: false,
                },
                SmallDoorType::Opening,
                gen_depth,
                box_,
                direction,
            ))
        }
    }
}

fn find_filler_corridor_box(
    builder: &StructurePiecesBuilder,
    foot_x: i32,
    foot_y: i32,
    foot_z: i32,
    direction: Direction,
) -> Option<BoundingBox> {
    let orient =
        |depth| BoundingBox::orient_box(foot_x, foot_y, foot_z, -1, -1, 0, 5, 5, depth, direction);
    let box_ = orient(4);
    let collision = builder.find_collision_piece(box_)?;
    let collision_box = collision.bounding_box();
    if collision_box.min_y() == box_.min_y() {
        for depth in (1..=2).rev() {
            let candidate = orient(depth);
            if !collision_box.intersects(candidate) {
                return Some(orient(depth + 1));
            }
        }
    }
    None
}

fn update_piece_weight(pieces: &[PieceWeight]) -> (bool, i32) {
    let mut has_any_pieces = false;
    let mut total_weight = 0;
    for piece in pieces {
        if piece.max_place_count > 0 && piece.place_count < piece.max_place_count {
            has_any_pieces = true;
        }
        total_weight += piece.weight;
    }
    (has_any_pieces, total_weight)
}

#[expect(
    clippy::too_many_arguments,
    reason = "mirrors vanilla generatePieceFromSmallDoor"
)]
fn generate_piece_from_small_door(
    state: &mut StrongholdGen,
    builder: &StructurePiecesBuilder,
    random: &mut RandomSource,
    foot_x: i32,
    foot_y: i32,
    foot_z: i32,
    direction: Direction,
    depth: i32,
) -> Option<StructurePiece> {
    let (has_any_pieces, total_weight) = update_piece_weight(&state.current_pieces);
    if !has_any_pieces {
        return None;
    }

    if let Some(imposed) = state.imposed_piece.take() {
        let created = create_stronghold_piece(
            imposed, builder, random, foot_x, foot_y, foot_z, direction, depth,
        );
        if created.is_some() {
            return created;
        }
    }

    let mut num_attempts = 0;
    while num_attempts < 5 {
        num_attempts += 1;
        let mut weight_selection = random.next_i32_bounded(total_weight);

        for index in 0..state.current_pieces.len() {
            let piece = state.current_pieces[index];
            weight_selection -= piece.weight;
            if weight_selection >= 0 {
                continue;
            }
            if !piece.do_place(depth) || state.previous_piece == Some(piece.piece_type) {
                break;
            }
            let created = create_stronghold_piece(
                piece.piece_type,
                builder,
                random,
                foot_x,
                foot_y,
                foot_z,
                direction,
                depth,
            );
            if let Some(created) = created {
                state.current_pieces[index].place_count += 1;
                state.previous_piece = Some(piece.piece_type);
                if !state.current_pieces[index].is_valid() {
                    state.current_pieces.remove(index);
                }
                return Some(created);
            }
        }
    }

    let box_ = find_filler_corridor_box(builder, foot_x, foot_y, foot_z, direction)?;
    if box_.min_y() <= 1 {
        return None;
    }
    let steps = if direction == Direction::North || direction == Direction::South {
        box_.z_span()
    } else {
        box_.x_span()
    };
    Some(make_piece(
        StrongholdKind::FillerCorridor { steps },
        SmallDoorType::Opening,
        depth,
        box_,
        direction,
    ))
}

#[expect(
    clippy::too_many_arguments,
    reason = "mirrors vanilla generateAndAddPiece"
)]
fn generate_and_add_piece(
    state: &mut StrongholdGen,
    builder: &mut StructurePiecesBuilder,
    random: &mut RandomSource,
    foot_x: i32,
    foot_y: i32,
    foot_z: i32,
    direction: Direction,
    depth: i32,
) {
    if depth > MAX_DEPTH {
        return;
    }
    if (foot_x - state.start_bb.min_x()).abs() > 112
        || (foot_z - state.start_bb.min_z()).abs() > 112
    {
        return;
    }

    let new_piece = generate_piece_from_small_door(
        state,
        builder,
        random,
        foot_x,
        foot_y,
        foot_z,
        direction,
        depth + 1,
    );
    if let Some(new_piece) = new_piece {
        let index = builder.add_piece_indexed(new_piece);
        state.pending_children.push(index);
    }
}

#[expect(clippy::too_many_arguments, reason = "mirrors vanilla child helpers")]
fn child_forward(
    state: &mut StrongholdGen,
    builder: &mut StructurePiecesBuilder,
    random: &mut RandomSource,
    box_: BoundingBox,
    orientation: Direction,
    gen_depth: i32,
    x_off: i32,
    y_off: i32,
) {
    let (x, y, z, dir) = match orientation {
        Direction::North => (
            box_.min_x() + x_off,
            box_.min_y() + y_off,
            box_.min_z() - 1,
            orientation,
        ),
        Direction::South => (
            box_.min_x() + x_off,
            box_.min_y() + y_off,
            box_.max_z() + 1,
            orientation,
        ),
        Direction::West => (
            box_.min_x() - 1,
            box_.min_y() + y_off,
            box_.min_z() + x_off,
            orientation,
        ),
        Direction::East => (
            box_.max_x() + 1,
            box_.min_y() + y_off,
            box_.min_z() + x_off,
            orientation,
        ),
        _ => return,
    };
    generate_and_add_piece(state, builder, random, x, y, z, dir, gen_depth);
}

#[expect(clippy::too_many_arguments, reason = "mirrors vanilla child helpers")]
fn child_left(
    state: &mut StrongholdGen,
    builder: &mut StructurePiecesBuilder,
    random: &mut RandomSource,
    box_: BoundingBox,
    orientation: Direction,
    gen_depth: i32,
    y_off: i32,
    z_off: i32,
) {
    let (x, y, z, dir) = match orientation {
        Direction::North | Direction::South => (
            box_.min_x() - 1,
            box_.min_y() + y_off,
            box_.min_z() + z_off,
            Direction::West,
        ),
        Direction::West | Direction::East => (
            box_.min_x() + z_off,
            box_.min_y() + y_off,
            box_.min_z() - 1,
            Direction::North,
        ),
        _ => return,
    };
    generate_and_add_piece(state, builder, random, x, y, z, dir, gen_depth);
}

#[expect(clippy::too_many_arguments, reason = "mirrors vanilla child helpers")]
fn child_right(
    state: &mut StrongholdGen,
    builder: &mut StructurePiecesBuilder,
    random: &mut RandomSource,
    box_: BoundingBox,
    orientation: Direction,
    gen_depth: i32,
    y_off: i32,
    z_off: i32,
) {
    let (x, y, z, dir) = match orientation {
        Direction::North | Direction::South => (
            box_.max_x() + 1,
            box_.min_y() + y_off,
            box_.min_z() + z_off,
            Direction::East,
        ),
        Direction::West | Direction::East => (
            box_.min_x() + z_off,
            box_.min_y() + y_off,
            box_.max_z() + 1,
            Direction::South,
        ),
        _ => return,
    };
    generate_and_add_piece(state, builder, random, x, y, z, dir, gen_depth);
}

fn add_children(
    index: usize,
    state: &mut StrongholdGen,
    builder: &mut StructurePiecesBuilder,
    random: &mut RandomSource,
) {
    let piece = builder.piece_at(index);
    let box_ = piece.base.bounding_box;
    let gen_depth = piece.base.gen_depth;
    let Some(orientation) = piece.base.orientation() else {
        return;
    };
    let PieceKind::Stronghold(stronghold) = &piece.kind else {
        return;
    };

    match stronghold.kind {
        StrongholdKind::ChestCorridor { .. }
        | StrongholdKind::PrisonHall
        | StrongholdKind::StraightStairsDown => {
            child_forward(state, builder, random, box_, orientation, gen_depth, 1, 1);
        }
        StrongholdKind::StairsDown { is_source } => {
            if is_source {
                state.imposed_piece = Some(StrongholdPieceType::FiveCrossing);
            }
            child_forward(state, builder, random, box_, orientation, gen_depth, 1, 1);
        }
        StrongholdKind::LeftTurn => {
            if orientation == Direction::North || orientation == Direction::East {
                child_left(state, builder, random, box_, orientation, gen_depth, 1, 1);
            } else {
                child_right(state, builder, random, box_, orientation, gen_depth, 1, 1);
            }
        }
        StrongholdKind::RightTurn => {
            if orientation == Direction::North || orientation == Direction::East {
                child_right(state, builder, random, box_, orientation, gen_depth, 1, 1);
            } else {
                child_left(state, builder, random, box_, orientation, gen_depth, 1, 1);
            }
        }
        StrongholdKind::RoomCrossing { .. } => {
            child_forward(state, builder, random, box_, orientation, gen_depth, 4, 1);
            child_left(state, builder, random, box_, orientation, gen_depth, 1, 4);
            child_right(state, builder, random, box_, orientation, gen_depth, 1, 4);
        }
        StrongholdKind::Straight {
            left_child,
            right_child,
        } => {
            child_forward(state, builder, random, box_, orientation, gen_depth, 1, 1);
            if left_child {
                child_left(state, builder, random, box_, orientation, gen_depth, 1, 2);
            }
            if right_child {
                child_right(state, builder, random, box_, orientation, gen_depth, 1, 2);
            }
        }
        StrongholdKind::FiveCrossing {
            left_low,
            left_high,
            right_low,
            right_high,
        } => {
            let (mut z_off_a, mut z_off_b) = (3, 5);
            if orientation == Direction::West || orientation == Direction::North {
                z_off_a = 8 - z_off_a;
                z_off_b = 8 - z_off_b;
            }
            child_forward(state, builder, random, box_, orientation, gen_depth, 5, 1);
            if left_low {
                child_left(
                    state,
                    builder,
                    random,
                    box_,
                    orientation,
                    gen_depth,
                    z_off_a,
                    1,
                );
            }
            if left_high {
                child_left(
                    state,
                    builder,
                    random,
                    box_,
                    orientation,
                    gen_depth,
                    z_off_b,
                    7,
                );
            }
            if right_low {
                child_right(
                    state,
                    builder,
                    random,
                    box_,
                    orientation,
                    gen_depth,
                    z_off_a,
                    1,
                );
            }
            if right_high {
                child_right(
                    state,
                    builder,
                    random,
                    box_,
                    orientation,
                    gen_depth,
                    z_off_b,
                    7,
                );
            }
        }
        StrongholdKind::PortalRoom { .. } => {
            state.portal_room_placed = true;
        }
        StrongholdKind::Library { .. } | StrongholdKind::FillerCorridor { .. } => {}
    }
}

const fn stone_bricks() -> BlockState {
    Block::STONE_BRICKS.default_state()
}

fn smooth_stone_selector(random: &mut RandomSource, is_edge: bool) -> BlockState {
    if !is_edge {
        return cave_air();
    }
    let selection = random.next_f32();
    if selection < 0.2 {
        Block::CRACKED_STONE_BRICKS.default_state()
    } else if selection < 0.5 {
        Block::MOSSY_STONE_BRICKS.default_state()
    } else if selection < 0.55 {
        Block::INFESTED_STONE_BRICKS.default_state()
    } else {
        stone_bricks()
    }
}

impl StrongholdPiece {
    pub fn post_process(
        &mut self,
        base: &mut PieceBase,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        chunk_bb: BoundingBox,
    ) {
        let piece = &*base;
        let entry_door = self.entry_door;
        let orientation = piece.orientation().unwrap_or(Direction::North);
        match &mut self.kind {
            StrongholdKind::ChestCorridor { has_placed_chest } => {
                chest_corridor(
                    piece,
                    region,
                    random,
                    chunk_bb,
                    entry_door,
                    has_placed_chest,
                );
            }
            StrongholdKind::FillerCorridor { steps } => {
                filler_corridor(piece, region, chunk_bb, *steps);
            }
            StrongholdKind::FiveCrossing {
                left_low,
                left_high,
                right_low,
                right_high,
            } => five_crossing(
                piece,
                region,
                random,
                chunk_bb,
                entry_door,
                *left_low,
                *left_high,
                *right_low,
                *right_high,
            ),
            StrongholdKind::LeftTurn => {
                turn(
                    piece,
                    region,
                    random,
                    chunk_bb,
                    entry_door,
                    orientation,
                    true,
                );
            }
            StrongholdKind::RightTurn => {
                turn(
                    piece,
                    region,
                    random,
                    chunk_bb,
                    entry_door,
                    orientation,
                    false,
                );
            }
            StrongholdKind::Library { is_tall } => {
                library(piece, region, random, chunk_bb, entry_door, *is_tall);
            }
            StrongholdKind::PortalRoom { has_placed_spawner } => {
                portal_room(piece, region, random, chunk_bb, has_placed_spawner);
            }
            StrongholdKind::PrisonHall => {
                prison_hall(piece, region, random, chunk_bb, entry_door);
            }
            StrongholdKind::RoomCrossing { room_type } => {
                room_crossing(piece, region, random, chunk_bb, entry_door, *room_type);
            }
            StrongholdKind::StairsDown { .. } => {
                stairs_down(piece, region, random, chunk_bb, entry_door);
            }
            StrongholdKind::Straight {
                left_child,
                right_child,
            } => straight(
                piece,
                region,
                random,
                chunk_bb,
                entry_door,
                *left_child,
                *right_child,
            ),
            StrongholdKind::StraightStairsDown => {
                straight_stairs_down(piece, region, random, chunk_bb, entry_door);
            }
        }
    }
}

fn generate_small_door(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    chunk_bb: BoundingBox,
    door_type: SmallDoorType,
    foot_x: i32,
    foot_y: i32,
    foot_z: i32,
) {
    let bricks = stone_bricks();
    match door_type {
        SmallDoorType::Opening => {
            let cave_air = cave_air();
            base.generate_box(
                region,
                chunk_bb,
                foot_x,
                foot_y,
                foot_z,
                foot_x + 2,
                foot_y + 2,
                foot_z,
                cave_air,
                cave_air,
                false,
            );
        }
        SmallDoorType::WoodDoor => {
            base.place_block(region, bricks, foot_x, foot_y, foot_z, chunk_bb);
            base.place_block(region, bricks, foot_x, foot_y + 1, foot_z, chunk_bb);
            base.place_block(region, bricks, foot_x, foot_y + 2, foot_z, chunk_bb);
            base.place_block(region, bricks, foot_x + 1, foot_y + 2, foot_z, chunk_bb);
            base.place_block(region, bricks, foot_x + 2, foot_y + 2, foot_z, chunk_bb);
            base.place_block(region, bricks, foot_x + 2, foot_y + 1, foot_z, chunk_bb);
            base.place_block(region, bricks, foot_x + 2, foot_y, foot_z, chunk_bb);
            let door = Block::OAK_DOOR.default_state();
            base.place_block(region, door, foot_x + 1, foot_y, foot_z, chunk_bb);
            base.place_block(
                region,
                door.with(Properties::DOUBLE_BLOCK_HALF, DoubleBlockHalf::Upper),
                foot_x + 1,
                foot_y + 1,
                foot_z,
                chunk_bb,
            );
        }
        SmallDoorType::Grates => {
            let cave_air = cave_air();
            base.place_block(region, cave_air, foot_x + 1, foot_y, foot_z, chunk_bb);
            base.place_block(region, cave_air, foot_x + 1, foot_y + 1, foot_z, chunk_bb);
            let bars = Block::IRON_BARS.default_state();
            let west = bars.with(Properties::WEST, true);
            let east = bars.with(Properties::EAST, true);
            let we = west.with(Properties::EAST, true);
            base.place_block(region, west, foot_x, foot_y, foot_z, chunk_bb);
            base.place_block(region, west, foot_x, foot_y + 1, foot_z, chunk_bb);
            base.place_block(region, we, foot_x, foot_y + 2, foot_z, chunk_bb);
            base.place_block(region, we, foot_x + 1, foot_y + 2, foot_z, chunk_bb);
            base.place_block(region, we, foot_x + 2, foot_y + 2, foot_z, chunk_bb);
            base.place_block(region, east, foot_x + 2, foot_y + 1, foot_z, chunk_bb);
            base.place_block(region, east, foot_x + 2, foot_y, foot_z, chunk_bb);
        }
        SmallDoorType::IronDoor => {
            base.place_block(region, bricks, foot_x, foot_y, foot_z, chunk_bb);
            base.place_block(region, bricks, foot_x, foot_y + 1, foot_z, chunk_bb);
            base.place_block(region, bricks, foot_x, foot_y + 2, foot_z, chunk_bb);
            base.place_block(region, bricks, foot_x + 1, foot_y + 2, foot_z, chunk_bb);
            base.place_block(region, bricks, foot_x + 2, foot_y + 2, foot_z, chunk_bb);
            base.place_block(region, bricks, foot_x + 2, foot_y + 1, foot_z, chunk_bb);
            base.place_block(region, bricks, foot_x + 2, foot_y, foot_z, chunk_bb);
            let door = Block::IRON_DOOR.default_state();
            base.place_block(region, door, foot_x + 1, foot_y, foot_z, chunk_bb);
            base.place_block(
                region,
                door.with(Properties::DOUBLE_BLOCK_HALF, DoubleBlockHalf::Upper),
                foot_x + 1,
                foot_y + 1,
                foot_z,
                chunk_bb,
            );
            let button = Block::STONE_BUTTON.default_state();
            base.place_block(
                region,
                button.with(Properties::HORIZONTAL_FACING, Direction::North),
                foot_x + 2,
                foot_y + 1,
                foot_z + 1,
                chunk_bb,
            );
            base.place_block(
                region,
                button.with(Properties::HORIZONTAL_FACING, Direction::South),
                foot_x + 2,
                foot_y + 1,
                foot_z - 1,
                chunk_bb,
            );
        }
    }
}

#[expect(clippy::too_many_arguments, reason = "mirrors vanilla generateBox")]
fn selected_box(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    chunk_bb: BoundingBox,
    random: &mut RandomSource,
    x0: i32,
    y0: i32,
    z0: i32,
    x1: i32,
    y1: i32,
    z1: i32,
    skip_air: bool,
) {
    base.generate_box_selected(
        region,
        chunk_bb,
        x0,
        y0,
        z0,
        x1,
        y1,
        z1,
        skip_air,
        random,
        |random, _, _, _, is_edge| smooth_stone_selector(random, is_edge),
    );
}

fn chest_corridor(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    chunk_bb: BoundingBox,
    entry_door: SmallDoorType,
    has_placed_chest: &mut bool,
) {
    selected_box(base, region, chunk_bb, random, 0, 0, 0, 4, 4, 6, true);
    generate_small_door(base, region, chunk_bb, entry_door, 1, 1, 0);
    generate_small_door(base, region, chunk_bb, SmallDoorType::Opening, 1, 1, 6);
    let bricks = stone_bricks();
    base.generate_box(region, chunk_bb, 3, 1, 2, 3, 1, 4, bricks, bricks, false);
    let slab = Block::STONE_BRICK_SLAB.default_state();
    base.place_block(region, slab, 3, 1, 1, chunk_bb);
    base.place_block(region, slab, 3, 1, 5, chunk_bb);
    base.place_block(region, slab, 3, 2, 2, chunk_bb);
    base.place_block(region, slab, 3, 2, 4, chunk_bb);
    for z in 2..=4 {
        base.place_block(region, slab, 2, 1, z, chunk_bb);
    }
    if !*has_placed_chest && chunk_bb.is_inside(base.world_pos(3, 2, 3)) {
        *has_placed_chest = true;
        base.create_chest(region, chunk_bb, random, 3, 2, 3);
    }
}

fn filler_corridor(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    chunk_bb: BoundingBox,
    steps: i32,
) {
    let bricks = stone_bricks();
    let cave_air = cave_air();
    for i in 0..steps {
        for x in 0..=4 {
            base.place_block(region, bricks, x, 0, i, chunk_bb);
        }
        for y in 1..=3 {
            base.place_block(region, bricks, 0, y, i, chunk_bb);
            base.place_block(region, cave_air, 1, y, i, chunk_bb);
            base.place_block(region, cave_air, 2, y, i, chunk_bb);
            base.place_block(region, cave_air, 3, y, i, chunk_bb);
            base.place_block(region, bricks, 4, y, i, chunk_bb);
        }
        for x in 0..=4 {
            base.place_block(region, bricks, x, 4, i, chunk_bb);
        }
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "mirrors vanilla FiveCrossing::postProcess"
)]
fn five_crossing(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    chunk_bb: BoundingBox,
    entry_door: SmallDoorType,
    left_low: bool,
    left_high: bool,
    right_low: bool,
    right_high: bool,
) {
    let cave_air = cave_air();
    selected_box(base, region, chunk_bb, random, 0, 0, 0, 9, 8, 10, true);
    generate_small_door(base, region, chunk_bb, entry_door, 4, 3, 0);
    let gair = |x0, y0, z0, x1, y1, z1| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, cave_air, cave_air, false,
        );
    };
    if left_low {
        gair(0, 3, 1, 0, 5, 3);
    }
    if right_low {
        gair(9, 3, 1, 9, 5, 3);
    }
    if left_high {
        gair(0, 5, 7, 0, 7, 9);
    }
    if right_high {
        gair(9, 5, 7, 9, 7, 9);
    }
    gair(5, 1, 10, 7, 3, 10);
    selected_box(base, region, chunk_bb, random, 1, 2, 1, 8, 2, 6, false);
    selected_box(base, region, chunk_bb, random, 4, 1, 5, 4, 4, 9, false);
    selected_box(base, region, chunk_bb, random, 8, 1, 5, 8, 4, 9, false);
    selected_box(base, region, chunk_bb, random, 1, 4, 7, 3, 4, 9, false);
    selected_box(base, region, chunk_bb, random, 1, 3, 5, 3, 3, 6, false);
    let slab = Block::SMOOTH_STONE_SLAB.default_state();
    let gslab = |x0, y0, z0, x1, y1, z1, state| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, false,
        );
    };
    gslab(1, 3, 4, 3, 3, 4, slab);
    gslab(1, 4, 6, 3, 4, 6, slab);
    selected_box(base, region, chunk_bb, random, 5, 1, 7, 7, 1, 8, false);
    gslab(5, 1, 9, 7, 1, 9, slab);
    gslab(5, 2, 7, 7, 2, 7, slab);
    gslab(4, 5, 7, 4, 5, 9, slab);
    gslab(8, 5, 7, 8, 5, 9, slab);
    let double_slab = slab.with(Properties::SLAB_TYPE, SlabType::Double);
    gslab(5, 5, 7, 7, 5, 9, double_slab);
    base.place_block(
        region,
        Block::WALL_TORCH
            .default_state()
            .with(Properties::HORIZONTAL_FACING, Direction::South),
        6,
        5,
        6,
        chunk_bb,
    );
}

fn turn(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    chunk_bb: BoundingBox,
    entry_door: SmallDoorType,
    orientation: Direction,
    is_left: bool,
) {
    let cave_air = cave_air();
    selected_box(base, region, chunk_bb, random, 0, 0, 0, 4, 4, 4, true);
    generate_small_door(base, region, chunk_bb, entry_door, 1, 1, 0);
    let north_or_east = orientation == Direction::North || orientation == Direction::East;
    let open_east = if is_left {
        !north_or_east
    } else {
        north_or_east
    };
    let x = if open_east { 4 } else { 0 };
    base.generate_box(
        region, chunk_bb, x, 1, 1, x, 3, 3, cave_air, cave_air, false,
    );
}

#[expect(
    clippy::too_many_lines,
    reason = "mirrors vanilla Library::postProcess"
)]
fn library(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    chunk_bb: BoundingBox,
    entry_door: SmallDoorType,
    is_tall: bool,
) {
    let current_height = if is_tall { 11 } else { 6 };
    selected_box(
        base,
        region,
        chunk_bb,
        random,
        0,
        0,
        0,
        13,
        current_height - 1,
        14,
        true,
    );
    generate_small_door(base, region, chunk_bb, entry_door, 4, 1, 0);
    let cobweb = Block::COBWEB.default_state();
    base.generate_maybe_box(
        region, chunk_bb, random, 0.07, 2, 1, 1, 11, 4, 13, cobweb, cobweb, false, false,
    );

    let planks = Block::OAK_PLANKS.default_state();
    let bookshelf = Block::BOOKSHELF.default_state();
    let gbox = |x0, y0, z0, x1, y1, z1, state| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, false,
        );
    };
    let wall_torch = Block::WALL_TORCH.default_state();

    for d in 1..=13 {
        if (d - 1) % 4 == 0 {
            gbox(1, 1, d, 1, 4, d, planks);
            gbox(12, 1, d, 12, 4, d, planks);
            base.place_block(
                region,
                wall_torch.with(Properties::HORIZONTAL_FACING, Direction::East),
                2,
                3,
                d,
                chunk_bb,
            );
            base.place_block(
                region,
                wall_torch.with(Properties::HORIZONTAL_FACING, Direction::West),
                11,
                3,
                d,
                chunk_bb,
            );
            if is_tall {
                gbox(1, 6, d, 1, 9, d, planks);
                gbox(12, 6, d, 12, 9, d, planks);
            }
        } else {
            gbox(1, 1, d, 1, 4, d, bookshelf);
            gbox(12, 1, d, 12, 4, d, bookshelf);
            if is_tall {
                gbox(1, 6, d, 1, 9, d, bookshelf);
                gbox(12, 6, d, 12, 9, d, bookshelf);
            }
        }
    }

    let mut d = 3;
    while d < 12 {
        gbox(3, 1, d, 4, 3, d, bookshelf);
        gbox(6, 1, d, 7, 3, d, bookshelf);
        gbox(9, 1, d, 10, 3, d, bookshelf);
        d += 2;
    }

    if is_tall {
        gbox(1, 5, 1, 3, 5, 13, planks);
        gbox(10, 5, 1, 12, 5, 13, planks);
        gbox(4, 5, 1, 9, 5, 2, planks);
        gbox(4, 5, 12, 9, 5, 13, planks);
        base.place_block(region, planks, 9, 5, 11, chunk_bb);
        base.place_block(region, planks, 8, 5, 11, chunk_bb);
        base.place_block(region, planks, 9, 5, 10, chunk_bb);

        let fence = Block::OAK_FENCE.default_state();
        let we_fence = fence
            .with(Properties::WEST, true)
            .with(Properties::EAST, true);
        let ns_fence = fence
            .with(Properties::NORTH, true)
            .with(Properties::SOUTH, true);
        gbox(3, 6, 3, 3, 6, 11, ns_fence);
        gbox(10, 6, 3, 10, 6, 9, ns_fence);
        gbox(4, 6, 2, 9, 6, 2, we_fence);
        gbox(4, 6, 12, 7, 6, 12, we_fence);
        let ne = fence
            .with(Properties::NORTH, true)
            .with(Properties::EAST, true);
        let se = fence
            .with(Properties::SOUTH, true)
            .with(Properties::EAST, true);
        let nw = fence
            .with(Properties::NORTH, true)
            .with(Properties::WEST, true);
        let sw = fence
            .with(Properties::SOUTH, true)
            .with(Properties::WEST, true);
        base.place_block(region, ne, 3, 6, 2, chunk_bb);
        base.place_block(region, se, 3, 6, 12, chunk_bb);
        base.place_block(region, nw, 10, 6, 2, chunk_bb);

        for i in 0..=2 {
            base.place_block(region, sw, 8 + i, 6, 12 - i, chunk_bb);
            if i != 2 {
                base.place_block(region, ne, 8 + i, 6, 11 - i, chunk_bb);
            }
        }

        let ladder = Block::LADDER
            .default_state()
            .with(Properties::HORIZONTAL_FACING, Direction::South);
        for y in 1..=7 {
            base.place_block(region, ladder, 10, y, 13, chunk_bb);
        }

        let e_fence = fence.with(Properties::EAST, true);
        let w_fence = fence.with(Properties::WEST, true);
        base.place_block(region, e_fence, 6, 9, 7, chunk_bb);
        base.place_block(region, w_fence, 7, 9, 7, chunk_bb);
        base.place_block(region, e_fence, 6, 8, 7, chunk_bb);
        base.place_block(region, w_fence, 7, 8, 7, chunk_bb);
        let nswe_fence = ns_fence
            .with(Properties::WEST, true)
            .with(Properties::EAST, true);
        base.place_block(region, nswe_fence, 6, 7, 7, chunk_bb);
        base.place_block(region, nswe_fence, 7, 7, 7, chunk_bb);
        base.place_block(region, e_fence, 5, 7, 7, chunk_bb);
        base.place_block(region, w_fence, 8, 7, 7, chunk_bb);
        base.place_block(
            region,
            e_fence.with(Properties::NORTH, true),
            6,
            7,
            6,
            chunk_bb,
        );
        base.place_block(
            region,
            e_fence.with(Properties::SOUTH, true),
            6,
            7,
            8,
            chunk_bb,
        );
        base.place_block(
            region,
            w_fence.with(Properties::NORTH, true),
            7,
            7,
            6,
            chunk_bb,
        );
        base.place_block(
            region,
            w_fence.with(Properties::SOUTH, true),
            7,
            7,
            8,
            chunk_bb,
        );
        let torch = Block::TORCH.default_state();
        base.place_block(region, torch, 5, 8, 7, chunk_bb);
        base.place_block(region, torch, 8, 8, 7, chunk_bb);
        base.place_block(region, torch, 6, 8, 6, chunk_bb);
        base.place_block(region, torch, 6, 8, 8, chunk_bb);
        base.place_block(region, torch, 7, 8, 6, chunk_bb);
        base.place_block(region, torch, 7, 8, 8, chunk_bb);
    }

    base.create_chest(region, chunk_bb, random, 3, 3, 5);
    if is_tall {
        base.place_block(region, cave_air(), 12, 9, 1, chunk_bb);
        base.create_chest(region, chunk_bb, random, 12, 8, 1);
    }
}

fn portal_room(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    chunk_bb: BoundingBox,
    has_placed_spawner: &mut bool,
) {
    selected_box(base, region, chunk_bb, random, 0, 0, 0, 10, 7, 15, false);
    generate_small_door(base, region, chunk_bb, SmallDoorType::Grates, 4, 1, 0);
    selected_box(base, region, chunk_bb, random, 1, 6, 1, 1, 6, 14, false);
    selected_box(base, region, chunk_bb, random, 9, 6, 1, 9, 6, 14, false);
    selected_box(base, region, chunk_bb, random, 2, 6, 1, 8, 6, 2, false);
    selected_box(base, region, chunk_bb, random, 2, 6, 14, 8, 6, 14, false);
    selected_box(base, region, chunk_bb, random, 1, 1, 1, 2, 1, 4, false);
    selected_box(base, region, chunk_bb, random, 8, 1, 1, 9, 1, 4, false);
    let lava = Block::LAVA.default_state();
    let gbox = |x0, y0, z0, x1, y1, z1, state| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, false,
        );
    };
    gbox(1, 1, 1, 1, 1, 3, lava);
    gbox(9, 1, 1, 9, 1, 3, lava);
    selected_box(base, region, chunk_bb, random, 3, 1, 8, 7, 1, 12, false);
    gbox(4, 1, 9, 6, 1, 11, lava);

    let bars = Block::IRON_BARS.default_state();
    let ns_bars = bars
        .with(Properties::NORTH, true)
        .with(Properties::SOUTH, true);
    let we_bars = bars
        .with(Properties::WEST, true)
        .with(Properties::EAST, true);
    let mut z = 3;
    while z < 14 {
        gbox(0, 3, z, 0, 4, z, ns_bars);
        gbox(10, 3, z, 10, 4, z, ns_bars);
        z += 2;
    }
    let mut x = 2;
    while x < 9 {
        gbox(x, 3, 15, x, 4, 15, we_bars);
        x += 2;
    }

    let stairs = Block::STONE_BRICK_STAIRS
        .default_state()
        .with(Properties::HORIZONTAL_FACING, Direction::North);
    selected_box(base, region, chunk_bb, random, 4, 1, 5, 6, 1, 7, false);
    selected_box(base, region, chunk_bb, random, 4, 2, 6, 6, 2, 7, false);
    selected_box(base, region, chunk_bb, random, 4, 3, 7, 6, 3, 7, false);
    for x in 4..=6 {
        base.place_block(region, stairs, x, 1, 4, chunk_bb);
        base.place_block(region, stairs, x, 2, 5, chunk_bb);
        base.place_block(region, stairs, x, 3, 6, chunk_bb);
    }

    let frame = Block::END_PORTAL_FRAME.default_state();
    let north_frame = frame.with(Properties::HORIZONTAL_FACING, Direction::North);
    let south_frame = frame.with(Properties::HORIZONTAL_FACING, Direction::South);
    let east_frame = frame.with(Properties::HORIZONTAL_FACING, Direction::East);
    let west_frame = frame.with(Properties::HORIZONTAL_FACING, Direction::West);
    let mut all_eyes = true;
    let mut eyes = [false; 12];
    for eye in &mut eyes {
        *eye = random.next_f32() > 0.9;
        all_eyes &= *eye;
    }

    let eye = |state: BlockState, has_eye: bool| state.with(Properties::EYE, has_eye);
    base.place_block(region, eye(north_frame, eyes[0]), 4, 3, 8, chunk_bb);
    base.place_block(region, eye(north_frame, eyes[1]), 5, 3, 8, chunk_bb);
    base.place_block(region, eye(north_frame, eyes[2]), 6, 3, 8, chunk_bb);
    base.place_block(region, eye(south_frame, eyes[3]), 4, 3, 12, chunk_bb);
    base.place_block(region, eye(south_frame, eyes[4]), 5, 3, 12, chunk_bb);
    base.place_block(region, eye(south_frame, eyes[5]), 6, 3, 12, chunk_bb);
    base.place_block(region, eye(east_frame, eyes[6]), 3, 3, 9, chunk_bb);
    base.place_block(region, eye(east_frame, eyes[7]), 3, 3, 10, chunk_bb);
    base.place_block(region, eye(east_frame, eyes[8]), 3, 3, 11, chunk_bb);
    base.place_block(region, eye(west_frame, eyes[9]), 7, 3, 9, chunk_bb);
    base.place_block(region, eye(west_frame, eyes[10]), 7, 3, 10, chunk_bb);
    base.place_block(region, eye(west_frame, eyes[11]), 7, 3, 11, chunk_bb);

    if all_eyes {
        let portal = Block::END_PORTAL.default_state();
        for z in 9..=11 {
            for x in 4..=6 {
                base.place_block(region, portal, x, 3, z, chunk_bb);
            }
        }
    }

    if !*has_placed_spawner {
        let pos = base.world_pos(5, 3, 6);
        if chunk_bb.is_inside(pos) {
            *has_placed_spawner = true;
            region.set_block(pos, Block::SPAWNER.default_state());
        }
    }
}

fn prison_hall(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    chunk_bb: BoundingBox,
    entry_door: SmallDoorType,
) {
    let cave_air = cave_air();
    selected_box(base, region, chunk_bb, random, 0, 0, 0, 8, 4, 10, true);
    generate_small_door(base, region, chunk_bb, entry_door, 1, 1, 0);
    base.generate_box(
        region, chunk_bb, 1, 1, 10, 3, 3, 10, cave_air, cave_air, false,
    );
    selected_box(base, region, chunk_bb, random, 4, 1, 1, 4, 3, 1, false);
    selected_box(base, region, chunk_bb, random, 4, 1, 3, 4, 3, 3, false);
    selected_box(base, region, chunk_bb, random, 4, 1, 7, 4, 3, 7, false);
    selected_box(base, region, chunk_bb, random, 4, 1, 9, 4, 3, 9, false);

    let bars = Block::IRON_BARS.default_state();
    let ns = bars
        .with(Properties::NORTH, true)
        .with(Properties::SOUTH, true);
    let nse = ns.with(Properties::EAST, true);
    let we = bars
        .with(Properties::WEST, true)
        .with(Properties::EAST, true);
    for y in 1..=3 {
        base.place_block(region, ns, 4, y, 4, chunk_bb);
        base.place_block(region, nse, 4, y, 5, chunk_bb);
        base.place_block(region, ns, 4, y, 6, chunk_bb);
        base.place_block(region, we, 5, y, 5, chunk_bb);
        base.place_block(region, we, 6, y, 5, chunk_bb);
        base.place_block(region, we, 7, y, 5, chunk_bb);
    }
    base.place_block(region, ns, 4, 3, 2, chunk_bb);
    base.place_block(region, ns, 4, 3, 8, chunk_bb);

    let door = Block::IRON_DOOR
        .default_state()
        .with(Properties::HORIZONTAL_FACING, Direction::West);
    let door_top = door.with(Properties::DOUBLE_BLOCK_HALF, DoubleBlockHalf::Upper);
    base.place_block(region, door, 4, 1, 2, chunk_bb);
    base.place_block(region, door_top, 4, 2, 2, chunk_bb);
    base.place_block(region, door, 4, 1, 8, chunk_bb);
    base.place_block(region, door_top, 4, 2, 8, chunk_bb);
}

#[expect(
    clippy::too_many_lines,
    reason = "mirrors vanilla RoomCrossing::postProcess"
)]
fn room_crossing(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    chunk_bb: BoundingBox,
    entry_door: SmallDoorType,
    room_type: i32,
) {
    let cave_air = cave_air();
    selected_box(base, region, chunk_bb, random, 0, 0, 0, 10, 6, 10, true);
    generate_small_door(base, region, chunk_bb, entry_door, 4, 1, 0);
    let gair = |x0, y0, z0, x1, y1, z1| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, cave_air, cave_air, false,
        );
    };
    gair(4, 1, 10, 6, 3, 10);
    gair(0, 1, 4, 0, 3, 6);
    gair(10, 1, 4, 10, 3, 6);

    let bricks = stone_bricks();
    let slab = Block::SMOOTH_STONE_SLAB.default_state();
    let wall_torch = Block::WALL_TORCH.default_state();
    let cobblestone = Block::COBBLESTONE.default_state();
    let planks = Block::OAK_PLANKS.default_state();

    match room_type {
        0 => {
            base.place_block(region, bricks, 5, 1, 5, chunk_bb);
            base.place_block(region, bricks, 5, 2, 5, chunk_bb);
            base.place_block(region, bricks, 5, 3, 5, chunk_bb);
            base.place_block(
                region,
                wall_torch.with(Properties::HORIZONTAL_FACING, Direction::West),
                4,
                3,
                5,
                chunk_bb,
            );
            base.place_block(
                region,
                wall_torch.with(Properties::HORIZONTAL_FACING, Direction::East),
                6,
                3,
                5,
                chunk_bb,
            );
            base.place_block(
                region,
                wall_torch.with(Properties::HORIZONTAL_FACING, Direction::South),
                5,
                3,
                4,
                chunk_bb,
            );
            base.place_block(
                region,
                wall_torch.with(Properties::HORIZONTAL_FACING, Direction::North),
                5,
                3,
                6,
                chunk_bb,
            );
            base.place_block(region, slab, 4, 1, 4, chunk_bb);
            base.place_block(region, slab, 4, 1, 5, chunk_bb);
            base.place_block(region, slab, 4, 1, 6, chunk_bb);
            base.place_block(region, slab, 6, 1, 4, chunk_bb);
            base.place_block(region, slab, 6, 1, 5, chunk_bb);
            base.place_block(region, slab, 6, 1, 6, chunk_bb);
            base.place_block(region, slab, 5, 1, 4, chunk_bb);
            base.place_block(region, slab, 5, 1, 6, chunk_bb);
        }
        1 => {
            for i in 0..5 {
                base.place_block(region, bricks, 3, 1, 3 + i, chunk_bb);
                base.place_block(region, bricks, 7, 1, 3 + i, chunk_bb);
                base.place_block(region, bricks, 3 + i, 1, 3, chunk_bb);
                base.place_block(region, bricks, 3 + i, 1, 7, chunk_bb);
            }
            base.place_block(region, bricks, 5, 1, 5, chunk_bb);
            base.place_block(region, bricks, 5, 2, 5, chunk_bb);
            base.place_block(region, bricks, 5, 3, 5, chunk_bb);
            base.place_block(region, Block::WATER.default_state(), 5, 4, 5, chunk_bb);
        }
        2 => {
            for z in 1..=9 {
                base.place_block(region, cobblestone, 1, 3, z, chunk_bb);
                base.place_block(region, cobblestone, 9, 3, z, chunk_bb);
            }
            for x in 1..=9 {
                base.place_block(region, cobblestone, x, 3, 1, chunk_bb);
                base.place_block(region, cobblestone, x, 3, 9, chunk_bb);
            }
            base.place_block(region, cobblestone, 5, 1, 4, chunk_bb);
            base.place_block(region, cobblestone, 5, 1, 6, chunk_bb);
            base.place_block(region, cobblestone, 5, 3, 4, chunk_bb);
            base.place_block(region, cobblestone, 5, 3, 6, chunk_bb);
            base.place_block(region, cobblestone, 4, 1, 5, chunk_bb);
            base.place_block(region, cobblestone, 6, 1, 5, chunk_bb);
            base.place_block(region, cobblestone, 4, 3, 5, chunk_bb);
            base.place_block(region, cobblestone, 6, 3, 5, chunk_bb);
            for y in 1..=3 {
                base.place_block(region, cobblestone, 4, y, 4, chunk_bb);
                base.place_block(region, cobblestone, 6, y, 4, chunk_bb);
                base.place_block(region, cobblestone, 4, y, 6, chunk_bb);
                base.place_block(region, cobblestone, 6, y, 6, chunk_bb);
            }
            base.place_block(region, wall_torch, 5, 3, 5, chunk_bb);
            for z in 2..=8 {
                base.place_block(region, planks, 2, 3, z, chunk_bb);
                base.place_block(region, planks, 3, 3, z, chunk_bb);
                if z <= 3 || z >= 7 {
                    base.place_block(region, planks, 4, 3, z, chunk_bb);
                    base.place_block(region, planks, 5, 3, z, chunk_bb);
                    base.place_block(region, planks, 6, 3, z, chunk_bb);
                }
                base.place_block(region, planks, 7, 3, z, chunk_bb);
                base.place_block(region, planks, 8, 3, z, chunk_bb);
            }
            let ladder = Block::LADDER
                .default_state()
                .with(Properties::HORIZONTAL_FACING, Direction::West);
            base.place_block(region, ladder, 9, 1, 3, chunk_bb);
            base.place_block(region, ladder, 9, 2, 3, chunk_bb);
            base.place_block(region, ladder, 9, 3, 3, chunk_bb);
            base.create_chest(region, chunk_bb, random, 3, 4, 8);
        }
        _ => {}
    }
}

fn stairs_down(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    chunk_bb: BoundingBox,
    entry_door: SmallDoorType,
) {
    selected_box(base, region, chunk_bb, random, 0, 0, 0, 4, 10, 4, true);
    generate_small_door(base, region, chunk_bb, entry_door, 1, 7, 0);
    generate_small_door(base, region, chunk_bb, SmallDoorType::Opening, 1, 1, 4);
    let bricks = stone_bricks();
    let slab = Block::SMOOTH_STONE_SLAB.default_state();
    base.place_block(region, bricks, 2, 6, 1, chunk_bb);
    base.place_block(region, bricks, 1, 5, 1, chunk_bb);
    base.place_block(region, slab, 1, 6, 1, chunk_bb);
    base.place_block(region, bricks, 1, 5, 2, chunk_bb);
    base.place_block(region, bricks, 1, 4, 3, chunk_bb);
    base.place_block(region, slab, 1, 5, 3, chunk_bb);
    base.place_block(region, bricks, 2, 4, 3, chunk_bb);
    base.place_block(region, bricks, 3, 3, 3, chunk_bb);
    base.place_block(region, slab, 3, 4, 3, chunk_bb);
    base.place_block(region, bricks, 3, 3, 2, chunk_bb);
    base.place_block(region, bricks, 3, 2, 1, chunk_bb);
    base.place_block(region, slab, 3, 3, 1, chunk_bb);
    base.place_block(region, bricks, 2, 2, 1, chunk_bb);
    base.place_block(region, bricks, 1, 1, 1, chunk_bb);
    base.place_block(region, slab, 1, 2, 1, chunk_bb);
    base.place_block(region, bricks, 1, 1, 2, chunk_bb);
    base.place_block(region, slab, 1, 1, 3, chunk_bb);
}

fn straight(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    chunk_bb: BoundingBox,
    entry_door: SmallDoorType,
    left_child: bool,
    right_child: bool,
) {
    let cave_air = cave_air();
    selected_box(base, region, chunk_bb, random, 0, 0, 0, 4, 4, 6, true);
    generate_small_door(base, region, chunk_bb, entry_door, 1, 1, 0);
    generate_small_door(base, region, chunk_bb, SmallDoorType::Opening, 1, 1, 6);
    let wall_torch = Block::WALL_TORCH.default_state();
    let east_torch = wall_torch.with(Properties::HORIZONTAL_FACING, Direction::East);
    let west_torch = wall_torch.with(Properties::HORIZONTAL_FACING, Direction::West);
    base.maybe_generate_block(region, chunk_bb, random, 0.1, 1, 2, 1, east_torch);
    base.maybe_generate_block(region, chunk_bb, random, 0.1, 3, 2, 1, west_torch);
    base.maybe_generate_block(region, chunk_bb, random, 0.1, 1, 2, 5, east_torch);
    base.maybe_generate_block(region, chunk_bb, random, 0.1, 3, 2, 5, west_torch);
    if left_child {
        base.generate_box(
            region, chunk_bb, 0, 1, 2, 0, 3, 4, cave_air, cave_air, false,
        );
    }
    if right_child {
        base.generate_box(
            region, chunk_bb, 4, 1, 2, 4, 3, 4, cave_air, cave_air, false,
        );
    }
}

fn straight_stairs_down(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    chunk_bb: BoundingBox,
    entry_door: SmallDoorType,
) {
    selected_box(base, region, chunk_bb, random, 0, 0, 0, 4, 10, 7, true);
    generate_small_door(base, region, chunk_bb, entry_door, 1, 7, 0);
    generate_small_door(base, region, chunk_bb, SmallDoorType::Opening, 1, 1, 7);
    let stairs = Block::COBBLESTONE_STAIRS
        .default_state()
        .with(Properties::HORIZONTAL_FACING, Direction::South);
    let bricks = stone_bricks();
    for i in 0..6 {
        base.place_block(region, stairs, 1, 6 - i, 1 + i, chunk_bb);
        base.place_block(region, stairs, 2, 6 - i, 1 + i, chunk_bb);
        base.place_block(region, stairs, 3, 6 - i, 1 + i, chunk_bb);
        if i < 5 {
            base.place_block(region, bricks, 1, 5 - i, 1 + i, chunk_bb);
            base.place_block(region, bricks, 2, 5 - i, 1 + i, chunk_bb);
            base.place_block(region, bricks, 3, 5 - i, 1 + i, chunk_bb);
        }
    }
}
