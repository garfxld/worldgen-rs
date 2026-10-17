use shulkr::world::block::{Block, BlockState, Properties};

use crate::util::rng::RandomSource;
use crate::util::rng::legacy_random::LegacyRandomSource;
use crate::world::worldgen::region::WorldGenRegion;
use crate::world::worldgen::structure::pieces::structure_pieces_builder::StructurePiecesBuilder;
use crate::world::worldgen::structure::structure_piece::{
    PieceBase, PieceKind, StructurePiece, air,
};
use shulkr::util::BoundingBox;
use shulkr::util::Direction;

const MAX_DEPTH: i32 = 30;
const LOWEST_Y_POSITION: i32 = 10;
pub const MAGIC_START_Y: i32 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgePieceType {
    BridgeStraight,
    BridgeCrossing,
    RoomCrossing,
    StairsRoom,
    MonsterThrone,
    CastleEntrance,
    CastleSmallCorridor,
    CastleSmallCorridorCrossing,
    CastleSmallCorridorRightTurn,
    CastleSmallCorridorLeftTurn,
    CastleCorridorStairs,
    CastleCorridorTBalcony,
    CastleStalkRoom,
}

#[derive(Debug, Clone, Copy)]
struct PieceWeight {
    id: u8,
    piece_type: BridgePieceType,
    weight: i32,
    place_count: i32,
    max_place_count: i32,
    allow_in_row: bool,
}

impl PieceWeight {
    const fn is_valid(self) -> bool {
        self.max_place_count == 0 || self.place_count < self.max_place_count
    }

    const fn do_place(self) -> bool {
        self.is_valid()
    }
}

fn bridge_piece_weights() -> Vec<PieceWeight> {
    vec![
        PieceWeight {
            id: 0,
            piece_type: BridgePieceType::BridgeStraight,
            weight: 30,
            place_count: 0,
            max_place_count: 0,
            allow_in_row: true,
        },
        PieceWeight {
            id: 1,
            piece_type: BridgePieceType::BridgeCrossing,
            weight: 10,
            place_count: 0,
            max_place_count: 4,
            allow_in_row: false,
        },
        PieceWeight {
            id: 2,
            piece_type: BridgePieceType::RoomCrossing,
            weight: 10,
            place_count: 0,
            max_place_count: 4,
            allow_in_row: false,
        },
        PieceWeight {
            id: 3,
            piece_type: BridgePieceType::StairsRoom,
            weight: 10,
            place_count: 0,
            max_place_count: 3,
            allow_in_row: false,
        },
        PieceWeight {
            id: 4,
            piece_type: BridgePieceType::MonsterThrone,
            weight: 5,
            place_count: 0,
            max_place_count: 2,
            allow_in_row: false,
        },
        PieceWeight {
            id: 5,
            piece_type: BridgePieceType::CastleEntrance,
            weight: 5,
            place_count: 0,
            max_place_count: 1,
            allow_in_row: false,
        },
    ]
}

fn castle_piece_weights() -> Vec<PieceWeight> {
    vec![
        PieceWeight {
            id: 100,
            piece_type: BridgePieceType::CastleSmallCorridor,
            weight: 25,
            place_count: 0,
            max_place_count: 0,
            allow_in_row: true,
        },
        PieceWeight {
            id: 101,
            piece_type: BridgePieceType::CastleSmallCorridorCrossing,
            weight: 15,
            place_count: 0,
            max_place_count: 5,
            allow_in_row: false,
        },
        PieceWeight {
            id: 102,
            piece_type: BridgePieceType::CastleSmallCorridorRightTurn,
            weight: 5,
            place_count: 0,
            max_place_count: 10,
            allow_in_row: false,
        },
        PieceWeight {
            id: 103,
            piece_type: BridgePieceType::CastleSmallCorridorLeftTurn,
            weight: 5,
            place_count: 0,
            max_place_count: 10,
            allow_in_row: false,
        },
        PieceWeight {
            id: 104,
            piece_type: BridgePieceType::CastleCorridorStairs,
            weight: 10,
            place_count: 0,
            max_place_count: 3,
            allow_in_row: true,
        },
        PieceWeight {
            id: 105,
            piece_type: BridgePieceType::CastleCorridorTBalcony,
            weight: 7,
            place_count: 0,
            max_place_count: 2,
            allow_in_row: false,
        },
        PieceWeight {
            id: 106,
            piece_type: BridgePieceType::CastleStalkRoom,
            weight: 5,
            place_count: 0,
            max_place_count: 2,
            allow_in_row: false,
        },
    ]
}

#[derive(Debug)]
pub enum NetherBridgeKind {
    BridgeStraight,
    BridgeCrossing,
    BridgeEndFiller { self_seed: i32 },
    RoomCrossing,
    StairsRoom,
    MonsterThrone { has_placed_spawner: bool },
    CastleEntrance,
    CastleSmallCorridor,
    CastleSmallCorridorCrossing,
    CastleSmallCorridorRightTurn { is_needing_chest: bool },
    CastleSmallCorridorLeftTurn { is_needing_chest: bool },
    CastleCorridorStairs,
    CastleCorridorTBalcony,
    CastleStalkRoom,
}

#[derive(Debug)]
pub struct NetherBridgePiece {
    pub kind: NetherBridgeKind,
}

struct FortressGen {
    start_bb: BoundingBox,
    available_bridge_pieces: Vec<PieceWeight>,
    available_castle_pieces: Vec<PieceWeight>,
    previous_piece: Option<u8>,
    pending_children: Vec<usize>,
}

const fn make_piece(
    kind: NetherBridgeKind,
    gen_depth: i32,
    bounding_box: BoundingBox,
    direction: Direction,
) -> StructurePiece {
    let mut base = PieceBase::new(gen_depth, bounding_box);
    base.set_orientation(Some(direction));
    StructurePiece::new(base, PieceKind::NetherBridge(NetherBridgePiece { kind }))
}

const fn is_ok_box(box_: BoundingBox) -> bool {
    box_.min_y() > LOWEST_Y_POSITION
}

fn try_box(builder: &StructurePiecesBuilder, box_: BoundingBox) -> Option<BoundingBox> {
    (is_ok_box(box_) && builder.find_collision_piece(box_).is_none()).then_some(box_)
}

pub fn new_start_piece(random: &mut RandomSource, west: i32, north: i32) -> StructurePiece {
    let direction = random.next_horizontal_direction();
    let box_ = PieceBase::make_bounding_box(west, MAGIC_START_Y, north, direction, 19, 10, 19);
    make_piece(NetherBridgeKind::BridgeCrossing, 0, box_, direction)
}

