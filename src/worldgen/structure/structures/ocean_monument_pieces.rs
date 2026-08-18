use crate::registry::blocks::vanilla_blocks;
use crate::util::direction::Direction;
use crate::util::geometry::BoundingBox;
use crate::util::ident::BlockState;
use crate::util::rng::RandomSource;
use crate::util::shuffle;
use crate::worldgen::region::WorldGenRegion;
use crate::worldgen::structure::structure_piece::{PieceBase, PieceKind, StructurePiece, air};

const DOWN: usize = Direction::Down.get_3d_data_value();
const UP: usize = Direction::Up.get_3d_data_value();
const NORTH: usize = Direction::North.get_3d_data_value();
const SOUTH: usize = Direction::South.get_3d_data_value();
const WEST: usize = Direction::West.get_3d_data_value();
const EAST: usize = Direction::East.get_3d_data_value();

const GRIDROOM_WIDTH: i32 = 8;
const GRIDROOM_DEPTH: i32 = 8;
const GRIDROOM_HEIGHT: i32 = 4;
const GRID_FLOOR_COUNT: i32 = 25;
const GRID_SIZE: usize = 75;
const GRIDROOM_SOURCE_INDEX: usize = get_room_index(2, 0, 0);
const GRIDROOM_TOP_CONNECT_INDEX: usize = get_room_index(2, 2, 0);
const GRIDROOM_LEFTWING_CONNECT_INDEX: usize = get_room_index(0, 1, 0);
const GRIDROOM_RIGHTWING_CONNECT_INDEX: usize = get_room_index(4, 1, 0);
const LEFTWING_INDEX: i32 = 1001;
const RIGHTWING_INDEX: i32 = 1002;
const PENTHOUSE_INDEX: i32 = 1003;

pub const WIDTH: i32 = 58;
pub const HEIGHT: i32 = 22;
pub const DEPTH: i32 = 58;
pub const BIOME_RANGE_CHECK: i32 = 29;

const fn get_room_index(room_x: i32, room_y: i32, room_z: i32) -> usize {
    (room_y * 25 + room_z * 5 + room_x) as usize
}

fn base_gray() -> BlockState {
    vanilla_blocks::PRISMARINE.default_state()
}

fn base_light() -> BlockState {
    vanilla_blocks::PRISMARINE_BRICKS.default_state()
}

fn base_black() -> BlockState {
    vanilla_blocks::DARK_PRISMARINE.default_state()
}

fn dot_deco_data() -> BlockState {
    base_light()
}

fn lamp_block() -> BlockState {
    vanilla_blocks::SEA_LANTERN.default_state()
}

fn fill_block() -> BlockState {
    vanilla_blocks::WATER.default_state()
}

fn is_fill_keep(state: BlockState) -> bool {
    let block = state.get_block();
    block == &vanilla_blocks::ICE
        || block == &vanilla_blocks::PACKED_ICE
        || block == &vanilla_blocks::BLUE_ICE
        || block == &vanilla_blocks::WATER
}

#[derive(Debug)]
struct RoomDefinition {
    index: i32,
    connections: [Option<usize>; 6],
    has_opening: [bool; 6],
    claimed: bool,
    is_source: bool,
    scan_index: i32,
}

impl RoomDefinition {
    const fn new(index: i32) -> Self {
        Self {
            index,
            connections: [None; 6],
            has_opening: [false; 6],
            claimed: false,
            is_source: false,
            scan_index: 0,
        }
    }

    const fn is_special(&self) -> bool {
        self.index >= GRID_SIZE as i32
    }

    fn count_openings(&self) -> i32 {
        self.has_opening.iter().filter(|open| **open).count() as i32
    }
}

const fn connection(rooms: &[RoomDefinition], room: usize, direction: usize) -> usize {
    rooms[room].connections[direction].expect("monument room should have a connection")
}

const fn connected(rooms: &[RoomDefinition], room: usize, direction: usize) -> &RoomDefinition {
    &rooms[connection(rooms, room, direction)]
}

const fn grid_room(room_grid: &[Option<usize>; GRID_SIZE], index: usize) -> usize {
    room_grid[index].expect("monument grid room should exist")
}

const fn set_connection(
    rooms: &mut [RoomDefinition],
    room: usize,
    direction: Direction,
    other: usize,
) {
    rooms[room].connections[direction.get_3d_data_value()] = Some(other);
    rooms[other].connections[direction.opposite().get_3d_data_value()] = Some(room);
}

fn update_openings(rooms: &mut [RoomDefinition], room: usize) {
    for i in 0..6 {
        rooms[room].has_opening[i] = rooms[room].connections[i].is_some();
    }
}

fn find_source(rooms: &mut [RoomDefinition], room: usize, scan_index: i32) -> bool {
    if rooms[room].is_source {
        return true;
    }

    rooms[room].scan_index = scan_index;

    for i in 0..6 {
        let Some(connection) = rooms[room].connections[i] else {
            continue;
        };
        if rooms[room].has_opening[i]
            && rooms[connection].scan_index != scan_index
            && find_source(rooms, connection, scan_index)
        {
            return true;
        }
    }

    false
}

#[derive(Debug, Clone, Copy)]
enum Fitter {
    DoubleXY,
    DoubleYZ,
    DoubleZ,
    DoubleX,
    DoubleY,
    SimpleTop,
    Simple,
}

const FITTERS: [Fitter; 7] = [
    Fitter::DoubleXY,
    Fitter::DoubleYZ,
    Fitter::DoubleZ,
    Fitter::DoubleX,
    Fitter::DoubleY,
    Fitter::SimpleTop,
    Fitter::Simple,
];

impl Fitter {
    const fn fits(self, rooms: &[RoomDefinition], room: usize) -> bool {
        let definition = &rooms[room];
        match self {
            Self::DoubleXY => {
                if definition.has_opening[EAST]
                    && !connected(rooms, room, EAST).claimed
                    && definition.has_opening[UP]
                    && !connected(rooms, room, UP).claimed
                {
                    let east = connection(rooms, room, EAST);
                    rooms[east].has_opening[UP] && !connected(rooms, east, UP).claimed
                } else {
                    false
                }
            }
            Self::DoubleYZ => {
                if definition.has_opening[NORTH]
                    && !connected(rooms, room, NORTH).claimed
                    && definition.has_opening[UP]
                    && !connected(rooms, room, UP).claimed
                {
                    let north = connection(rooms, room, NORTH);
                    rooms[north].has_opening[UP] && !connected(rooms, north, UP).claimed
                } else {
                    false
                }
            }
            Self::DoubleZ => {
                definition.has_opening[NORTH] && !connected(rooms, room, NORTH).claimed
            }
            Self::DoubleX => definition.has_opening[EAST] && !connected(rooms, room, EAST).claimed,
            Self::DoubleY => definition.has_opening[UP] && !connected(rooms, room, UP).claimed,
            Self::SimpleTop => {
                !definition.has_opening[WEST]
                    && !definition.has_opening[EAST]
                    && !definition.has_opening[NORTH]
                    && !definition.has_opening[SOUTH]
                    && !definition.has_opening[UP]
            }
            Self::Simple => true,
        }
    }

    fn create(
        self,
        orientation: Direction,
        rooms: &mut [RoomDefinition],
        room: usize,
        random: &mut RandomSource,
    ) -> ChildPiece {
        match self {
            Self::DoubleXY => {
                let east = connection(rooms, room, EAST);
                let up = connection(rooms, room, UP);
                let east_up = connection(rooms, east, UP);
                rooms[room].claimed = true;
                rooms[east].claimed = true;
                rooms[up].claimed = true;
                rooms[east_up].claimed = true;
                room_piece(orientation, rooms, room, 2, 2, 1, RoomPieceKind::DoubleXY)
            }
            Self::DoubleYZ => {
                let north = connection(rooms, room, NORTH);
                let up = connection(rooms, room, UP);
                let north_up = connection(rooms, north, UP);
                rooms[room].claimed = true;
                rooms[north].claimed = true;
                rooms[up].claimed = true;
                rooms[north_up].claimed = true;
                room_piece(orientation, rooms, room, 1, 2, 2, RoomPieceKind::DoubleYZ)
            }
            Self::DoubleZ => {
                let mut source = room;
                if !rooms[room].has_opening[NORTH] || connected(rooms, room, NORTH).claimed {
                    source = connection(rooms, room, SOUTH);
                }

                let north = connection(rooms, source, NORTH);
                rooms[source].claimed = true;
                rooms[north].claimed = true;
                room_piece(orientation, rooms, source, 1, 1, 2, RoomPieceKind::DoubleZ)
            }
            Self::DoubleX => {
                let east = connection(rooms, room, EAST);
                rooms[room].claimed = true;
                rooms[east].claimed = true;
                room_piece(orientation, rooms, room, 2, 1, 1, RoomPieceKind::DoubleX)
            }
            Self::DoubleY => {
                let up = connection(rooms, room, UP);
                rooms[room].claimed = true;
                rooms[up].claimed = true;
                room_piece(orientation, rooms, room, 1, 2, 1, RoomPieceKind::DoubleY)
            }
            Self::SimpleTop => {
                rooms[room].claimed = true;
                room_piece(orientation, rooms, room, 1, 1, 1, RoomPieceKind::SimpleTop)
            }
            Self::Simple => {
                rooms[room].claimed = true;
                let main_design = random.next_i32_bounded(3);
                room_piece(
                    orientation,
                    rooms,
                    room,
                    1,
                    1,
                    1,
                    RoomPieceKind::Simple { main_design },
                )
            }
        }
    }
}

#[derive(Debug)]
enum RoomPieceKind {
    Entry,
    Core,
    DoubleX,
    DoubleXY,
    DoubleY,
    DoubleYZ,
    DoubleZ,
    Simple { main_design: i32 },
    SimpleTop,
    Wing { main_design: i32 },
    Penthouse,
}

#[derive(Debug)]
struct ChildPiece {
    base: PieceBase,
    room: usize,
    kind: RoomPieceKind,
}

const fn room_bounding_box(
    orientation: Direction,
    room_index: i32,
    room_width: i32,
    room_height: i32,
    room_depth: i32,
) -> BoundingBox {
    let room_x = room_index % 5;
    let room_z = room_index / 5 % 5;
    let room_y = room_index / 25;
    let bounding_box = PieceBase::make_bounding_box(
        0,
        0,
        0,
        orientation,
        room_width * GRIDROOM_WIDTH,
        room_height * GRIDROOM_HEIGHT,
        room_depth * GRIDROOM_DEPTH,
    );

    match orientation {
        Direction::North => {
            bounding_box.moved(room_x * 8, room_y * 4, -(room_z + room_depth) * 8 + 1)
        }
        Direction::South => bounding_box.moved(room_x * 8, room_y * 4, room_z * 8),
        Direction::West => {
            bounding_box.moved(-(room_z + room_depth) * 8 + 1, room_y * 4, room_x * 8)
        }
        _ => bounding_box.moved(room_z * 8, room_y * 4, room_x * 8),
    }
}

const fn room_piece(
    orientation: Direction,
    rooms: &[RoomDefinition],
    room: usize,
    room_width: i32,
    room_height: i32,
    room_depth: i32,
    kind: RoomPieceKind,
) -> ChildPiece {
    let bounding_box = room_bounding_box(
        orientation,
        rooms[room].index,
        room_width,
        room_height,
        room_depth,
    );
    let mut base = PieceBase::new(1, bounding_box);
    base.set_orientation(Some(orientation));
    ChildPiece { base, room, kind }
}

const fn boxed_piece(
    orientation: Direction,
    bounding_box: BoundingBox,
    kind: RoomPieceKind,
) -> ChildPiece {
    let mut base = PieceBase::new(1, bounding_box);
    base.set_orientation(Some(orientation));
    ChildPiece {
        base,
        room: 0,
        kind,
    }
}

#[derive(Debug)]
pub struct MonumentBuilding {
    rooms: Vec<RoomDefinition>,
    children: Vec<ChildPiece>,
}

pub fn create_top_piece(
    random: &mut RandomSource,
    west: i32,
    north: i32,
    direction: Direction,
) -> StructurePiece {
    let bounding_box =
        PieceBase::make_bounding_box(west, 39, north, direction, WIDTH, HEIGHT + 1, DEPTH);
    let mut base = PieceBase::new(0, bounding_box);
    base.set_orientation(Some(direction));

    let mut rooms = Vec::new();
    let (room_order, source_room, core_room) = generate_room_graph(&mut rooms, random);
    rooms[source_room].claimed = true;

    let mut children = Vec::new();
    children.push(room_piece(
        direction,
        &rooms,
        source_room,
        1,
        1,
        1,
        RoomPieceKind::Entry,
    ));
    children.push(room_piece(
        direction,
        &rooms,
        core_room,
        2,
        2,
        2,
        RoomPieceKind::Core,
    ));

    for room in room_order {
        if rooms[room].claimed || rooms[room].is_special() {
            continue;
        }
        for fitter in FITTERS {
            if fitter.fits(&rooms, room) {
                children.push(fitter.create(direction, &mut rooms, room, random));
                break;
            }
        }
    }

    let offset = base.world_pos(9, 0, 22);
    for child in &mut children {
        child.base.move_by(offset.x(), offset.y(), offset.z());
    }

    let left_wing = BoundingBox::new(base.world_pos(1, 1, 1).0, base.world_pos(23, 8, 21).0);
    let right_wing = BoundingBox::new(base.world_pos(34, 1, 1).0, base.world_pos(56, 8, 21).0);
    let penthouse = BoundingBox::new(base.world_pos(22, 13, 22).0, base.world_pos(35, 17, 35).0);
    let mut wing_random = random.next_i32();
    children.push(boxed_piece(
        direction,
        left_wing,
        RoomPieceKind::Wing {
            main_design: wing_random & 1,
        },
    ));
    wing_random = wing_random.wrapping_add(1);
    children.push(boxed_piece(
        direction,
        right_wing,
        RoomPieceKind::Wing {
            main_design: wing_random & 1,
        },
    ));
    children.push(boxed_piece(direction, penthouse, RoomPieceKind::Penthouse));

    StructurePiece::new(
        base,
        PieceKind::OceanMonument(MonumentBuilding { rooms, children }),
    )
}

fn generate_room_graph(
    rooms: &mut Vec<RoomDefinition>,
    random: &mut RandomSource,
) -> (Vec<usize>, usize, usize) {
    let mut room_grid: [Option<usize>; GRID_SIZE] = [None; GRID_SIZE];

    for x in 0..5 {
        for z in 0..4 {
            let pos = get_room_index(x, 0, z);
            rooms.push(RoomDefinition::new(pos as i32));
            room_grid[pos] = Some(rooms.len() - 1);
        }
    }

    for x in 0..5 {
        for z in 0..4 {
            let pos = get_room_index(x, 1, z);
            rooms.push(RoomDefinition::new(pos as i32));
            room_grid[pos] = Some(rooms.len() - 1);
        }
    }

    for x in 1..4 {
        for z in 0..2 {
            let pos = get_room_index(x, 2, z);
            rooms.push(RoomDefinition::new(pos as i32));
            room_grid[pos] = Some(rooms.len() - 1);
        }
    }

    let source_room = grid_room(&room_grid, GRIDROOM_SOURCE_INDEX);

    for x in 0..5 {
        for z in 0..5 {
            for y in 0..3 {
                let pos = get_room_index(x, y, z);
                let Some(room) = room_grid[pos] else {
                    continue;
                };
                for direction in Direction::ALL {
                    let (step_x, step_y, step_z) = direction.offset();
                    let neigh_x = x + step_x;
                    let neigh_y = y + step_y;
                    let neigh_z = z + step_z;
                    if !(0..5).contains(&neigh_x)
                        || !(0..5).contains(&neigh_z)
                        || !(0..3).contains(&neigh_y)
                    {
                        continue;
                    }
                    let neigh_pos = get_room_index(neigh_x, neigh_y, neigh_z);
                    let Some(neighbor) = room_grid[neigh_pos] else {
                        continue;
                    };
                    if neigh_z == z {
                        set_connection(rooms, room, direction, neighbor);
                    } else {
                        set_connection(rooms, room, direction.opposite(), neighbor);
                    }
                }
            }
        }
    }

    rooms.push(RoomDefinition::new(PENTHOUSE_INDEX));
    let roof_room = rooms.len() - 1;
    rooms.push(RoomDefinition::new(LEFTWING_INDEX));
    let left_wing = rooms.len() - 1;
    rooms.push(RoomDefinition::new(RIGHTWING_INDEX));
    let right_wing = rooms.len() - 1;

    set_connection(
        rooms,
        grid_room(&room_grid, GRIDROOM_TOP_CONNECT_INDEX),
        Direction::Up,
        roof_room,
    );
    set_connection(
        rooms,
        grid_room(&room_grid, GRIDROOM_LEFTWING_CONNECT_INDEX),
        Direction::South,
        left_wing,
    );
    set_connection(
        rooms,
        grid_room(&room_grid, GRIDROOM_RIGHTWING_CONNECT_INDEX),
        Direction::South,
        right_wing,
    );
    rooms[roof_room].claimed = true;
    rooms[left_wing].claimed = true;
    rooms[right_wing].claimed = true;
    rooms[source_room].is_source = true;

    let core_room = grid_room(&room_grid, get_room_index(random.next_i32_bounded(4), 0, 2));
    rooms[core_room].claimed = true;
    let core_east = connection(rooms, core_room, EAST);
    let core_north = connection(rooms, core_room, NORTH);
    let core_east_north = connection(rooms, core_east, NORTH);
    let core_up = connection(rooms, core_room, UP);
    let core_east_up = connection(rooms, core_east, UP);
    let core_north_up = connection(rooms, core_north, UP);
    let core_east_north_up = connection(rooms, core_east_north, UP);
    rooms[core_east].claimed = true;
    rooms[core_north].claimed = true;
    rooms[core_east_north].claimed = true;
    rooms[core_up].claimed = true;
    rooms[core_east_up].claimed = true;
    rooms[core_north_up].claimed = true;
    rooms[core_east_north_up].claimed = true;

    let mut room_order = Vec::new();
    for room in room_grid.into_iter().flatten() {
        update_openings(rooms, room);
        room_order.push(room);
    }
    update_openings(rooms, roof_room);

    shuffle(&mut room_order, random);

    let mut scan_index = 1;
    for room in &room_order {
        let room = *room;
        let mut close_count = 0;
        let mut attempt_count = 0;

        while close_count < 2 && attempt_count < 5 {
            attempt_count += 1;
            let f = random.next_i32_bounded(6) as usize;
            if !rooms[room].has_opening[f] {
                continue;
            }
            let neighbor = connection(rooms, room, f);
            let of = Direction::from_3d_data_value(f)
                .opposite()
                .get_3d_data_value();
            rooms[room].has_opening[f] = false;
            rooms[neighbor].has_opening[of] = false;
            let first = find_source(rooms, room, scan_index);
            scan_index += 1;
            let reachable = first && {
                let second = find_source(rooms, neighbor, scan_index);
                scan_index += 1;
                second
            };
            if reachable {
                close_count += 1;
            } else {
                rooms[room].has_opening[f] = true;
                rooms[neighbor].has_opening[of] = true;
            }
        }
    }

    room_order.push(roof_room);
    room_order.push(left_wing);
    room_order.push(right_wing);
    (room_order, source_room, core_room)
}

#[expect(
    clippy::too_many_arguments,
    reason = "mirrors vanilla generateWaterBox"
)]
fn generate_water_box(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    chunk_bb: BoundingBox,
    x0: i32,
    y0: i32,
    z0: i32,
    x1: i32,
    y1: i32,
    z1: i32,
) {
    for y in y0..=y1 {
        for x in x0..=x1 {
            for z in z0..=z1 {
                let state = base.get_block(region, x, y, z, chunk_bb);
                if is_fill_keep(state) {
                    continue;
                }
                if base.world_y(y) >= region.sea_level() && state != fill_block() {
                    base.place_block(region, air(), x, y, z, chunk_bb);
                } else {
                    base.place_block(region, fill_block(), x, y, z, chunk_bb);
                }
            }
        }
    }
}