pub fn generate_pieces(
    builder: &mut StructurePiecesBuilder,
    random: &mut RandomSource,
    block_x: i32,
    block_z: i32,
) {
    let start = new_start_piece(random, block_x, block_z);
    let start_bb = start.bounding_box();
    let start_index = builder.add_piece_indexed(start);
    let mut state = FortressGen {
        start_bb,
        available_bridge_pieces: bridge_piece_weights(),
        available_castle_pieces: castle_piece_weights(),
        previous_piece: None,
        pending_children: Vec::new(),
    };

    add_children(start_index, &mut state, builder, random);

    while !state.pending_children.is_empty() {
        let index = random.next_i32_bounded(state.pending_children.len() as i32) as usize;
        let piece_index = state.pending_children.remove(index);
        add_children(piece_index, &mut state, builder, random);
    }

    builder.move_inside_heights(random, 48, 70);
}

#[expect(
    clippy::too_many_arguments,
    reason = "mirrors the vanilla bridge piece factories"
)]
fn create_bridge_piece(
    piece_type: BridgePieceType,
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
        BridgePieceType::BridgeStraight => {
            let box_ = try_box(builder, orient(-1, -3, 0, 5, 10, 19))?;
            Some(make_piece(
                NetherBridgeKind::BridgeStraight,
                gen_depth,
                box_,
                direction,
            ))
        }
        BridgePieceType::BridgeCrossing => {
            let box_ = try_box(builder, orient(-8, -3, 0, 19, 10, 19))?;
            Some(make_piece(
                NetherBridgeKind::BridgeCrossing,
                gen_depth,
                box_,
                direction,
            ))
        }
        BridgePieceType::RoomCrossing => {
            let box_ = try_box(builder, orient(-2, 0, 0, 7, 9, 7))?;
            Some(make_piece(
                NetherBridgeKind::RoomCrossing,
                gen_depth,
                box_,
                direction,
            ))
        }
        BridgePieceType::StairsRoom => {
            let box_ = try_box(builder, orient(-2, 0, 0, 7, 11, 7))?;
            Some(make_piece(
                NetherBridgeKind::StairsRoom,
                gen_depth,
                box_,
                direction,
            ))
        }
        BridgePieceType::MonsterThrone => {
            let box_ = try_box(builder, orient(-2, 0, 0, 7, 8, 9))?;
            Some(make_piece(
                NetherBridgeKind::MonsterThrone {
                    has_placed_spawner: false,
                },
                gen_depth,
                box_,
                direction,
            ))
        }
        BridgePieceType::CastleEntrance => {
            let box_ = try_box(builder, orient(-5, -3, 0, 13, 14, 13))?;
            Some(make_piece(
                NetherBridgeKind::CastleEntrance,
                gen_depth,
                box_,
                direction,
            ))
        }
        BridgePieceType::CastleSmallCorridor => {
            let box_ = try_box(builder, orient(-1, 0, 0, 5, 7, 5))?;
            Some(make_piece(
                NetherBridgeKind::CastleSmallCorridor,
                gen_depth,
                box_,
                direction,
            ))
        }
        BridgePieceType::CastleSmallCorridorCrossing => {
            let box_ = try_box(builder, orient(-1, 0, 0, 5, 7, 5))?;
            Some(make_piece(
                NetherBridgeKind::CastleSmallCorridorCrossing,
                gen_depth,
                box_,
                direction,
            ))
        }
        BridgePieceType::CastleSmallCorridorRightTurn => {
            let box_ = try_box(builder, orient(-1, 0, 0, 5, 7, 5))?;
            let is_needing_chest = random.next_i32_bounded(3) == 0;
            Some(make_piece(
                NetherBridgeKind::CastleSmallCorridorRightTurn { is_needing_chest },
                gen_depth,
                box_,
                direction,
            ))
        }
        BridgePieceType::CastleSmallCorridorLeftTurn => {
            let box_ = try_box(builder, orient(-1, 0, 0, 5, 7, 5))?;
            let is_needing_chest = random.next_i32_bounded(3) == 0;
            Some(make_piece(
                NetherBridgeKind::CastleSmallCorridorLeftTurn { is_needing_chest },
                gen_depth,
                box_,
                direction,
            ))
        }
        BridgePieceType::CastleCorridorStairs => {
            let box_ = try_box(builder, orient(-1, -7, 0, 5, 14, 10))?;
            Some(make_piece(
                NetherBridgeKind::CastleCorridorStairs,
                gen_depth,
                box_,
                direction,
            ))
        }
        BridgePieceType::CastleCorridorTBalcony => {
            let box_ = try_box(builder, orient(-3, 0, 0, 9, 7, 9))?;
            Some(make_piece(
                NetherBridgeKind::CastleCorridorTBalcony,
                gen_depth,
                box_,
                direction,
            ))
        }
        BridgePieceType::CastleStalkRoom => {
            let box_ = try_box(builder, orient(-5, -3, 0, 13, 14, 13))?;
            Some(make_piece(
                NetherBridgeKind::CastleStalkRoom,
                gen_depth,
                box_,
                direction,
            ))
        }
    }
}

fn create_bridge_end_filler(
    builder: &StructurePiecesBuilder,
    random: &mut RandomSource,
    foot_x: i32,
    foot_y: i32,
    foot_z: i32,
    direction: Direction,
    gen_depth: i32,
) -> Option<StructurePiece> {
    let box_ = try_box(
        builder,
        BoundingBox::orient_box(foot_x, foot_y, foot_z, -1, -3, 0, 5, 10, 8, direction),
    )?;
    let self_seed = random.next_i32();
    Some(make_piece(
        NetherBridgeKind::BridgeEndFiller { self_seed },
        gen_depth,
        box_,
        direction,
    ))
}

fn update_piece_weight(pieces: &[PieceWeight]) -> i32 {
    let mut has_any_pieces = false;
    let mut total_weight = 0;
    for piece in pieces {
        if piece.max_place_count > 0 && piece.place_count < piece.max_place_count {
            has_any_pieces = true;
        }
        total_weight += piece.weight;
    }
    if has_any_pieces { total_weight } else { -1 }
}

#[expect(clippy::too_many_arguments, reason = "mirrors vanilla generatePiece")]
fn generate_piece(
    state: &mut FortressGen,
    is_castle: bool,
    builder: &StructurePiecesBuilder,
    random: &mut RandomSource,
    foot_x: i32,
    foot_y: i32,
    foot_z: i32,
    direction: Direction,
    depth: i32,
) -> Option<StructurePiece> {
    let total_weight = update_piece_weight(if is_castle {
        &state.available_castle_pieces
    } else {
        &state.available_bridge_pieces
    });
    let do_stuff = total_weight > 0 && depth <= MAX_DEPTH;
    let mut num_attempts = 0;

    while num_attempts < 5 && do_stuff {
        num_attempts += 1;
        let mut weight_selection = random.next_i32_bounded(total_weight);

        let len = if is_castle {
            state.available_castle_pieces.len()
        } else {
            state.available_bridge_pieces.len()
        };
        for index in 0..len {
            let piece = if is_castle {
                state.available_castle_pieces[index]
            } else {
                state.available_bridge_pieces[index]
            };
            weight_selection -= piece.weight;
            if weight_selection >= 0 {
                continue;
            }
            if !piece.do_place() || (state.previous_piece == Some(piece.id) && !piece.allow_in_row)
            {
                break;
            }
            let created = create_bridge_piece(
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
                let current_pieces = if is_castle {
                    &mut state.available_castle_pieces
                } else {
                    &mut state.available_bridge_pieces
                };
                current_pieces[index].place_count += 1;
                state.previous_piece = Some(piece.id);
                if !current_pieces[index].is_valid() {
                    current_pieces.remove(index);
                }
                return Some(created);
            }
        }
    }

    create_bridge_end_filler(builder, random, foot_x, foot_y, foot_z, direction, depth)
}

#[expect(
    clippy::too_many_arguments,
    reason = "mirrors vanilla generateAndAddPiece"
)]
fn generate_and_add_piece(
    state: &mut FortressGen,
    builder: &mut StructurePiecesBuilder,
    random: &mut RandomSource,
    foot_x: i32,
    foot_y: i32,
    foot_z: i32,
    direction: Direction,
    depth: i32,
    is_castle: bool,
) {
    if (foot_x - state.start_bb.min_x()).abs() <= 112
        && (foot_z - state.start_bb.min_z()).abs() <= 112
    {
        let new_piece = generate_piece(
            state,
            is_castle,
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
    } else {
        let _ = create_bridge_end_filler(builder, random, foot_x, foot_y, foot_z, direction, depth);
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "mirrors vanilla generateChildForward"
)]
fn generate_child_forward(
    state: &mut FortressGen,
    builder: &mut StructurePiecesBuilder,
    random: &mut RandomSource,
    box_: BoundingBox,
    orientation: Direction,
    gen_depth: i32,
    x_off: i32,
    y_off: i32,
    is_castle: bool,
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
    generate_and_add_piece(state, builder, random, x, y, z, dir, gen_depth, is_castle);
}

#[expect(
    clippy::too_many_arguments,
    reason = "mirrors vanilla generateChildLeft"
)]
fn generate_child_left(
    state: &mut FortressGen,
    builder: &mut StructurePiecesBuilder,
    random: &mut RandomSource,
    box_: BoundingBox,
    orientation: Direction,
    gen_depth: i32,
    y_off: i32,
    z_off: i32,
    is_castle: bool,
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
    generate_and_add_piece(state, builder, random, x, y, z, dir, gen_depth, is_castle);
}

#[expect(
    clippy::too_many_arguments,
    reason = "mirrors vanilla generateChildRight"
)]
fn generate_child_right(
    state: &mut FortressGen,
    builder: &mut StructurePiecesBuilder,
    random: &mut RandomSource,
    box_: BoundingBox,
    orientation: Direction,
    gen_depth: i32,
    y_off: i32,
    z_off: i32,
    is_castle: bool,
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
    generate_and_add_piece(state, builder, random, x, y, z, dir, gen_depth, is_castle);
}

fn add_children(
    index: usize,
    state: &mut FortressGen,
    builder: &mut StructurePiecesBuilder,
    random: &mut RandomSource,
) {
    let piece = builder.piece_at(index);
    let box_ = piece.base.bounding_box;
    let gen_depth = piece.base.gen_depth;
    let Some(orientation) = piece.base.orientation() else {
        return;
    };
    let PieceKind::NetherBridge(bridge) = &piece.kind else {
        return;
    };

    match &bridge.kind {
        NetherBridgeKind::BridgeCrossing => {
            generate_child_forward(
                state,
                builder,
                random,
                box_,
                orientation,
                gen_depth,
                8,
                3,
                false,
            );
            generate_child_left(
                state,
                builder,
                random,
                box_,
                orientation,
                gen_depth,
                3,
                8,
                false,
            );
            generate_child_right(
                state,
                builder,
                random,
                box_,
                orientation,
                gen_depth,
                3,
                8,
                false,
            );
        }
        NetherBridgeKind::BridgeStraight => {
            generate_child_forward(
                state,
                builder,
                random,
                box_,
                orientation,
                gen_depth,
                1,
                3,
                false,
            );
        }
        NetherBridgeKind::RoomCrossing => {
            generate_child_forward(
                state,
                builder,
                random,
                box_,
                orientation,
                gen_depth,
                2,
                0,
                false,
            );
            generate_child_left(
                state,
                builder,
                random,
                box_,
                orientation,
                gen_depth,
                0,
                2,
                false,
            );
            generate_child_right(
                state,
                builder,
                random,
                box_,
                orientation,
                gen_depth,
                0,
                2,
                false,
            );
        }
        NetherBridgeKind::StairsRoom => {
            generate_child_right(
                state,
                builder,
                random,
                box_,
                orientation,
                gen_depth,
                6,
                2,
                false,
            );
        }
        NetherBridgeKind::CastleEntrance => {
            generate_child_forward(
                state,
                builder,
                random,
                box_,
                orientation,
                gen_depth,
                5,
                3,
                true,
            );
        }
        NetherBridgeKind::CastleCorridorStairs => {
            generate_child_forward(
                state,
                builder,
                random,
                box_,
                orientation,
                gen_depth,
                1,
                0,
                true,
            );
        }
        NetherBridgeKind::CastleSmallCorridor => {
            generate_child_forward(
                state,
                builder,
                random,
                box_,
                orientation,
                gen_depth,
                1,
                0,
                true,
            );
        }
        NetherBridgeKind::CastleSmallCorridorCrossing => {
            generate_child_forward(
                state,
                builder,
                random,
                box_,
                orientation,
                gen_depth,
                1,
                0,
                true,
            );
            generate_child_left(
                state,
                builder,
                random,
                box_,
                orientation,
                gen_depth,
                0,
                1,
                true,
            );
            generate_child_right(
                state,
                builder,
                random,
                box_,
                orientation,
                gen_depth,
                0,
                1,
                true,
            );
        }
        NetherBridgeKind::CastleSmallCorridorLeftTurn { .. } => {
            generate_child_left(
                state,
                builder,
                random,
                box_,
                orientation,
                gen_depth,
                0,
                1,
                true,
            );
        }
        NetherBridgeKind::CastleSmallCorridorRightTurn { .. } => {
            generate_child_right(
                state,
                builder,
                random,
                box_,
                orientation,
                gen_depth,
                0,
                1,
                true,
            );
        }
        NetherBridgeKind::CastleCorridorTBalcony => {
            let z_off = if orientation == Direction::West || orientation == Direction::North {
                5
            } else {
                1
            };
            let left_castle = random.next_i32_bounded(8) > 0;
            generate_child_left(
                state,
                builder,
                random,
                box_,
                orientation,
                gen_depth,
                0,
                z_off,
                left_castle,
            );
            let right_castle = random.next_i32_bounded(8) > 0;
            generate_child_right(
                state,
                builder,
                random,
                box_,
                orientation,
                gen_depth,
                0,
                z_off,
                right_castle,
            );
        }
        NetherBridgeKind::CastleStalkRoom => {
            generate_child_forward(
                state,
                builder,
                random,
                box_,
                orientation,
                gen_depth,
                5,
                3,
                true,
            );
            generate_child_forward(
                state,
                builder,
                random,
                box_,
                orientation,
                gen_depth,
                5,
                11,
                true,
            );
        }
        NetherBridgeKind::MonsterThrone { .. } | NetherBridgeKind::BridgeEndFiller { .. } => {}
    }
}