fn generate_default_floor(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    chunk_bb: BoundingBox,
    x_off: i32,
    z_off: i32,
    down_opening: bool,
) {
    let gray = base_gray();
    let light = base_light();
    let gbox = |x0, y0, z0, x1, y1, z1, state| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, false,
        );
    };

    if down_opening {
        gbox(
            x_off,
            0,
            z_off,
            x_off + 2,
            0,
            z_off + GRIDROOM_DEPTH - 1,
            gray,
        );
        gbox(
            x_off + 5,
            0,
            z_off,
            x_off + GRIDROOM_WIDTH - 1,
            0,
            z_off + GRIDROOM_DEPTH - 1,
            gray,
        );
        gbox(x_off + 3, 0, z_off, x_off + 4, 0, z_off + 2, gray);
        gbox(
            x_off + 3,
            0,
            z_off + 5,
            x_off + 4,
            0,
            z_off + GRIDROOM_DEPTH - 1,
            gray,
        );
        gbox(x_off + 3, 0, z_off + 2, x_off + 4, 0, z_off + 2, light);
        gbox(x_off + 3, 0, z_off + 5, x_off + 4, 0, z_off + 5, light);
        gbox(x_off + 2, 0, z_off + 3, x_off + 2, 0, z_off + 4, light);
        gbox(x_off + 5, 0, z_off + 3, x_off + 5, 0, z_off + 4, light);
    } else {
        gbox(
            x_off,
            0,
            z_off,
            x_off + GRIDROOM_WIDTH - 1,
            0,
            z_off + GRIDROOM_DEPTH - 1,
            gray,
        );
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "mirrors vanilla generateBoxOnFillOnly"
)]
fn generate_box_on_fill_only(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    chunk_bb: BoundingBox,
    x0: i32,
    y0: i32,
    z0: i32,
    x1: i32,
    y1: i32,
    z1: i32,
    target_block: BlockState,
) {
    for y in y0..=y1 {
        for x in x0..=x1 {
            for z in z0..=z1 {
                if base.get_block(region, x, y, z, chunk_bb) == fill_block() {
                    base.place_block(region, target_block, x, y, z, chunk_bb);
                }
            }
        }
    }
}

const fn chunk_intersects(
    base: &PieceBase,
    chunk_bb: BoundingBox,
    x0: i32,
    z0: i32,
    x1: i32,
    z1: i32,
) -> bool {
    let wx0 = base.world_x(x0, z0);
    let wz0 = base.world_z(x0, z0);
    let wx1 = base.world_x(x1, z1);
    let wz1 = base.world_z(x1, z1);
    chunk_bb.intersects_xz(wx0.min(wx1), wz0.min(wz1), wx0.max(wx1), wz0.max(wz1))
}

impl MonumentBuilding {
    pub fn post_process(
        &mut self,
        base: &PieceBase,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        chunk_bb: BoundingBox,
    ) {
        let water_height = region.sea_level().max(64) - base.bounding_box.min_y();
        generate_water_box(base, region, chunk_bb, 0, 0, 0, 58, water_height, 58);
        generate_wing(base, region, chunk_bb, false, 0);
        generate_wing(base, region, chunk_bb, true, 33);
        generate_entrance_archs(base, region, chunk_bb);
        generate_entrance_wall(base, region, chunk_bb);
        generate_roof_piece(base, region, chunk_bb);
        generate_lower_wall(base, region, chunk_bb);
        generate_middle_wall(base, region, chunk_bb);
        generate_upper_wall(base, region, chunk_bb);

        let light = base_light();
        for pillar_x in 0..7 {
            let mut pillar_z = 0;
            while pillar_z < 7 {
                if pillar_z == 0 && pillar_x == 3 {
                    pillar_z = 6;
                }

                let bx = pillar_x * 9;
                let bz = pillar_z * 9;

                for w in 0..4 {
                    for d in 0..4 {
                        base.place_block(region, light, bx + w, 0, bz + d, chunk_bb);
                        base.fill_column_down(region, light, bx + w, -1, bz + d, chunk_bb);
                    }
                }

                if pillar_x == 0 || pillar_x == 6 {
                    pillar_z += 1;
                } else {
                    pillar_z += 6;
                }
            }
        }

        for i in 0..5 {
            generate_water_box(
                base,
                region,
                chunk_bb,
                -1 - i,
                i * 2,
                -1 - i,
                -1 - i,
                23,
                58 + i,
            );
            generate_water_box(
                base,
                region,
                chunk_bb,
                58 + i,
                i * 2,
                -1 - i,
                58 + i,
                23,
                58 + i,
            );
            generate_water_box(
                base,
                region,
                chunk_bb,
                -i,
                i * 2,
                -1 - i,
                57 + i,
                23,
                -1 - i,
            );
            generate_water_box(
                base,
                region,
                chunk_bb,
                -i,
                i * 2,
                58 + i,
                57 + i,
                23,
                58 + i,
            );
        }

        for child in &mut self.children {
            if child.base.bounding_box.intersects(chunk_bb) {
                child.post_process(&self.rooms, region, random, chunk_bb);
            }
        }
    }
}

impl ChildPiece {
    fn post_process(
        &mut self,
        rooms: &[RoomDefinition],
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        chunk_bb: BoundingBox,
    ) {
        let base = &self.base;
        let room = self.room;
        match self.kind {
            RoomPieceKind::Entry => entry_room(base, region, chunk_bb, rooms, room),
            RoomPieceKind::Core => core_room(base, region, chunk_bb),
            RoomPieceKind::DoubleX => double_x_room(base, region, chunk_bb, rooms, room),
            RoomPieceKind::DoubleXY => double_xy_room(base, region, chunk_bb, rooms, room),
            RoomPieceKind::DoubleY => double_y_room(base, region, chunk_bb, rooms, room),
            RoomPieceKind::DoubleYZ => double_yz_room(base, region, chunk_bb, rooms, room),
            RoomPieceKind::DoubleZ => double_z_room(base, region, chunk_bb, rooms, room),
            RoomPieceKind::Simple { main_design } => {
                simple_room(base, region, random, chunk_bb, rooms, room, main_design);
            }
            RoomPieceKind::SimpleTop => {
                simple_top_room(base, region, random, chunk_bb, rooms, room);
            }
            RoomPieceKind::Wing { main_design } => {
                wing_room(base, region, chunk_bb, main_design);
            }
            RoomPieceKind::Penthouse => penthouse(base, region, chunk_bb),
        }
    }
}

fn generate_wing(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    chunk_bb: BoundingBox,
    is_flipped: bool,
    xoff: i32,
) {
    if !chunk_intersects(base, chunk_bb, xoff, 0, xoff + 23, 20) {
        return;
    }

    let gray = base_gray();
    let light = base_light();
    let deco = dot_deco_data();
    let gbox = |x0, y0, z0, x1, y1, z1, state| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, false,
        );
    };
    let pb = |x, y, z, state| base.place_block(region, state, x, y, z, chunk_bb);

    gbox(xoff, 0, 0, xoff + 24, 0, 20, gray);
    generate_water_box(base, region, chunk_bb, xoff, 1, 0, xoff + 24, 10, 20);

    for i in 0..4 {
        gbox(xoff + i, i + 1, i, xoff + i, i + 1, 20, light);
        gbox(xoff + i + 7, i + 5, i + 7, xoff + i + 7, i + 5, 20, light);
        gbox(xoff + 17 - i, i + 5, i + 7, xoff + 17 - i, i + 5, 20, light);
        gbox(xoff + 24 - i, i + 1, i, xoff + 24 - i, i + 1, 20, light);
        gbox(xoff + i + 1, i + 1, i, xoff + 23 - i, i + 1, i, light);
        gbox(
            xoff + i + 8,
            i + 5,
            i + 7,
            xoff + 16 - i,
            i + 5,
            i + 7,
            light,
        );
    }

    gbox(xoff + 4, 4, 4, xoff + 6, 4, 20, gray);
    gbox(xoff + 7, 4, 4, xoff + 17, 4, 6, gray);
    gbox(xoff + 18, 4, 4, xoff + 20, 4, 20, gray);
    gbox(xoff + 11, 8, 11, xoff + 13, 8, 20, gray);
    pb(xoff + 12, 9, 12, deco);
    pb(xoff + 12, 9, 15, deco);
    pb(xoff + 12, 9, 18, deco);

    let left_pos = xoff + if is_flipped { 19 } else { 5 };
    let right_pos = xoff + if is_flipped { 5 } else { 19 };

    let mut z = 20;
    while z >= 5 {
        pb(left_pos, 5, z, deco);
        z -= 3;
    }

    let mut z = 19;
    while z >= 7 {
        pb(right_pos, 5, z, deco);
        z -= 3;
    }

    for i in 0..4 {
        let pos = if is_flipped {
            xoff + 24 - (17 - i * 3)
        } else {
            xoff + 17 - i * 3
        };
        pb(pos, 5, 5, deco);
    }

    pb(right_pos, 5, 5, deco);
    gbox(xoff + 11, 1, 12, xoff + 13, 7, 12, gray);
    gbox(xoff + 12, 1, 11, xoff + 12, 7, 13, gray);
}

fn generate_entrance_archs(base: &PieceBase, region: &WorldGenRegion<'_>, chunk_bb: BoundingBox) {
    if !chunk_intersects(base, chunk_bb, 22, 5, 35, 17) {
        return;
    }

    let gray = base_gray();
    let light = base_light();
    let lamp = lamp_block();
    let gbox = |x0, y0, z0, x1, y1, z1, state| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, false,
        );
    };
    let pb = |x, y, z, state| base.place_block(region, state, x, y, z, chunk_bb);

    generate_water_box(base, region, chunk_bb, 25, 0, 0, 32, 8, 20);

    for i in 0..4 {
        gbox(24, 2, 5 + i * 4, 24, 4, 5 + i * 4, light);
        gbox(22, 4, 5 + i * 4, 23, 4, 5 + i * 4, light);
        pb(25, 5, 5 + i * 4, light);
        pb(26, 6, 5 + i * 4, light);
        pb(26, 5, 5 + i * 4, lamp);
        gbox(33, 2, 5 + i * 4, 33, 4, 5 + i * 4, light);
        gbox(34, 4, 5 + i * 4, 35, 4, 5 + i * 4, light);
        pb(32, 5, 5 + i * 4, light);
        pb(31, 6, 5 + i * 4, light);
        pb(31, 5, 5 + i * 4, lamp);
        gbox(27, 6, 5 + i * 4, 30, 6, 5 + i * 4, gray);
    }
}