const fn bricks() -> BlockState {
    Block::NETHER_BRICKS.default_state()
}

const fn fence() -> BlockState {
    Block::NETHER_BRICK_FENCE.default_state()
}

fn we_fence() -> BlockState {
    fence()
        .with(Properties::WEST, true)
        .with(Properties::EAST, true)
}

fn ns_fence() -> BlockState {
    fence()
        .with(Properties::NORTH, true)
        .with(Properties::SOUTH, true)
}

impl NetherBridgePiece {
    pub fn post_process(
        &mut self,
        base: &mut PieceBase,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        chunk_bb: BoundingBox,
    ) {
        let piece = &*base;
        match &mut self.kind {
            NetherBridgeKind::BridgeCrossing => bridge_crossing(piece, region, chunk_bb),
            NetherBridgeKind::BridgeStraight => bridge_straight(piece, region, chunk_bb),
            NetherBridgeKind::BridgeEndFiller { self_seed } => {
                bridge_end_filler(piece, region, chunk_bb, *self_seed);
            }
            NetherBridgeKind::RoomCrossing => room_crossing(piece, region, chunk_bb),
            NetherBridgeKind::StairsRoom => stairs_room(piece, region, chunk_bb),
            NetherBridgeKind::MonsterThrone { has_placed_spawner } => {
                monster_throne(piece, region, chunk_bb, has_placed_spawner);
            }
            NetherBridgeKind::CastleEntrance => castle_entrance(piece, region, chunk_bb),
            NetherBridgeKind::CastleSmallCorridor => {
                castle_small_corridor(piece, region, chunk_bb);
            }
            NetherBridgeKind::CastleSmallCorridorCrossing => {
                castle_small_corridor_crossing(piece, region, chunk_bb);
            }
            NetherBridgeKind::CastleSmallCorridorRightTurn { is_needing_chest } => {
                castle_small_corridor_right_turn(piece, region, random, chunk_bb, is_needing_chest);
            }
            NetherBridgeKind::CastleSmallCorridorLeftTurn { is_needing_chest } => {
                castle_small_corridor_left_turn(piece, region, random, chunk_bb, is_needing_chest);
            }
            NetherBridgeKind::CastleCorridorStairs => {
                castle_corridor_stairs(piece, region, chunk_bb);
            }
            NetherBridgeKind::CastleCorridorTBalcony => {
                castle_corridor_t_balcony(piece, region, chunk_bb);
            }
            NetherBridgeKind::CastleStalkRoom => castle_stalk_room(piece, region, chunk_bb),
        }
    }
}

fn bridge_crossing(base: &PieceBase, region: &WorldGenRegion<'_>, chunk_bb: BoundingBox) {
    let bricks = bricks();
    let air = air();
    let gbox = |x0, y0, z0, x1, y1, z1, state| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, false,
        );
    };

    gbox(7, 3, 0, 11, 4, 18, bricks);
    gbox(0, 3, 7, 18, 4, 11, bricks);
    gbox(8, 5, 0, 10, 7, 18, air);
    gbox(0, 5, 8, 18, 7, 10, air);
    gbox(7, 5, 0, 7, 5, 7, bricks);
    gbox(7, 5, 11, 7, 5, 18, bricks);
    gbox(11, 5, 0, 11, 5, 7, bricks);
    gbox(11, 5, 11, 11, 5, 18, bricks);
    gbox(0, 5, 7, 7, 5, 7, bricks);
    gbox(11, 5, 7, 18, 5, 7, bricks);
    gbox(0, 5, 11, 7, 5, 11, bricks);
    gbox(11, 5, 11, 18, 5, 11, bricks);
    gbox(7, 2, 0, 11, 2, 5, bricks);
    gbox(7, 2, 13, 11, 2, 18, bricks);
    gbox(7, 0, 0, 11, 1, 3, bricks);
    gbox(7, 0, 15, 11, 1, 18, bricks);

    for x in 7..=11 {
        for z in 0..=2 {
            base.fill_column_down(region, bricks, x, -1, z, chunk_bb);
            base.fill_column_down(region, bricks, x, -1, 18 - z, chunk_bb);
        }
    }

    gbox(0, 2, 7, 5, 2, 11, bricks);
    gbox(13, 2, 7, 18, 2, 11, bricks);
    gbox(0, 0, 7, 3, 1, 11, bricks);
    gbox(15, 0, 7, 18, 1, 11, bricks);

    for x in 0..=2 {
        for z in 7..=11 {
            base.fill_column_down(region, bricks, x, -1, z, chunk_bb);
            base.fill_column_down(region, bricks, 18 - x, -1, z, chunk_bb);
        }
    }
}

fn bridge_straight(base: &PieceBase, region: &WorldGenRegion<'_>, chunk_bb: BoundingBox) {
    let bricks = bricks();
    let air = air();
    let gbox = |x0, y0, z0, x1, y1, z1, state| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, false,
        );
    };

    gbox(0, 3, 0, 4, 4, 18, bricks);
    gbox(1, 5, 0, 3, 7, 18, air);
    gbox(0, 5, 0, 0, 5, 18, bricks);
    gbox(4, 5, 0, 4, 5, 18, bricks);
    gbox(0, 2, 0, 4, 2, 5, bricks);
    gbox(0, 2, 13, 4, 2, 18, bricks);
    gbox(0, 0, 0, 4, 1, 3, bricks);
    gbox(0, 0, 15, 4, 1, 18, bricks);

    for x in 0..=4 {
        for z in 0..=2 {
            base.fill_column_down(region, bricks, x, -1, z, chunk_bb);
            base.fill_column_down(region, bricks, x, -1, 18 - z, chunk_bb);
        }
    }

    let nse_fence = ns_fence().with(Properties::EAST, true);
    let nsw_fence = ns_fence().with(Properties::WEST, true);
    gbox(0, 1, 1, 0, 4, 1, nse_fence);
    gbox(0, 3, 4, 0, 4, 4, nse_fence);
    gbox(0, 3, 14, 0, 4, 14, nse_fence);
    gbox(0, 1, 17, 0, 4, 17, nse_fence);
    gbox(4, 1, 1, 4, 4, 1, nsw_fence);
    gbox(4, 3, 4, 4, 4, 4, nsw_fence);
    gbox(4, 3, 14, 4, 4, 14, nsw_fence);
    gbox(4, 1, 17, 4, 4, 17, nsw_fence);
}