fn generate_entrance_wall(base: &PieceBase, region: &WorldGenRegion<'_>, chunk_bb: BoundingBox) {
    if !chunk_intersects(base, chunk_bb, 15, 20, 42, 21) {
        return;
    }

    let gray = base_gray();
    let light = base_light();
    let black = base_black();
    let gbox = |x0, y0, z0, x1, y1, z1, state| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, false,
        );
    };
    let pb = |x, y, z, state| base.place_block(region, state, x, y, z, chunk_bb);
    let water = |x0, y0, z0, x1, y1, z1| {
        generate_water_box(base, region, chunk_bb, x0, y0, z0, x1, y1, z1);
    };

    gbox(15, 0, 21, 42, 0, 21, gray);
    water(26, 1, 21, 31, 3, 21);
    gbox(21, 12, 21, 36, 12, 21, gray);
    gbox(17, 11, 21, 40, 11, 21, gray);
    gbox(16, 10, 21, 41, 10, 21, gray);
    gbox(15, 7, 21, 42, 9, 21, gray);
    gbox(16, 6, 21, 41, 6, 21, gray);
    gbox(17, 5, 21, 40, 5, 21, gray);
    gbox(21, 4, 21, 36, 4, 21, gray);
    gbox(22, 3, 21, 26, 3, 21, gray);
    gbox(31, 3, 21, 35, 3, 21, gray);
    gbox(23, 2, 21, 25, 2, 21, gray);
    gbox(32, 2, 21, 34, 2, 21, gray);
    gbox(28, 4, 20, 29, 4, 21, light);
    pb(27, 3, 21, light);
    pb(30, 3, 21, light);
    pb(26, 2, 21, light);
    pb(31, 2, 21, light);
    pb(25, 1, 21, light);
    pb(32, 1, 21, light);

    for i in 0..7 {
        pb(28 - i, 6 + i, 21, black);
        pb(29 + i, 6 + i, 21, black);
    }

    for i in 0..4 {
        pb(28 - i, 9 + i, 21, black);
        pb(29 + i, 9 + i, 21, black);
    }

    pb(28, 12, 21, black);
    pb(29, 12, 21, black);

    for i in 0..3 {
        pb(22 - i * 2, 8, 21, black);
        pb(22 - i * 2, 9, 21, black);
        pb(35 + i * 2, 8, 21, black);
        pb(35 + i * 2, 9, 21, black);
    }

    water(15, 13, 21, 42, 15, 21);
    water(15, 1, 21, 15, 6, 21);
    water(16, 1, 21, 16, 5, 21);
    water(17, 1, 21, 20, 4, 21);
    water(21, 1, 21, 21, 3, 21);
    water(22, 1, 21, 22, 2, 21);
    water(23, 1, 21, 24, 1, 21);
    water(42, 1, 21, 42, 6, 21);
    water(41, 1, 21, 41, 5, 21);
    water(37, 1, 21, 40, 4, 21);
    water(36, 1, 21, 36, 3, 21);
    water(33, 1, 21, 34, 1, 21);
    water(35, 1, 21, 35, 2, 21);
}

fn generate_roof_piece(base: &PieceBase, region: &WorldGenRegion<'_>, chunk_bb: BoundingBox) {
    if !chunk_intersects(base, chunk_bb, 21, 21, 36, 36) {
        return;
    }

    let gray = base_gray();
    let light = base_light();
    let lamp = lamp_block();
    let gbox = |x0, y0, z0, x1, y1, z1, state| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, false,
        );
    };
    let pb = |x, y, z, state| base.place_block(region, state, x, y, z, chunk_bb);

    gbox(21, 0, 22, 36, 0, 36, gray);
    generate_water_box(base, region, chunk_bb, 21, 1, 22, 36, 23, 36);

    for i in 0..4 {
        gbox(21 + i, 13 + i, 21 + i, 36 - i, 13 + i, 21 + i, light);
        gbox(21 + i, 13 + i, 36 - i, 36 - i, 13 + i, 36 - i, light);
        gbox(21 + i, 13 + i, 22 + i, 21 + i, 13 + i, 35 - i, light);
        gbox(36 - i, 13 + i, 22 + i, 36 - i, 13 + i, 35 - i, light);
    }

    gbox(25, 16, 25, 32, 16, 32, gray);
    gbox(25, 17, 25, 25, 19, 25, light);
    gbox(32, 17, 25, 32, 19, 25, light);
    gbox(25, 17, 32, 25, 19, 32, light);
    gbox(32, 17, 32, 32, 19, 32, light);
    pb(26, 20, 26, light);
    pb(27, 21, 27, light);
    pb(27, 20, 27, lamp);
    pb(26, 20, 31, light);
    pb(27, 21, 30, light);
    pb(27, 20, 30, lamp);
    pb(31, 20, 31, light);
    pb(30, 21, 30, light);
    pb(30, 20, 30, lamp);
    pb(31, 20, 26, light);
    pb(30, 21, 27, light);
    pb(30, 20, 27, lamp);
    gbox(28, 21, 27, 29, 21, 27, gray);
    gbox(27, 21, 28, 27, 21, 29, gray);
    gbox(28, 21, 30, 29, 21, 30, gray);
    gbox(30, 21, 28, 30, 21, 29, gray);
}

fn generate_lower_wall(base: &PieceBase, region: &WorldGenRegion<'_>, chunk_bb: BoundingBox) {
    let gray = base_gray();
    let light = base_light();
    let deco = dot_deco_data();
    let gbox = |x0, y0, z0, x1, y1, z1, state| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, false,
        );
    };
    let pb = |x, y, z, state| base.place_block(region, state, x, y, z, chunk_bb);

    if chunk_intersects(base, chunk_bb, 0, 21, 6, 58) {
        gbox(0, 0, 21, 6, 0, 57, gray);
        generate_water_box(base, region, chunk_bb, 0, 1, 21, 6, 7, 57);
        gbox(4, 4, 21, 6, 4, 53, gray);

        for i in 0..4 {
            gbox(i, i + 1, 21, i, i + 1, 57 - i, light);
        }

        let mut z = 23;
        while z < 53 {
            pb(5, 5, z, deco);
            z += 3;
        }

        pb(5, 5, 52, deco);

        for i in 0..4 {
            gbox(i, i + 1, 21, i, i + 1, 57 - i, light);
        }

        gbox(4, 1, 52, 6, 3, 52, gray);
        gbox(5, 1, 51, 5, 3, 53, gray);
    }

    if chunk_intersects(base, chunk_bb, 51, 21, 58, 58) {
        gbox(51, 0, 21, 57, 0, 57, gray);
        generate_water_box(base, region, chunk_bb, 51, 1, 21, 57, 7, 57);
        gbox(51, 4, 21, 53, 4, 53, gray);

        for i in 0..4 {
            gbox(57 - i, i + 1, 21, 57 - i, i + 1, 57 - i, light);
        }

        let mut z = 23;
        while z < 53 {
            pb(52, 5, z, deco);
            z += 3;
        }

        pb(52, 5, 52, deco);
        gbox(51, 1, 52, 53, 3, 52, gray);
        gbox(52, 1, 51, 52, 3, 53, gray);
    }

    if chunk_intersects(base, chunk_bb, 0, 51, 57, 57) {
        gbox(7, 0, 51, 50, 0, 57, gray);
        generate_water_box(base, region, chunk_bb, 7, 1, 51, 50, 10, 57);

        for i in 0..4 {
            gbox(i + 1, i + 1, 57 - i, 56 - i, i + 1, 57 - i, light);
        }
    }
}

fn generate_middle_wall(base: &PieceBase, region: &WorldGenRegion<'_>, chunk_bb: BoundingBox) {
    let gray = base_gray();
    let light = base_light();
    let deco = dot_deco_data();
    let gbox = |x0, y0, z0, x1, y1, z1, state| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, false,
        );
    };
    let pb = |x, y, z, state| base.place_block(region, state, x, y, z, chunk_bb);

    if chunk_intersects(base, chunk_bb, 7, 21, 13, 50) {
        gbox(7, 0, 21, 13, 0, 50, gray);
        generate_water_box(base, region, chunk_bb, 7, 1, 21, 13, 10, 50);
        gbox(11, 8, 21, 13, 8, 53, gray);

        for i in 0..4 {
            gbox(i + 7, i + 5, 21, i + 7, i + 5, 54, light);
        }

        let mut z = 21;
        while z <= 45 {
            pb(12, 9, z, deco);
            z += 3;
        }
    }

    if chunk_intersects(base, chunk_bb, 44, 21, 50, 54) {
        gbox(44, 0, 21, 50, 0, 50, gray);
        generate_water_box(base, region, chunk_bb, 44, 1, 21, 50, 10, 50);
        gbox(44, 8, 21, 46, 8, 53, gray);

        for i in 0..4 {
            gbox(50 - i, i + 5, 21, 50 - i, i + 5, 54, light);
        }

        let mut z = 21;
        while z <= 45 {
            pb(45, 9, z, deco);
            z += 3;
        }
    }

    if chunk_intersects(base, chunk_bb, 8, 44, 49, 54) {
        gbox(14, 0, 44, 43, 0, 50, gray);
        generate_water_box(base, region, chunk_bb, 14, 1, 44, 43, 10, 50);

        let mut x = 12;
        while x <= 45 {
            pb(x, 9, 45, deco);
            pb(x, 9, 52, deco);
            if x == 12 || x == 18 || x == 24 || x == 33 || x == 39 || x == 45 {
                pb(x, 9, 47, deco);
                pb(x, 9, 50, deco);
                pb(x, 10, 45, deco);
                pb(x, 10, 46, deco);
                pb(x, 10, 51, deco);
                pb(x, 10, 52, deco);
                pb(x, 11, 47, deco);
                pb(x, 11, 50, deco);
                pb(x, 12, 48, deco);
                pb(x, 12, 49, deco);
            }
            x += 3;
        }

        for i in 0..3 {
            gbox(8 + i, 5 + i, 54, 49 - i, 5 + i, 54, gray);
        }

        gbox(11, 8, 54, 46, 8, 54, light);
        gbox(14, 8, 44, 43, 8, 53, gray);
    }
}

fn generate_upper_wall(base: &PieceBase, region: &WorldGenRegion<'_>, chunk_bb: BoundingBox) {
    let gray = base_gray();
    let light = base_light();
    let deco = dot_deco_data();
    let gbox = |x0, y0, z0, x1, y1, z1, state| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, false,
        );
    };
    let pb = |x, y, z, state| base.place_block(region, state, x, y, z, chunk_bb);

    if chunk_intersects(base, chunk_bb, 14, 21, 20, 43) {
        gbox(14, 0, 21, 20, 0, 43, gray);
        generate_water_box(base, region, chunk_bb, 14, 1, 22, 20, 14, 43);
        gbox(18, 12, 22, 20, 12, 39, gray);
        gbox(18, 12, 21, 20, 12, 21, light);

        for i in 0..4 {
            gbox(i + 14, i + 9, 21, i + 14, i + 9, 43 - i, light);
        }

        let mut z = 23;
        while z <= 39 {
            pb(19, 13, z, deco);
            z += 3;
        }
    }

    if chunk_intersects(base, chunk_bb, 37, 21, 43, 43) {
        gbox(37, 0, 21, 43, 0, 43, gray);
        generate_water_box(base, region, chunk_bb, 37, 1, 22, 43, 14, 43);
        gbox(37, 12, 22, 39, 12, 39, gray);
        gbox(37, 12, 21, 39, 12, 21, light);

        for i in 0..4 {
            gbox(43 - i, i + 9, 21, 43 - i, i + 9, 43 - i, light);
        }

        let mut z = 23;
        while z <= 39 {
            pb(38, 13, z, deco);
            z += 3;
        }
    }

    if chunk_intersects(base, chunk_bb, 15, 37, 42, 43) {
        gbox(21, 0, 37, 36, 0, 43, gray);
        generate_water_box(base, region, chunk_bb, 21, 1, 37, 36, 14, 43);
        gbox(21, 12, 37, 36, 12, 39, gray);

        for i in 0..4 {
            gbox(15 + i, i + 9, 43 - i, 42 - i, i + 9, 43 - i, light);
        }

        let mut x = 21;
        while x <= 36 {
            pb(x, 13, 38, deco);
            x += 3;
        }
    }
}

fn core_room(base: &PieceBase, region: &WorldGenRegion<'_>, chunk_bb: BoundingBox) {
    let gray = base_gray();
    let light = base_light();
    let black = base_black();
    let lamp = lamp_block();
    let gold = vanilla_blocks::GOLD_BLOCK.default_state();
    let gbox = |x0, y0, z0, x1, y1, z1, state| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, false,
        );
    };
    let pb = |x, y, z, state| base.place_block(region, state, x, y, z, chunk_bb);

    generate_box_on_fill_only(base, region, chunk_bb, 1, 8, 0, 14, 8, 14, gray);
    gbox(0, 7, 0, 0, 7, 15, light);
    gbox(15, 7, 0, 15, 7, 15, light);
    gbox(1, 7, 0, 15, 7, 0, light);
    gbox(1, 7, 15, 14, 7, 15, light);

    for yx in 1..=6 {
        let block = if yx == 2 || yx == 6 { gray } else { light };

        let mut x = 0;
        while x <= 15 {
            gbox(x, yx, 0, x, yx, 1, block);
            gbox(x, yx, 6, x, yx, 9, block);
            gbox(x, yx, 14, x, yx, 15, block);
            x += 15;
        }

        gbox(1, yx, 0, 1, yx, 0, block);
        gbox(6, yx, 0, 9, yx, 0, block);
        gbox(14, yx, 0, 14, yx, 0, block);
        gbox(1, yx, 15, 14, yx, 15, block);
    }

    gbox(6, 3, 6, 9, 6, 9, black);
    gbox(7, 4, 7, 8, 5, 8, gold);

    let mut yx = 3;
    while yx <= 6 {
        let mut x = 6;
        while x <= 9 {
            pb(x, yx, 6, lamp);
            pb(x, yx, 9, lamp);
            x += 3;
        }
        yx += 3;
    }

    gbox(5, 1, 6, 5, 2, 6, light);
    gbox(5, 1, 9, 5, 2, 9, light);
    gbox(10, 1, 6, 10, 2, 6, light);
    gbox(10, 1, 9, 10, 2, 9, light);
    gbox(6, 1, 5, 6, 2, 5, light);
    gbox(9, 1, 5, 9, 2, 5, light);
    gbox(6, 1, 10, 6, 2, 10, light);
    gbox(9, 1, 10, 9, 2, 10, light);
    gbox(5, 2, 5, 5, 6, 5, light);
    gbox(5, 2, 10, 5, 6, 10, light);
    gbox(10, 2, 5, 10, 6, 5, light);
    gbox(10, 2, 10, 10, 6, 10, light);
    gbox(5, 7, 1, 5, 7, 6, light);
    gbox(10, 7, 1, 10, 7, 6, light);
    gbox(5, 7, 9, 5, 7, 14, light);
    gbox(10, 7, 9, 10, 7, 14, light);
    gbox(1, 7, 5, 6, 7, 5, light);
    gbox(1, 7, 10, 6, 7, 10, light);
    gbox(9, 7, 5, 14, 7, 5, light);
    gbox(9, 7, 10, 14, 7, 10, light);
    gbox(2, 1, 2, 2, 1, 3, light);
    gbox(3, 1, 2, 3, 1, 2, light);
    gbox(13, 1, 2, 13, 1, 3, light);
    gbox(12, 1, 2, 12, 1, 2, light);
    gbox(2, 1, 12, 2, 1, 13, light);
    gbox(3, 1, 13, 3, 1, 13, light);
    gbox(13, 1, 12, 13, 1, 13, light);
    gbox(12, 1, 13, 12, 1, 13, light);
}

fn double_x_room(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    chunk_bb: BoundingBox,
    rooms: &[RoomDefinition],
    room: usize,
) {
    let gray = base_gray();
    let light = base_light();
    let lamp = lamp_block();
    let gbox = |x0, y0, z0, x1, y1, z1, state| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, false,
        );
    };
    let pb = |x, y, z, state| base.place_block(region, state, x, y, z, chunk_bb);
    let water = |x0, y0, z0, x1, y1, z1| {
        generate_water_box(base, region, chunk_bb, x0, y0, z0, x1, y1, z1);
    };

    let east = connected(rooms, room, EAST);
    let west = &rooms[room];

    if west.index / GRID_FLOOR_COUNT > 0 {
        generate_default_floor(base, region, chunk_bb, 8, 0, east.has_opening[DOWN]);
        generate_default_floor(base, region, chunk_bb, 0, 0, west.has_opening[DOWN]);
    }

    if west.connections[UP].is_none() {
        generate_box_on_fill_only(base, region, chunk_bb, 1, 4, 1, 7, 4, 6, gray);
    }

    if east.connections[UP].is_none() {
        generate_box_on_fill_only(base, region, chunk_bb, 8, 4, 1, 14, 4, 6, gray);
    }

    gbox(0, 3, 0, 0, 3, 7, light);
    gbox(15, 3, 0, 15, 3, 7, light);
    gbox(1, 3, 0, 15, 3, 0, light);
    gbox(1, 3, 7, 14, 3, 7, light);
    gbox(0, 2, 0, 0, 2, 7, gray);
    gbox(15, 2, 0, 15, 2, 7, gray);
    gbox(1, 2, 0, 15, 2, 0, gray);
    gbox(1, 2, 7, 14, 2, 7, gray);
    gbox(0, 1, 0, 0, 1, 7, light);
    gbox(15, 1, 0, 15, 1, 7, light);
    gbox(1, 1, 0, 15, 1, 0, light);
    gbox(1, 1, 7, 14, 1, 7, light);
    gbox(5, 1, 0, 10, 1, 4, light);
    gbox(6, 2, 0, 9, 2, 3, gray);
    gbox(5, 3, 0, 10, 3, 4, light);
    pb(6, 2, 3, lamp);
    pb(9, 2, 3, lamp);

    if west.has_opening[SOUTH] {
        water(3, 1, 0, 4, 2, 0);
    }

    if west.has_opening[NORTH] {
        water(3, 1, 7, 4, 2, 7);
    }

    if west.has_opening[WEST] {
        water(0, 1, 3, 0, 2, 4);
    }

    if east.has_opening[SOUTH] {
        water(11, 1, 0, 12, 2, 0);
    }

    if east.has_opening[NORTH] {
        water(11, 1, 7, 12, 2, 7);
    }

    if east.has_opening[EAST] {
        water(15, 1, 3, 15, 2, 4);
    }
}