fn bridge_end_filler(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    chunk_bb: BoundingBox,
    self_seed: i32,
) {
    let bricks = bricks();
    let mut self_random = RandomSource::Legacy(LegacyRandomSource::from_seed(self_seed as u64));
    let gbox = |x0, y0, z0, x1, y1, z1, state| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, false,
        );
    };

    for x in 0..=4 {
        for y in 3..=4 {
            let z = self_random.next_i32_bounded(8);
            gbox(x, y, 0, x, y, z, bricks);
        }
    }

    let z = self_random.next_i32_bounded(8);
    gbox(0, 5, 0, 0, 5, z, bricks);
    let z = self_random.next_i32_bounded(8);
    gbox(4, 5, 0, 4, 5, z, bricks);

    for x in 0..=4 {
        let zx = self_random.next_i32_bounded(5);
        gbox(x, 2, 0, x, 2, zx, bricks);
    }

    for x in 0..=4 {
        for y in 0..=1 {
            let zx = self_random.next_i32_bounded(3);
            gbox(x, y, 0, x, y, zx, bricks);
        }
    }
}

fn room_crossing(base: &PieceBase, region: &WorldGenRegion<'_>, chunk_bb: BoundingBox) {
    let bricks = bricks();
    let air = air();
    let gbox = |x0, y0, z0, x1, y1, z1, state| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, false,
        );
    };

    gbox(0, 0, 0, 6, 1, 6, bricks);
    gbox(0, 2, 0, 6, 7, 6, air);
    gbox(0, 2, 0, 1, 6, 0, bricks);
    gbox(0, 2, 6, 1, 6, 6, bricks);
    gbox(5, 2, 0, 6, 6, 0, bricks);
    gbox(5, 2, 6, 6, 6, 6, bricks);
    gbox(0, 2, 0, 0, 6, 1, bricks);
    gbox(0, 2, 5, 0, 6, 6, bricks);
    gbox(6, 2, 0, 6, 6, 1, bricks);
    gbox(6, 2, 5, 6, 6, 6, bricks);
    let we = we_fence();
    let ns = ns_fence();
    gbox(2, 6, 0, 4, 6, 0, bricks);
    gbox(2, 5, 0, 4, 5, 0, we);
    gbox(2, 6, 6, 4, 6, 6, bricks);
    gbox(2, 5, 6, 4, 5, 6, we);
    gbox(0, 6, 2, 0, 6, 4, bricks);
    gbox(0, 5, 2, 0, 5, 4, ns);
    gbox(6, 6, 2, 6, 6, 4, bricks);
    gbox(6, 5, 2, 6, 5, 4, ns);

    for x in 0..=6 {
        for z in 0..=6 {
            base.fill_column_down(region, bricks, x, -1, z, chunk_bb);
        }
    }
}

fn stairs_room(base: &PieceBase, region: &WorldGenRegion<'_>, chunk_bb: BoundingBox) {
    let bricks = bricks();
    let air = air();
    let gbox = |x0, y0, z0, x1, y1, z1, state| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, false,
        );
    };

    gbox(0, 0, 0, 6, 1, 6, bricks);
    gbox(0, 2, 0, 6, 10, 6, air);
    gbox(0, 2, 0, 1, 8, 0, bricks);
    gbox(5, 2, 0, 6, 8, 0, bricks);
    gbox(0, 2, 1, 0, 8, 6, bricks);
    gbox(6, 2, 1, 6, 8, 6, bricks);
    gbox(1, 2, 6, 5, 8, 6, bricks);
    let we = we_fence();
    let ns = ns_fence();
    gbox(0, 3, 2, 0, 5, 4, ns);
    gbox(6, 3, 2, 6, 5, 2, ns);
    gbox(6, 3, 4, 6, 5, 4, ns);
    base.place_block(region, bricks, 5, 2, 5, chunk_bb);
    gbox(4, 2, 5, 4, 3, 5, bricks);
    gbox(3, 2, 5, 3, 4, 5, bricks);
    gbox(2, 2, 5, 2, 5, 5, bricks);
    gbox(1, 2, 5, 1, 6, 5, bricks);
    gbox(1, 7, 1, 5, 7, 4, bricks);
    gbox(6, 8, 2, 6, 8, 4, air);
    gbox(2, 6, 0, 4, 8, 0, bricks);
    gbox(2, 5, 0, 4, 5, 0, we);

    for x in 0..=6 {
        for z in 0..=6 {
            base.fill_column_down(region, bricks, x, -1, z, chunk_bb);
        }
    }
}

fn monster_throne(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    chunk_bb: BoundingBox,
    has_placed_spawner: &mut bool,
) {
    let bricks = bricks();
    let air = air();
    let gbox = |x0, y0, z0, x1, y1, z1, state| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, false,
        );
    };

    gbox(0, 2, 0, 6, 7, 7, air);
    gbox(1, 0, 0, 5, 1, 7, bricks);
    gbox(1, 2, 1, 5, 2, 7, bricks);
    gbox(1, 3, 2, 5, 3, 7, bricks);
    gbox(1, 4, 3, 5, 4, 7, bricks);
    gbox(1, 2, 0, 1, 4, 2, bricks);
    gbox(5, 2, 0, 5, 4, 2, bricks);
    gbox(1, 5, 2, 1, 5, 3, bricks);
    gbox(5, 5, 2, 5, 5, 3, bricks);
    gbox(0, 5, 3, 0, 5, 8, bricks);
    gbox(6, 5, 3, 6, 5, 8, bricks);
    gbox(1, 5, 8, 5, 5, 8, bricks);

    let we = we_fence();
    let ns = ns_fence();
    base.place_block(
        region,
        fence().with(Properties::WEST, true),
        1,
        6,
        3,
        chunk_bb,
    );
    base.place_block(
        region,
        fence().with(Properties::EAST, true),
        5,
        6,
        3,
        chunk_bb,
    );
    base.place_block(
        region,
        fence()
            .with(Properties::EAST, true)
            .with(Properties::NORTH, true),
        0,
        6,
        3,
        chunk_bb,
    );
    base.place_block(
        region,
        fence()
            .with(Properties::WEST, true)
            .with(Properties::NORTH, true),
        6,
        6,
        3,
        chunk_bb,
    );
    gbox(0, 6, 4, 0, 6, 7, ns);
    gbox(6, 6, 4, 6, 6, 7, ns);
    base.place_block(
        region,
        fence()
            .with(Properties::EAST, true)
            .with(Properties::SOUTH, true),
        0,
        6,
        8,
        chunk_bb,
    );
    base.place_block(
        region,
        fence()
            .with(Properties::WEST, true)
            .with(Properties::SOUTH, true),
        6,
        6,
        8,
        chunk_bb,
    );
    gbox(1, 6, 8, 5, 6, 8, we);
    base.place_block(
        region,
        fence().with(Properties::EAST, true),
        1,
        7,
        8,
        chunk_bb,
    );
    gbox(2, 7, 8, 4, 7, 8, we);
    base.place_block(
        region,
        fence().with(Properties::WEST, true),
        5,
        7,
        8,
        chunk_bb,
    );
    base.place_block(
        region,
        fence().with(Properties::EAST, true),
        2,
        8,
        8,
        chunk_bb,
    );
    base.place_block(region, we, 3, 8, 8, chunk_bb);
    base.place_block(
        region,
        fence().with(Properties::WEST, true),
        4,
        8,
        8,
        chunk_bb,
    );

    if !*has_placed_spawner {
        let pos = base.world_pos(3, 5, 5);
        if chunk_bb.is_inside(pos) {
            *has_placed_spawner = true;
            region.set_block(pos, Block::SPAWNER.default_state());
        }
    }

    for x in 0..=6 {
        for z in 0..=6 {
            base.fill_column_down(region, bricks, x, -1, z, chunk_bb);
        }
    }
}

fn castle_entrance(base: &PieceBase, region: &WorldGenRegion<'_>, chunk_bb: BoundingBox) {
    let bricks = bricks();
    let air = air();
    let gbox = |x0, y0, z0, x1, y1, z1, state| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, false,
        );
    };

    gbox(0, 3, 0, 12, 4, 12, bricks);
    gbox(0, 5, 0, 12, 13, 12, air);
    gbox(0, 5, 0, 1, 12, 12, bricks);
    gbox(11, 5, 0, 12, 12, 12, bricks);
    gbox(2, 5, 11, 4, 12, 12, bricks);
    gbox(8, 5, 11, 10, 12, 12, bricks);
    gbox(5, 9, 11, 7, 12, 12, bricks);
    gbox(2, 5, 0, 4, 12, 1, bricks);
    gbox(8, 5, 0, 10, 12, 1, bricks);
    gbox(5, 9, 0, 7, 12, 1, bricks);
    gbox(2, 11, 2, 10, 12, 10, bricks);
    gbox(5, 8, 0, 7, 8, 0, fence());

    let we = we_fence();
    let ns = ns_fence();
    let mut i = 1;
    while i <= 11 {
        gbox(i, 10, 0, i, 11, 0, we);
        gbox(i, 10, 12, i, 11, 12, we);
        gbox(0, 10, i, 0, 11, i, ns);
        gbox(12, 10, i, 12, 11, i, ns);
        base.place_block(region, bricks, i, 13, 0, chunk_bb);
        base.place_block(region, bricks, i, 13, 12, chunk_bb);
        base.place_block(region, bricks, 0, 13, i, chunk_bb);
        base.place_block(region, bricks, 12, 13, i, chunk_bb);
        if i != 11 {
            base.place_block(region, we, i + 1, 13, 0, chunk_bb);
            base.place_block(region, we, i + 1, 13, 12, chunk_bb);
            base.place_block(region, ns, 0, 13, i + 1, chunk_bb);
            base.place_block(region, ns, 12, 13, i + 1, chunk_bb);
        }
        i += 2;
    }

    let corner = |north: bool, east: bool| {
        fence()
            .with(
                if north {
                    Properties::NORTH
                } else {
                    Properties::SOUTH
                },
                true,
            )
            .with(
                if east {
                    Properties::EAST
                } else {
                    Properties::WEST
                },
                true,
            )
    };
    base.place_block(region, corner(true, true), 0, 13, 0, chunk_bb);
    base.place_block(region, corner(false, true), 0, 13, 12, chunk_bb);
    base.place_block(region, corner(false, false), 12, 13, 12, chunk_bb);
    base.place_block(region, corner(true, false), 12, 13, 0, chunk_bb);

    let mut z = 3;
    while z <= 9 {
        let west = ns.with(Properties::WEST, true);
        let east = ns.with(Properties::EAST, true);
        gbox(1, 7, z, 1, 8, z, west);
        gbox(11, 7, z, 11, 8, z, east);
        z += 2;
    }

    gbox(4, 2, 0, 8, 2, 12, bricks);
    gbox(0, 2, 4, 12, 2, 8, bricks);
    gbox(4, 0, 0, 8, 1, 3, bricks);
    gbox(4, 0, 9, 8, 1, 12, bricks);
    gbox(0, 0, 4, 3, 1, 8, bricks);
    gbox(9, 0, 4, 12, 1, 8, bricks);

    for x in 4..=8 {
        for z in 0..=2 {
            base.fill_column_down(region, bricks, x, -1, z, chunk_bb);
            base.fill_column_down(region, bricks, x, -1, 12 - z, chunk_bb);
        }
    }

    for x in 0..=2 {
        for z in 4..=8 {
            base.fill_column_down(region, bricks, x, -1, z, chunk_bb);
            base.fill_column_down(region, bricks, 12 - x, -1, z, chunk_bb);
        }
    }

    gbox(5, 5, 5, 7, 5, 7, bricks);
    gbox(6, 1, 6, 6, 4, 6, air);
    base.place_block(region, bricks, 6, 0, 6, chunk_bb);
    base.place_block(region, Block::LAVA.default_state(), 6, 5, 6, chunk_bb);
}

fn castle_small_corridor(base: &PieceBase, region: &WorldGenRegion<'_>, chunk_bb: BoundingBox) {
    let bricks = bricks();
    let air = air();
    let gbox = |x0, y0, z0, x1, y1, z1, state| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, false,
        );
    };

    gbox(0, 0, 0, 4, 1, 4, bricks);
    gbox(0, 2, 0, 4, 5, 4, air);
    let ns = ns_fence();
    gbox(0, 2, 0, 0, 5, 4, bricks);
    gbox(4, 2, 0, 4, 5, 4, bricks);
    gbox(0, 3, 1, 0, 4, 1, ns);
    gbox(0, 3, 3, 0, 4, 3, ns);
    gbox(4, 3, 1, 4, 4, 1, ns);
    gbox(4, 3, 3, 4, 4, 3, ns);
    gbox(0, 6, 0, 4, 6, 4, bricks);

    for x in 0..=4 {
        for z in 0..=4 {
            base.fill_column_down(region, bricks, x, -1, z, chunk_bb);
        }
    }
}