fn double_xy_room(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    chunk_bb: BoundingBox,
    rooms: &[RoomDefinition],
    room: usize,
) {
    let gray = base_gray();
    let light = base_light();
    let lamp = lamp_block();
    let gbox = |x0, y0, z0, x1, y1, z1, state| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, false,
        );
    };
    let pb = |x, y, z, state| base.place_block(region, state, x, y, z, chunk_bb);
    let water = |x0, y0, z0, x1, y1, z1| {
        generate_water_box(base, region, chunk_bb, x0, y0, z0, x1, y1, z1);
    };

    let east_index = connection(rooms, room, EAST);
    let east = &rooms[east_index];
    let west = &rooms[room];
    let west_up = connected(rooms, room, UP);
    let east_up = connected(rooms, east_index, UP);

    if west.index / GRID_FLOOR_COUNT > 0 {
        generate_default_floor(base, region, chunk_bb, 8, 0, east.has_opening[DOWN]);
        generate_default_floor(base, region, chunk_bb, 0, 0, west.has_opening[DOWN]);
    }

    if west_up.connections[UP].is_none() {
        generate_box_on_fill_only(base, region, chunk_bb, 1, 8, 1, 7, 8, 6, gray);
    }

    if east_up.connections[UP].is_none() {
        generate_box_on_fill_only(base, region, chunk_bb, 8, 8, 1, 14, 8, 6, gray);
    }

    for y in 1..=7 {
        let block = if y == 2 || y == 6 { gray } else { light };
        gbox(0, y, 0, 0, y, 7, block);
        gbox(15, y, 0, 15, y, 7, block);
        gbox(1, y, 0, 15, y, 0, block);
        gbox(1, y, 7, 14, y, 7, block);
    }

    gbox(2, 1, 3, 2, 7, 4, light);
    gbox(3, 1, 2, 4, 7, 2, light);
    gbox(3, 1, 5, 4, 7, 5, light);
    gbox(13, 1, 3, 13, 7, 4, light);
    gbox(11, 1, 2, 12, 7, 2, light);
    gbox(11, 1, 5, 12, 7, 5, light);
    gbox(5, 1, 3, 5, 3, 4, light);
    gbox(10, 1, 3, 10, 3, 4, light);
    gbox(5, 7, 2, 10, 7, 5, light);
    gbox(5, 5, 2, 5, 7, 2, light);
    gbox(10, 5, 2, 10, 7, 2, light);
    gbox(5, 5, 5, 5, 7, 5, light);
    gbox(10, 5, 5, 10, 7, 5, light);
    pb(6, 6, 2, light);
    pb(9, 6, 2, light);
    pb(6, 6, 5, light);
    pb(9, 6, 5, light);
    gbox(5, 4, 3, 6, 4, 4, light);
    gbox(9, 4, 3, 10, 4, 4, light);
    pb(5, 4, 2, lamp);
    pb(5, 4, 5, lamp);
    pb(10, 4, 2, lamp);
    pb(10, 4, 5, lamp);

    if west.has_opening[SOUTH] {
        water(3, 1, 0, 4, 2, 0);
    }

    if west.has_opening[NORTH] {
        water(3, 1, 7, 4, 2, 7);
    }

    if west.has_opening[WEST] {
        water(0, 1, 3, 0, 2, 4);
    }

    if east.has_opening[SOUTH] {
        water(11, 1, 0, 12, 2, 0);
    }

    if east.has_opening[NORTH] {
        water(11, 1, 7, 12, 2, 7);
    }

    if east.has_opening[EAST] {
        water(15, 1, 3, 15, 2, 4);
    }

    if west_up.has_opening[SOUTH] {
        water(3, 5, 0, 4, 6, 0);
    }

    if west_up.has_opening[NORTH] {
        water(3, 5, 7, 4, 6, 7);
    }

    if west_up.has_opening[WEST] {
        water(0, 5, 3, 0, 6, 4);
    }

    if east_up.has_opening[SOUTH] {
        water(11, 5, 0, 12, 6, 0);
    }

    if east_up.has_opening[NORTH] {
        water(11, 5, 7, 12, 6, 7);
    }

    if east_up.has_opening[EAST] {
        water(15, 5, 3, 15, 6, 4);
    }
}

fn double_y_room(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    chunk_bb: BoundingBox,
    rooms: &[RoomDefinition],
    room: usize,
) {
    let gray = base_gray();
    let light = base_light();
    let gbox = |x0, y0, z0, x1, y1, z1, state| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, false,
        );
    };

    if rooms[room].index / GRID_FLOOR_COUNT > 0 {
        generate_default_floor(base, region, chunk_bb, 0, 0, rooms[room].has_opening[DOWN]);
    }

    let above = connected(rooms, room, UP);
    if above.connections[UP].is_none() {
        generate_box_on_fill_only(base, region, chunk_bb, 1, 8, 1, 6, 8, 6, gray);
    }

    gbox(0, 4, 0, 0, 4, 7, light);
    gbox(7, 4, 0, 7, 4, 7, light);
    gbox(1, 4, 0, 6, 4, 0, light);
    gbox(1, 4, 7, 6, 4, 7, light);
    gbox(2, 4, 1, 2, 4, 2, light);
    gbox(1, 4, 2, 1, 4, 2, light);
    gbox(5, 4, 1, 5, 4, 2, light);
    gbox(6, 4, 2, 6, 4, 2, light);
    gbox(2, 4, 5, 2, 4, 6, light);
    gbox(1, 4, 5, 1, 4, 5, light);
    gbox(5, 4, 5, 5, 4, 6, light);
    gbox(6, 4, 5, 6, 4, 5, light);

    let mut definition = &rooms[room];

    let mut y = 1;
    while y <= 5 {
        let z = 0;
        if definition.has_opening[SOUTH] {
            gbox(2, y, z, 2, y + 2, z, light);
            gbox(5, y, z, 5, y + 2, z, light);
            gbox(3, y + 2, z, 4, y + 2, z, light);
        } else {
            gbox(0, y, z, 7, y + 2, z, light);
            gbox(0, y + 1, z, 7, y + 1, z, gray);
        }

        let z = 7;
        if definition.has_opening[NORTH] {
            gbox(2, y, z, 2, y + 2, z, light);
            gbox(5, y, z, 5, y + 2, z, light);
            gbox(3, y + 2, z, 4, y + 2, z, light);
        } else {
            gbox(0, y, z, 7, y + 2, z, light);
            gbox(0, y + 1, z, 7, y + 1, z, gray);
        }

        let x = 0;
        if definition.has_opening[WEST] {
            gbox(x, y, 2, x, y + 2, 2, light);
            gbox(x, y, 5, x, y + 2, 5, light);
            gbox(x, y + 2, 3, x, y + 2, 4, light);
        } else {
            gbox(x, y, 0, x, y + 2, 7, light);
            gbox(x, y + 1, 0, x, y + 1, 7, gray);
        }

        let x = 7;
        if definition.has_opening[EAST] {
            gbox(x, y, 2, x, y + 2, 2, light);
            gbox(x, y, 5, x, y + 2, 5, light);
            gbox(x, y + 2, 3, x, y + 2, 4, light);
        } else {
            gbox(x, y, 0, x, y + 2, 7, light);
            gbox(x, y + 1, 0, x, y + 1, 7, gray);
        }

        definition = above;
        y += 4;
    }
}

fn double_yz_room(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    chunk_bb: BoundingBox,
    rooms: &[RoomDefinition],
    room: usize,
) {
    let gray = base_gray();
    let light = base_light();
    let black = base_black();
    let lamp = lamp_block();
    let gbox = |x0, y0, z0, x1, y1, z1, state| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, false,
        );
    };
    let water = |x0, y0, z0, x1, y1, z1| {
        generate_water_box(base, region, chunk_bb, x0, y0, z0, x1, y1, z1);
    };

    let north_index = connection(rooms, room, NORTH);
    let north = &rooms[north_index];
    let south = &rooms[room];
    let north_up = connected(rooms, north_index, UP);
    let south_up = connected(rooms, room, UP);

    if south.index / GRID_FLOOR_COUNT > 0 {
        generate_default_floor(base, region, chunk_bb, 0, 8, north.has_opening[DOWN]);
        generate_default_floor(base, region, chunk_bb, 0, 0, south.has_opening[DOWN]);
    }

    if south_up.connections[UP].is_none() {
        generate_box_on_fill_only(base, region, chunk_bb, 1, 8, 1, 6, 8, 7, gray);
    }

    if north_up.connections[UP].is_none() {
        generate_box_on_fill_only(base, region, chunk_bb, 1, 8, 8, 6, 8, 14, gray);
    }

    for y in 1..=7 {
        let block = if y == 2 || y == 6 { gray } else { light };
        gbox(0, y, 0, 0, y, 15, block);
        gbox(7, y, 0, 7, y, 15, block);
        gbox(1, y, 0, 6, y, 0, block);
        gbox(1, y, 15, 6, y, 15, block);
    }

    for y in 1..=7 {
        let block = if y == 2 || y == 6 { lamp } else { black };
        gbox(3, y, 7, 4, y, 8, block);
    }

    if south.has_opening[SOUTH] {
        water(3, 1, 0, 4, 2, 0);
    }

    if south.has_opening[EAST] {
        water(7, 1, 3, 7, 2, 4);
    }

    if south.has_opening[WEST] {
        water(0, 1, 3, 0, 2, 4);
    }

    if north.has_opening[NORTH] {
        water(3, 1, 15, 4, 2, 15);
    }

    if north.has_opening[WEST] {
        water(0, 1, 11, 0, 2, 12);
    }

    if north.has_opening[EAST] {
        water(7, 1, 11, 7, 2, 12);
    }

    if south_up.has_opening[SOUTH] {
        water(3, 5, 0, 4, 6, 0);
    }

    if south_up.has_opening[EAST] {
        water(7, 5, 3, 7, 6, 4);
        gbox(5, 4, 2, 6, 4, 5, light);
        gbox(6, 1, 2, 6, 3, 2, light);
        gbox(6, 1, 5, 6, 3, 5, light);
    }

    if south_up.has_opening[WEST] {
        water(0, 5, 3, 0, 6, 4);
        gbox(1, 4, 2, 2, 4, 5, light);
        gbox(1, 1, 2, 1, 3, 2, light);
        gbox(1, 1, 5, 1, 3, 5, light);
    }

    if north_up.has_opening[NORTH] {
        water(3, 5, 15, 4, 6, 15);
    }

    if north_up.has_opening[WEST] {
        water(0, 5, 11, 0, 6, 12);
        gbox(1, 4, 10, 2, 4, 13, light);
        gbox(1, 1, 10, 1, 3, 10, light);
        gbox(1, 1, 13, 1, 3, 13, light);
    }

    if north_up.has_opening[EAST] {
        water(7, 5, 11, 7, 6, 12);
        gbox(5, 4, 10, 6, 4, 13, light);
        gbox(6, 1, 10, 6, 3, 10, light);
        gbox(6, 1, 13, 6, 3, 13, light);
    }
}