fn castle_small_corridor_crossing(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    chunk_bb: BoundingBox,
) {
    let bricks = bricks();
    let air = air();
    let gbox = |x0, y0, z0, x1, y1, z1, state| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, false,
        );
    };

    gbox(0, 0, 0, 4, 1, 4, bricks);
    gbox(0, 2, 0, 4, 5, 4, air);
    gbox(0, 2, 0, 0, 5, 0, bricks);
    gbox(4, 2, 0, 4, 5, 0, bricks);
    gbox(0, 2, 4, 0, 5, 4, bricks);
    gbox(4, 2, 4, 4, 5, 4, bricks);
    gbox(0, 6, 0, 4, 6, 4, bricks);

    for x in 0..=4 {
        for z in 0..=4 {
            base.fill_column_down(region, bricks, x, -1, z, chunk_bb);
        }
    }
}

fn castle_small_corridor_left_turn(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    chunk_bb: BoundingBox,
    is_needing_chest: &mut bool,
) {
    let bricks = bricks();
    let air = air();
    let gbox = |x0, y0, z0, x1, y1, z1, state| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, false,
        );
    };

    gbox(0, 0, 0, 4, 1, 4, bricks);
    gbox(0, 2, 0, 4, 5, 4, air);
    let we = we_fence();
    let ns = ns_fence();
    gbox(4, 2, 0, 4, 5, 4, bricks);
    gbox(4, 3, 1, 4, 4, 1, ns);
    gbox(4, 3, 3, 4, 4, 3, ns);
    gbox(0, 2, 0, 0, 5, 0, bricks);
    gbox(0, 2, 4, 3, 5, 4, bricks);
    gbox(1, 3, 4, 1, 4, 4, we);
    gbox(3, 3, 4, 3, 4, 4, we);
    if *is_needing_chest && chunk_bb.is_inside(base.world_pos(3, 2, 3)) {
        *is_needing_chest = false;
        base.create_chest(region, chunk_bb, random, 3, 2, 3);
    }

    gbox(0, 6, 0, 4, 6, 4, bricks);

    for x in 0..=4 {
        for z in 0..=4 {
            base.fill_column_down(region, bricks, x, -1, z, chunk_bb);
        }
    }
}

fn castle_small_corridor_right_turn(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    chunk_bb: BoundingBox,
    is_needing_chest: &mut bool,
) {
    let bricks = bricks();
    let air = air();
    let gbox = |x0, y0, z0, x1, y1, z1, state| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, false,
        );
    };

    gbox(0, 0, 0, 4, 1, 4, bricks);
    gbox(0, 2, 0, 4, 5, 4, air);
    let we = we_fence();
    let ns = ns_fence();
    gbox(0, 2, 0, 0, 5, 4, bricks);
    gbox(0, 3, 1, 0, 4, 1, ns);
    gbox(0, 3, 3, 0, 4, 3, ns);
    gbox(4, 2, 0, 4, 5, 0, bricks);
    gbox(1, 2, 4, 4, 5, 4, bricks);
    gbox(1, 3, 4, 1, 4, 4, we);
    gbox(3, 3, 4, 3, 4, 4, we);
    if *is_needing_chest && chunk_bb.is_inside(base.world_pos(1, 2, 3)) {
        *is_needing_chest = false;
        base.create_chest(region, chunk_bb, random, 1, 2, 3);
    }

    gbox(0, 6, 0, 4, 6, 4, bricks);

    for x in 0..=4 {
        for z in 0..=4 {
            base.fill_column_down(region, bricks, x, -1, z, chunk_bb);
        }
    }
}

fn castle_corridor_stairs(base: &PieceBase, region: &WorldGenRegion<'_>, chunk_bb: BoundingBox) {
    let bricks = bricks();
    let air = air();
    let stairs = Block::NETHER_BRICK_STAIRS
        .default_state()
        .with(Properties::HORIZONTAL_FACING, Direction::South);
    let ns = ns_fence();
    let gbox = |x0, y0, z0, x1, y1, z1, state| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, false,
        );
    };

    for step in 0..=9 {
        let floor = 1.max(7 - step);
        let roof = (floor + 5).max(14 - step).min(13);
        let z = step;
        gbox(0, 0, z, 4, floor, z, bricks);
        gbox(1, floor + 1, z, 3, roof - 1, z, air);
        if step <= 6 {
            base.place_block(region, stairs, 1, floor + 1, z, chunk_bb);
            base.place_block(region, stairs, 2, floor + 1, z, chunk_bb);
            base.place_block(region, stairs, 3, floor + 1, z, chunk_bb);
        }
        gbox(0, roof, z, 4, roof, z, bricks);
        gbox(0, floor + 1, z, 0, roof - 1, z, bricks);
        gbox(4, floor + 1, z, 4, roof - 1, z, bricks);
        if (step & 1) == 0 {
            gbox(0, floor + 2, z, 0, floor + 3, z, ns);
            gbox(4, floor + 2, z, 4, floor + 3, z, ns);
        }
        for x in 0..=4 {
            base.fill_column_down(region, bricks, x, -1, z, chunk_bb);
        }
    }
}

fn castle_corridor_t_balcony(base: &PieceBase, region: &WorldGenRegion<'_>, chunk_bb: BoundingBox) {
    let bricks = bricks();
    let air = air();
    let we = we_fence();
    let ns = ns_fence();
    let gbox = |x0, y0, z0, x1, y1, z1, state| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, false,
        );
    };

    gbox(0, 0, 0, 8, 1, 8, bricks);
    gbox(0, 2, 0, 8, 5, 8, air);
    gbox(0, 6, 0, 8, 6, 5, bricks);
    gbox(0, 2, 0, 2, 5, 0, bricks);
    gbox(6, 2, 0, 8, 5, 0, bricks);
    gbox(1, 3, 0, 1, 4, 0, we);
    gbox(7, 3, 0, 7, 4, 0, we);
    gbox(0, 2, 4, 8, 2, 8, bricks);
    gbox(1, 1, 4, 2, 2, 4, air);
    gbox(6, 1, 4, 7, 2, 4, air);
    gbox(1, 3, 8, 7, 3, 8, we);
    base.place_block(
        region,
        fence()
            .with(Properties::EAST, true)
            .with(Properties::SOUTH, true),
        0,
        3,
        8,
        chunk_bb,
    );
    base.place_block(
        region,
        fence()
            .with(Properties::WEST, true)
            .with(Properties::SOUTH, true),
        8,
        3,
        8,
        chunk_bb,
    );
    gbox(0, 3, 6, 0, 3, 7, ns);
    gbox(8, 3, 6, 8, 3, 7, ns);
    gbox(0, 3, 4, 0, 5, 5, bricks);
    gbox(8, 3, 4, 8, 5, 5, bricks);
    gbox(1, 3, 5, 2, 5, 5, bricks);
    gbox(6, 3, 5, 7, 5, 5, bricks);
    gbox(1, 4, 5, 1, 5, 5, we);
    gbox(7, 4, 5, 7, 5, 5, we);

    for z in 0..=5 {
        for x in 0..=8 {
            base.fill_column_down(region, bricks, x, -1, z, chunk_bb);
        }
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "mirrors vanilla CastleStalkRoom::postProcess"
)]
fn castle_stalk_room(base: &PieceBase, region: &WorldGenRegion<'_>, chunk_bb: BoundingBox) {
    let bricks = bricks();
    let air = air();
    let gbox = |x0, y0, z0, x1, y1, z1, state| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, false,
        );
    };

    gbox(0, 3, 0, 12, 4, 12, bricks);
    gbox(0, 5, 0, 12, 13, 12, air);
    gbox(0, 5, 0, 1, 12, 12, bricks);
    gbox(11, 5, 0, 12, 12, 12, bricks);
    gbox(2, 5, 11, 4, 12, 12, bricks);
    gbox(8, 5, 11, 10, 12, 12, bricks);
    gbox(5, 9, 11, 7, 12, 12, bricks);
    gbox(2, 5, 0, 4, 12, 1, bricks);
    gbox(8, 5, 0, 10, 12, 1, bricks);
    gbox(5, 9, 0, 7, 12, 1, bricks);
    gbox(2, 11, 2, 10, 12, 10, bricks);

    let we = we_fence();
    let ns = ns_fence();
    let nsw = ns.with(Properties::WEST, true);
    let nse = ns.with(Properties::EAST, true);

    let mut i = 1;
    while i <= 11 {
        gbox(i, 10, 0, i, 11, 0, we);
        gbox(i, 10, 12, i, 11, 12, we);
        gbox(0, 10, i, 0, 11, i, ns);
        gbox(12, 10, i, 12, 11, i, ns);
        base.place_block(region, bricks, i, 13, 0, chunk_bb);
        base.place_block(region, bricks, i, 13, 12, chunk_bb);
        base.place_block(region, bricks, 0, 13, i, chunk_bb);
        base.place_block(region, bricks, 12, 13, i, chunk_bb);
        if i != 11 {
            base.place_block(region, we, i + 1, 13, 0, chunk_bb);
            base.place_block(region, we, i + 1, 13, 12, chunk_bb);
            base.place_block(region, ns, 0, 13, i + 1, chunk_bb);
            base.place_block(region, ns, 12, 13, i + 1, chunk_bb);
        }
        i += 2;
    }

    let corner = |north: bool, east: bool| {
        fence()
            .with(
                if north {
                    Properties::NORTH
                } else {
                    Properties::SOUTH
                },
                true,
            )
            .with(
                if east {
                    Properties::EAST
                } else {
                    Properties::WEST
                },
                true,
            )
    };
    base.place_block(region, corner(true, true), 0, 13, 0, chunk_bb);
    base.place_block(region, corner(false, true), 0, 13, 12, chunk_bb);
    base.place_block(region, corner(false, false), 12, 13, 12, chunk_bb);
    base.place_block(region, corner(true, false), 12, 13, 0, chunk_bb);

    let mut z = 3;
    while z <= 9 {
        gbox(1, 7, z, 1, 8, z, nsw);
        gbox(11, 7, z, 11, 8, z, nse);
        z += 2;
    }

    let stairs = Block::NETHER_BRICK_STAIRS
        .default_state()
        .with(Properties::HORIZONTAL_FACING, Direction::North);

    for i in 0..=6 {
        let z = i + 4;
        for x in 5..=7 {
            base.place_block(region, stairs, x, 5 + i, z, chunk_bb);
        }
        if (5..=8).contains(&z) {
            gbox(5, 5, z, 7, i + 4, z, bricks);
        } else if (9..=10).contains(&z) {
            gbox(5, 8, z, 7, i + 4, z, bricks);
        }
        if i >= 1 {
            gbox(5, 6 + i, z, 7, 9 + i, z, air);
        }
    }

    for x in 5..=7 {
        base.place_block(region, stairs, x, 12, 11, chunk_bb);
    }

    gbox(5, 6, 7, 5, 7, 7, nse);
    gbox(7, 6, 7, 7, 7, 7, nsw);
    gbox(5, 13, 12, 7, 13, 12, air);
    gbox(2, 5, 2, 3, 5, 3, bricks);
    gbox(2, 5, 9, 3, 5, 10, bricks);
    gbox(2, 5, 4, 2, 5, 8, bricks);
    gbox(9, 5, 2, 10, 5, 3, bricks);
    gbox(9, 5, 9, 10, 5, 10, bricks);
    gbox(10, 5, 4, 10, 5, 8, bricks);

    let east_stairs = stairs.with(Properties::HORIZONTAL_FACING, Direction::East);
    let west_stairs = stairs.with(Properties::HORIZONTAL_FACING, Direction::West);
    base.place_block(region, west_stairs, 4, 5, 2, chunk_bb);
    base.place_block(region, west_stairs, 4, 5, 3, chunk_bb);
    base.place_block(region, west_stairs, 4, 5, 9, chunk_bb);
    base.place_block(region, west_stairs, 4, 5, 10, chunk_bb);
    base.place_block(region, east_stairs, 8, 5, 2, chunk_bb);
    base.place_block(region, east_stairs, 8, 5, 3, chunk_bb);
    base.place_block(region, east_stairs, 8, 5, 9, chunk_bb);
    base.place_block(region, east_stairs, 8, 5, 10, chunk_bb);

    let soul_sand = Block::SOUL_SAND.default_state();
    let nether_wart = Block::NETHER_WART.default_state();
    gbox(3, 4, 4, 4, 4, 8, soul_sand);
    gbox(8, 4, 4, 9, 4, 8, soul_sand);
    gbox(3, 5, 4, 4, 5, 8, nether_wart);
    gbox(8, 5, 4, 9, 5, 8, nether_wart);
    gbox(4, 2, 0, 8, 2, 12, bricks);
    gbox(0, 2, 4, 12, 2, 8, bricks);
    gbox(4, 0, 0, 8, 1, 3, bricks);
    gbox(4, 0, 9, 8, 1, 12, bricks);
    gbox(0, 0, 4, 3, 1, 8, bricks);
    gbox(9, 0, 4, 12, 1, 8, bricks);

    for x in 4..=8 {
        for z in 0..=2 {
            base.fill_column_down(region, bricks, x, -1, z, chunk_bb);
            base.fill_column_down(region, bricks, x, -1, 12 - z, chunk_bb);
        }
    }

    for x in 0..=2 {
        for z in 4..=8 {
            base.fill_column_down(region, bricks, x, -1, z, chunk_bb);
            base.fill_column_down(region, bricks, 12 - x, -1, z, chunk_bb);
        }
    }
}