fn double_z_room(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    chunk_bb: BoundingBox,
    rooms: &[RoomDefinition],
    room: usize,
) {
    let gray = base_gray();
    let light = base_light();
    let lamp = lamp_block();
    let gbox = |x0, y0, z0, x1, y1, z1, state| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, false,
        );
    };
    let pb = |x, y, z, state| base.place_block(region, state, x, y, z, chunk_bb);
    let water = |x0, y0, z0, x1, y1, z1| {
        generate_water_box(base, region, chunk_bb, x0, y0, z0, x1, y1, z1);
    };

    let north = connected(rooms, room, NORTH);
    let south = &rooms[room];

    if south.index / GRID_FLOOR_COUNT > 0 {
        generate_default_floor(base, region, chunk_bb, 0, 8, north.has_opening[DOWN]);
        generate_default_floor(base, region, chunk_bb, 0, 0, south.has_opening[DOWN]);
    }

    if south.connections[UP].is_none() {
        generate_box_on_fill_only(base, region, chunk_bb, 1, 4, 1, 6, 4, 7, gray);
    }

    if north.connections[UP].is_none() {
        generate_box_on_fill_only(base, region, chunk_bb, 1, 4, 8, 6, 4, 14, gray);
    }

    gbox(0, 3, 0, 0, 3, 15, light);
    gbox(7, 3, 0, 7, 3, 15, light);
    gbox(1, 3, 0, 7, 3, 0, light);
    gbox(1, 3, 15, 6, 3, 15, light);
    gbox(0, 2, 0, 0, 2, 15, gray);
    gbox(7, 2, 0, 7, 2, 15, gray);
    gbox(1, 2, 0, 7, 2, 0, gray);
    gbox(1, 2, 15, 6, 2, 15, gray);
    gbox(0, 1, 0, 0, 1, 15, light);
    gbox(7, 1, 0, 7, 1, 15, light);
    gbox(1, 1, 0, 7, 1, 0, light);
    gbox(1, 1, 15, 6, 1, 15, light);
    gbox(1, 1, 1, 1, 1, 2, light);
    gbox(6, 1, 1, 6, 1, 2, light);
    gbox(1, 3, 1, 1, 3, 2, light);
    gbox(6, 3, 1, 6, 3, 2, light);
    gbox(1, 1, 13, 1, 1, 14, light);
    gbox(6, 1, 13, 6, 1, 14, light);
    gbox(1, 3, 13, 1, 3, 14, light);
    gbox(6, 3, 13, 6, 3, 14, light);
    gbox(2, 1, 6, 2, 3, 6, light);
    gbox(5, 1, 6, 5, 3, 6, light);
    gbox(2, 1, 9, 2, 3, 9, light);
    gbox(5, 1, 9, 5, 3, 9, light);
    gbox(3, 2, 6, 4, 2, 6, light);
    gbox(3, 2, 9, 4, 2, 9, light);
    gbox(2, 2, 7, 2, 2, 8, light);
    gbox(5, 2, 7, 5, 2, 8, light);
    pb(2, 2, 5, lamp);
    pb(5, 2, 5, lamp);
    pb(2, 2, 10, lamp);
    pb(5, 2, 10, lamp);
    pb(2, 3, 5, light);
    pb(5, 3, 5, light);
    pb(2, 3, 10, light);
    pb(5, 3, 10, light);

    if south.has_opening[SOUTH] {
        water(3, 1, 0, 4, 2, 0);
    }

    if south.has_opening[EAST] {
        water(7, 1, 3, 7, 2, 4);
    }

    if south.has_opening[WEST] {
        water(0, 1, 3, 0, 2, 4);
    }

    if north.has_opening[NORTH] {
        water(3, 1, 15, 4, 2, 15);
    }

    if north.has_opening[WEST] {
        water(0, 1, 11, 0, 2, 12);
    }

    if north.has_opening[EAST] {
        water(7, 1, 11, 7, 2, 12);
    }
}

fn entry_room(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    chunk_bb: BoundingBox,
    rooms: &[RoomDefinition],
    room: usize,
) {
    let light = base_light();
    let gbox = |x0, y0, z0, x1, y1, z1, state| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, false,
        );
    };
    let water = |x0, y0, z0, x1, y1, z1| {
        generate_water_box(base, region, chunk_bb, x0, y0, z0, x1, y1, z1);
    };

    gbox(0, 3, 0, 2, 3, 7, light);
    gbox(5, 3, 0, 7, 3, 7, light);
    gbox(0, 2, 0, 1, 2, 7, light);
    gbox(6, 2, 0, 7, 2, 7, light);
    gbox(0, 1, 0, 0, 1, 7, light);
    gbox(7, 1, 0, 7, 1, 7, light);
    gbox(0, 1, 7, 7, 3, 7, light);
    gbox(1, 1, 0, 2, 3, 0, light);
    gbox(5, 1, 0, 6, 3, 0, light);

    let definition = &rooms[room];
    if definition.has_opening[NORTH] {
        water(3, 1, 7, 4, 2, 7);
    }

    if definition.has_opening[WEST] {
        water(0, 1, 3, 1, 2, 4);
    }

    if definition.has_opening[EAST] {
        water(6, 1, 3, 7, 2, 4);
    }
}

fn penthouse(base: &PieceBase, region: &WorldGenRegion<'_>, chunk_bb: BoundingBox) {
    let gray = base_gray();
    let light = base_light();
    let black = base_black();
    let lamp = lamp_block();
    let gbox = |x0, y0, z0, x1, y1, z1, state| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, false,
        );
    };
    let pb = |x, y, z, state| base.place_block(region, state, x, y, z, chunk_bb);

    gbox(2, -1, 2, 11, -1, 11, light);
    gbox(0, -1, 0, 1, -1, 11, gray);
    gbox(12, -1, 0, 13, -1, 11, gray);
    gbox(2, -1, 0, 11, -1, 1, gray);
    gbox(2, -1, 12, 11, -1, 13, gray);
    gbox(0, 0, 0, 0, 0, 13, light);
    gbox(13, 0, 0, 13, 0, 13, light);
    gbox(1, 0, 0, 12, 0, 0, light);
    gbox(1, 0, 13, 12, 0, 13, light);

    let mut i = 2;
    while i <= 11 {
        pb(0, 0, i, lamp);
        pb(13, 0, i, lamp);
        pb(i, 0, 0, lamp);
        i += 3;
    }

    gbox(2, 0, 3, 4, 0, 9, light);
    gbox(9, 0, 3, 11, 0, 9, light);
    gbox(4, 0, 9, 9, 0, 11, light);
    pb(5, 0, 8, light);
    pb(8, 0, 8, light);
    pb(10, 0, 10, light);
    pb(3, 0, 10, light);
    gbox(3, 0, 3, 3, 0, 7, black);
    gbox(10, 0, 3, 10, 0, 7, black);
    gbox(6, 0, 10, 7, 0, 10, black);

    let mut x = 3;
    for _ in 0..2 {
        let mut z = 2;
        while z <= 8 {
            gbox(x, 0, z, x, 2, z, light);
            z += 3;
        }
        x = 10;
    }

    gbox(5, 0, 10, 5, 2, 10, light);
    gbox(8, 0, 10, 8, 2, 10, light);
    gbox(6, -1, 7, 7, -1, 8, black);
    generate_water_box(base, region, chunk_bb, 6, -1, 3, 7, -1, 4);
}

fn simple_room(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    chunk_bb: BoundingBox,
    rooms: &[RoomDefinition],
    room: usize,
    main_design: i32,
) {
    let gray = base_gray();
    let light = base_light();
    let black = base_black();
    let lamp = lamp_block();
    let gbox = |x0, y0, z0, x1, y1, z1, state| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, false,
        );
    };
    let pb = |x, y, z, state| base.place_block(region, state, x, y, z, chunk_bb);
    let water = |x0, y0, z0, x1, y1, z1| {
        generate_water_box(base, region, chunk_bb, x0, y0, z0, x1, y1, z1);
    };

    let definition = &rooms[room];
    if definition.index / GRID_FLOOR_COUNT > 0 {
        generate_default_floor(base, region, chunk_bb, 0, 0, definition.has_opening[DOWN]);
    }

    if definition.connections[UP].is_none() {
        generate_box_on_fill_only(base, region, chunk_bb, 1, 4, 1, 6, 4, 6, gray);
    }

    let center_pillar = main_design != 0
        && random.next_bool()
        && !definition.has_opening[DOWN]
        && !definition.has_opening[UP]
        && definition.count_openings() > 1;

    if main_design == 0 {
        gbox(0, 1, 0, 2, 1, 2, light);
        gbox(0, 3, 0, 2, 3, 2, light);
        gbox(0, 2, 0, 0, 2, 2, gray);
        gbox(1, 2, 0, 2, 2, 0, gray);
        pb(1, 2, 1, lamp);
        gbox(5, 1, 0, 7, 1, 2, light);
        gbox(5, 3, 0, 7, 3, 2, light);
        gbox(7, 2, 0, 7, 2, 2, gray);
        gbox(5, 2, 0, 6, 2, 0, gray);
        pb(6, 2, 1, lamp);
        gbox(0, 1, 5, 2, 1, 7, light);
        gbox(0, 3, 5, 2, 3, 7, light);
        gbox(0, 2, 5, 0, 2, 7, gray);
        gbox(1, 2, 7, 2, 2, 7, gray);
        pb(1, 2, 6, lamp);
        gbox(5, 1, 5, 7, 1, 7, light);
        gbox(5, 3, 5, 7, 3, 7, light);
        gbox(7, 2, 5, 7, 2, 7, gray);
        gbox(5, 2, 7, 6, 2, 7, gray);
        pb(6, 2, 6, lamp);

        if definition.has_opening[SOUTH] {
            gbox(3, 3, 0, 4, 3, 0, light);
        } else {
            gbox(3, 3, 0, 4, 3, 1, light);
            gbox(3, 2, 0, 4, 2, 0, gray);
            gbox(3, 1, 0, 4, 1, 1, light);
        }

        if definition.has_opening[NORTH] {
            gbox(3, 3, 7, 4, 3, 7, light);
        } else {
            gbox(3, 3, 6, 4, 3, 7, light);
            gbox(3, 2, 7, 4, 2, 7, gray);
            gbox(3, 1, 6, 4, 1, 7, light);
        }

        if definition.has_opening[WEST] {
            gbox(0, 3, 3, 0, 3, 4, light);
        } else {
            gbox(0, 3, 3, 1, 3, 4, light);
            gbox(0, 2, 3, 0, 2, 4, gray);
            gbox(0, 1, 3, 1, 1, 4, light);
        }

        if definition.has_opening[EAST] {
            gbox(7, 3, 3, 7, 3, 4, light);
        } else {
            gbox(6, 3, 3, 7, 3, 4, light);
            gbox(7, 2, 3, 7, 2, 4, gray);
            gbox(6, 1, 3, 7, 1, 4, light);
        }
    } else if main_design == 1 {
        gbox(2, 1, 2, 2, 3, 2, light);
        gbox(2, 1, 5, 2, 3, 5, light);
        gbox(5, 1, 5, 5, 3, 5, light);
        gbox(5, 1, 2, 5, 3, 2, light);
        pb(2, 2, 2, lamp);
        pb(2, 2, 5, lamp);
        pb(5, 2, 5, lamp);
        pb(5, 2, 2, lamp);
        gbox(0, 1, 0, 1, 3, 0, light);
        gbox(0, 1, 1, 0, 3, 1, light);
        gbox(0, 1, 7, 1, 3, 7, light);
        gbox(0, 1, 6, 0, 3, 6, light);
        gbox(6, 1, 7, 7, 3, 7, light);
        gbox(7, 1, 6, 7, 3, 6, light);
        gbox(6, 1, 0, 7, 3, 0, light);
        gbox(7, 1, 1, 7, 3, 1, light);
        pb(1, 2, 0, gray);
        pb(0, 2, 1, gray);
        pb(1, 2, 7, gray);
        pb(0, 2, 6, gray);
        pb(6, 2, 7, gray);
        pb(7, 2, 6, gray);
        pb(6, 2, 0, gray);
        pb(7, 2, 1, gray);

        if !definition.has_opening[SOUTH] {
            gbox(1, 3, 0, 6, 3, 0, light);
            gbox(1, 2, 0, 6, 2, 0, gray);
            gbox(1, 1, 0, 6, 1, 0, light);
        }

        if !definition.has_opening[NORTH] {
            gbox(1, 3, 7, 6, 3, 7, light);
            gbox(1, 2, 7, 6, 2, 7, gray);
            gbox(1, 1, 7, 6, 1, 7, light);
        }

        if !definition.has_opening[WEST] {
            gbox(0, 3, 1, 0, 3, 6, light);
            gbox(0, 2, 1, 0, 2, 6, gray);
            gbox(0, 1, 1, 0, 1, 6, light);
        }

        if !definition.has_opening[EAST] {
            gbox(7, 3, 1, 7, 3, 6, light);
            gbox(7, 2, 1, 7, 2, 6, gray);
            gbox(7, 1, 1, 7, 1, 6, light);
        }
    } else if main_design == 2 {
        gbox(0, 1, 0, 0, 1, 7, light);
        gbox(7, 1, 0, 7, 1, 7, light);
        gbox(1, 1, 0, 6, 1, 0, light);
        gbox(1, 1, 7, 6, 1, 7, light);
        gbox(0, 2, 0, 0, 2, 7, black);
        gbox(7, 2, 0, 7, 2, 7, black);
        gbox(1, 2, 0, 6, 2, 0, black);
        gbox(1, 2, 7, 6, 2, 7, black);
        gbox(0, 3, 0, 0, 3, 7, light);
        gbox(7, 3, 0, 7, 3, 7, light);
        gbox(1, 3, 0, 6, 3, 0, light);
        gbox(1, 3, 7, 6, 3, 7, light);
        gbox(0, 1, 3, 0, 2, 4, black);
        gbox(7, 1, 3, 7, 2, 4, black);
        gbox(3, 1, 0, 4, 2, 0, black);
        gbox(3, 1, 7, 4, 2, 7, black);

        if definition.has_opening[SOUTH] {
            water(3, 1, 0, 4, 2, 0);
        }

        if definition.has_opening[NORTH] {
            water(3, 1, 7, 4, 2, 7);
        }

        if definition.has_opening[WEST] {
            water(0, 1, 3, 0, 2, 4);
        }

        if definition.has_opening[EAST] {
            water(7, 1, 3, 7, 2, 4);
        }
    }

    if center_pillar {
        gbox(3, 1, 3, 4, 1, 4, light);
        gbox(3, 2, 3, 4, 2, 4, gray);
        gbox(3, 3, 3, 4, 3, 4, light);
    }
}

fn simple_top_room(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    chunk_bb: BoundingBox,
    rooms: &[RoomDefinition],
    room: usize,
) {
    let gray = base_gray();
    let light = base_light();
    let black = base_black();
    let gbox = |x0, y0, z0, x1, y1, z1, state| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, false,
        );
    };
    let water = |x0, y0, z0, x1, y1, z1| {
        generate_water_box(base, region, chunk_bb, x0, y0, z0, x1, y1, z1);
    };

    let definition = &rooms[room];
    if definition.index / GRID_FLOOR_COUNT > 0 {
        generate_default_floor(base, region, chunk_bb, 0, 0, definition.has_opening[DOWN]);
    }

    if definition.connections[UP].is_none() {
        generate_box_on_fill_only(base, region, chunk_bb, 1, 4, 1, 6, 4, 6, gray);
    }

    for x in 1..=6 {
        for z in 1..=6 {
            if random.next_i32_bounded(3) != 0 {
                let y0 = 2 + i32::from(random.next_i32_bounded(4) != 0);
                let wet_sponge = vanilla_blocks::WET_SPONGE.default_state();
                gbox(x, y0, z, x, 3, z, wet_sponge);
            }
        }
    }

    gbox(0, 1, 0, 0, 1, 7, light);
    gbox(7, 1, 0, 7, 1, 7, light);
    gbox(1, 1, 0, 6, 1, 0, light);
    gbox(1, 1, 7, 6, 1, 7, light);
    gbox(0, 2, 0, 0, 2, 7, black);
    gbox(7, 2, 0, 7, 2, 7, black);
    gbox(1, 2, 0, 6, 2, 0, black);
    gbox(1, 2, 7, 6, 2, 7, black);
    gbox(0, 3, 0, 0, 3, 7, light);
    gbox(7, 3, 0, 7, 3, 7, light);
    gbox(1, 3, 0, 6, 3, 0, light);
    gbox(1, 3, 7, 6, 3, 7, light);
    gbox(0, 1, 3, 0, 2, 4, black);
    gbox(7, 1, 3, 7, 2, 4, black);
    gbox(3, 1, 0, 4, 2, 0, black);
    gbox(3, 1, 7, 4, 2, 7, black);

    if definition.has_opening[SOUTH] {
        water(3, 1, 0, 4, 2, 0);
    }
}

fn wing_room(
    base: &PieceBase,
    region: &WorldGenRegion<'_>,
    chunk_bb: BoundingBox,
    main_design: i32,
) {
    let light = base_light();
    let black = base_black();
    let lamp = lamp_block();
    let gbox = |x0, y0, z0, x1, y1, z1, state| {
        base.generate_box(
            region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, false,
        );
    };
    let pb = |x, y, z, state| base.place_block(region, state, x, y, z, chunk_bb);

    if main_design == 0 {
        for i in 0..4 {
            gbox(10 - i, 3 - i, 20 - i, 12 + i, 3 - i, 20, light);
        }

        gbox(7, 0, 6, 15, 0, 16, light);
        gbox(6, 0, 6, 6, 3, 20, light);
        gbox(16, 0, 6, 16, 3, 20, light);
        gbox(7, 1, 7, 7, 1, 20, light);
        gbox(15, 1, 7, 15, 1, 20, light);
        gbox(7, 1, 6, 9, 3, 6, light);
        gbox(13, 1, 6, 15, 3, 6, light);
        gbox(8, 1, 7, 9, 1, 7, light);
        gbox(13, 1, 7, 14, 1, 7, light);
        gbox(9, 0, 5, 13, 0, 5, light);
        gbox(10, 0, 7, 12, 0, 7, black);
        gbox(8, 0, 10, 8, 0, 12, black);
        gbox(14, 0, 10, 14, 0, 12, black);

        let mut z = 18;
        while z >= 7 {
            pb(6, 3, z, lamp);
            pb(16, 3, z, lamp);
            z -= 3;
        }

        pb(10, 0, 10, lamp);
        pb(12, 0, 10, lamp);
        pb(10, 0, 12, lamp);
        pb(12, 0, 12, lamp);
        pb(8, 3, 6, lamp);
        pb(14, 3, 6, lamp);
        pb(4, 2, 4, light);
        pb(4, 1, 4, lamp);
        pb(4, 0, 4, light);
        pb(18, 2, 4, light);
        pb(18, 1, 4, lamp);
        pb(18, 0, 4, light);
        pb(4, 2, 18, light);
        pb(4, 1, 18, lamp);
        pb(4, 0, 18, light);
        pb(18, 2, 18, light);
        pb(18, 1, 18, lamp);
        pb(18, 0, 18, light);
        pb(9, 7, 20, light);
        pb(13, 7, 20, light);
        gbox(6, 0, 21, 7, 4, 21, light);
        gbox(15, 0, 21, 16, 4, 21, light);
    } else if main_design == 1 {
        gbox(9, 3, 18, 13, 3, 20, light);
        gbox(9, 0, 18, 9, 2, 18, light);
        gbox(13, 0, 18, 13, 2, 18, light);

        let mut x = 9;
        for _ in 0..2 {
            pb(x, 6, 20, light);
            pb(x, 5, 20, lamp);
            pb(x, 4, 20, light);
            x = 13;
        }

        gbox(7, 3, 7, 15, 3, 14, light);

        let mut x = 10;
        for _ in 0..2 {
            gbox(x, 0, 10, x, 6, 10, light);
            gbox(x, 0, 12, x, 6, 12, light);
            pb(x, 0, 10, lamp);
            pb(x, 0, 12, lamp);
            pb(x, 4, 10, lamp);
            pb(x, 4, 12, lamp);
            x = 12;
        }

        let mut x = 8;
        for _ in 0..2 {
            gbox(x, 0, 7, x, 2, 7, light);
            gbox(x, 0, 14, x, 2, 14, light);
            x = 14;
        }

        gbox(8, 3, 8, 8, 3, 13, black);
        gbox(14, 3, 8, 14, 3, 13, black);
    }
}
