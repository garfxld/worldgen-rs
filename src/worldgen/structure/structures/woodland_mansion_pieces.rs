use crate::registry::blocks::properties::Properties;
use crate::registry::blocks::vanilla_blocks;
use crate::util::direction::Direction;
use crate::util::geometry::BoundingBox;
use crate::util::ident::Identifier;
use crate::util::mirror::Mirror;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::util::rotation::Rotation;
use crate::worldgen::block_predicate::HolderSet;
use crate::worldgen::region::WorldGenRegion;
use crate::worldgen::structure::structure_piece::{
    PieceBase, PieceKind, StructurePiece, create_chest_at,
};
use crate::worldgen::structure::template_structure_piece::TemplateStructurePiece;
use crate::worldgen::structure::templatesystem::structure_place_settings::StructurePlaceSettings;
use crate::worldgen::structure::templatesystem::structure_processor::StructureProcessor;
use crate::worldgen::structure::templatesystem::structure_template::zero_position_with_transform;

const CLEAR: i32 = 0;
const CORRIDOR: i32 = 1;
const ROOM: i32 = 2;
const START_ROOM: i32 = 3;
const BLOCKED: i32 = 5;
const ROOM_1X1: i32 = 65536;
const ROOM_1X2: i32 = 131_072;
const ROOM_2X2: i32 = 262_144;
const ROOM_ORIGIN_FLAG: i32 = 1_048_576;
const ROOM_DOOR_FLAG: i32 = 2_097_152;
const ROOM_STAIRS_FLAG: i32 = 4_194_304;
const ROOM_CORRIDOR_FLAG: i32 = 8_388_608;
const ROOM_TYPE_MASK: i32 = 983_040;
const ROOM_ID_MASK: i32 = 65535;

pub struct SimpleGrid {
    grid: Vec<i32>,
    width: i32,
    height: i32,
    value_if_outside: i32,
}

impl SimpleGrid {
    fn new(width: i32, height: i32, value_if_outside: i32) -> Self {
        Self {
            grid: vec![0; (width * height) as usize],
            width,
            height,
            value_if_outside,
        }
    }

    fn set(&mut self, x: i32, y: i32, value: i32) {
        if x >= 0 && x < self.width && y >= 0 && y < self.height {
            self.grid[(x * self.height + y) as usize] = value;
        }
    }

    fn set_area(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, value: i32) {
        for y in y0..=y1 {
            for x in x0..=x1 {
                self.set(x, y, value);
            }
        }
    }

    fn get(&self, x: i32, y: i32) -> i32 {
        if x >= 0 && x < self.width && y >= 0 && y < self.height {
            self.grid[(x * self.height + y) as usize]
        } else {
            self.value_if_outside
        }
    }

    fn set_if(&mut self, x: i32, y: i32, if_value: i32, value: i32) {
        if self.get(x, y) == if_value {
            self.set(x, y, value);
        }
    }

    fn edges_to(&self, x: i32, y: i32, if_value: i32) -> bool {
        self.get(x - 1, y) == if_value
            || self.get(x + 1, y) == if_value
            || self.get(x, y + 1) == if_value
            || self.get(x, y - 1) == if_value
    }
}

pub struct MansionGrid {
    base_grid: SimpleGrid,
    third_floor_grid: SimpleGrid,
    floor_rooms: [SimpleGrid; 3],
    entrance_x: i32,
    entrance_y: i32,
}

fn is_house(grid: &SimpleGrid, x: i32, y: i32) -> bool {
    let value = grid.get(x, y);
    value == CORRIDOR || value == ROOM || value == START_ROOM || value == 4
}

impl MansionGrid {
    fn new(random: &mut RandomSource) -> Self {
        let entrance_x = 7;
        let entrance_y = 4;
        let mut base_grid = SimpleGrid::new(11, 11, BLOCKED);
        base_grid.set_area(
            entrance_x,
            entrance_y,
            entrance_x + 1,
            entrance_y + 1,
            START_ROOM,
        );
        base_grid.set_area(entrance_x - 1, entrance_y, entrance_x - 1, entrance_y + 1, ROOM);
        base_grid.set_area(
            entrance_x + 2,
            entrance_y - 2,
            entrance_x + 3,
            entrance_y + 3,
            BLOCKED,
        );
        base_grid.set_area(
            entrance_x + 1,
            entrance_y - 2,
            entrance_x + 1,
            entrance_y - 1,
            CORRIDOR,
        );
        base_grid.set_area(
            entrance_x + 1,
            entrance_y + 2,
            entrance_x + 1,
            entrance_y + 3,
            CORRIDOR,
        );
        base_grid.set(entrance_x - 1, entrance_y - 1, CORRIDOR);
        base_grid.set(entrance_x - 1, entrance_y + 2, CORRIDOR);
        base_grid.set_area(0, 0, 11, 1, BLOCKED);
        base_grid.set_area(0, 9, 11, 11, BLOCKED);
        recursive_corridor(
            &mut base_grid,
            random,
            entrance_x,
            entrance_y - 2,
            Direction::West,
            6,
        );
        recursive_corridor(
            &mut base_grid,
            random,
            entrance_x,
            entrance_y + 3,
            Direction::West,
            6,
        );
        recursive_corridor(
            &mut base_grid,
            random,
            entrance_x - 2,
            entrance_y - 1,
            Direction::West,
            3,
        );
        recursive_corridor(
            &mut base_grid,
            random,
            entrance_x - 2,
            entrance_y + 2,
            Direction::West,
            3,
        );

        while clean_edges(&mut base_grid) {}

        let mut floor_rooms = [
            SimpleGrid::new(11, 11, BLOCKED),
            SimpleGrid::new(11, 11, BLOCKED),
            SimpleGrid::new(11, 11, BLOCKED),
        ];
        identify_rooms(&base_grid, &mut floor_rooms[0], random);
        identify_rooms(&base_grid, &mut floor_rooms[1], random);
        floor_rooms[0].set_area(
            entrance_x + 1,
            entrance_y,
            entrance_x + 1,
            entrance_y + 1,
            ROOM_CORRIDOR_FLAG,
        );
        floor_rooms[1].set_area(
            entrance_x + 1,
            entrance_y,
            entrance_x + 1,
            entrance_y + 1,
            ROOM_CORRIDOR_FLAG,
        );
        let third_floor_grid = SimpleGrid::new(base_grid.width, base_grid.height, BLOCKED);

        let mut grid = Self {
            base_grid,
            third_floor_grid,
            floor_rooms,
            entrance_x,
            entrance_y,
        };
        grid.setup_third_floor(random);
        let Self {
            third_floor_grid,
            floor_rooms,
            ..
        } = &mut grid;
        identify_rooms(third_floor_grid, &mut floor_rooms[2], random);
        grid
    }

    fn is_room_id(&self, x: i32, y: i32, floor: usize, room_id: i32) -> bool {
        (self.floor_rooms[floor].get(x, y) & ROOM_ID_MASK) == room_id
    }

    fn get_1x2_room_direction(
        &self,
        x: i32,
        y: i32,
        floor_num: usize,
        room_id: i32,
    ) -> Option<Direction> {
        for direction in Direction::HORIZONTAL {
            let (dx, dz) = direction.offset_xz();
            if self.is_room_id(x + dx, y + dz, floor_num, room_id) {
                return Some(direction);
            }
        }
        None
    }

    fn setup_third_floor(&mut self, random: &mut RandomSource) {
        let mut potential_rooms = Vec::new();
        for y in 0..self.third_floor_grid.height {
            for x in 0..self.third_floor_grid.width {
                let room_data = self.floor_rooms[1].get(x, y);
                let room_type = room_data & ROOM_TYPE_MASK;
                if room_type == ROOM_1X2 && (room_data & ROOM_DOOR_FLAG) == ROOM_DOOR_FLAG {
                    potential_rooms.push((x, y));
                }
            }
        }

        if potential_rooms.is_empty() {
            let (w, h) = (self.third_floor_grid.width, self.third_floor_grid.height);
            self.third_floor_grid.set_area(0, 0, w, h, BLOCKED);
            return;
        }

        let index = random.next_i32_bounded(potential_rooms.len() as i32) as usize;
        let (room_x, room_y) = potential_rooms[index];
        let room_data = self.floor_rooms[1].get(room_x, room_y);
        self.floor_rooms[1].set(room_x, room_y, room_data | ROOM_STAIRS_FLAG);
        let room_dir = self
            .get_1x2_room_direction(room_x, room_y, 1, room_data & ROOM_ID_MASK)
            .expect("1x2 room must have a partner cell");
        let (ddx, ddz) = room_dir.offset_xz();
        let room_end_x = room_x + ddx;
        let room_end_y = room_y + ddz;

        for y in 0..self.third_floor_grid.height {
            for x in 0..self.third_floor_grid.width {
                if !is_house(&self.base_grid, x, y) {
                    self.third_floor_grid.set(x, y, BLOCKED);
                } else if x == room_x && y == room_y {
                    self.third_floor_grid.set(x, y, START_ROOM);
                } else if x == room_end_x && y == room_end_y {
                    self.third_floor_grid.set(x, y, START_ROOM);
                    self.floor_rooms[2].set(x, y, ROOM_CORRIDOR_FLAG);
                }
            }
        }

        let mut potential_corridors = Vec::new();
        for direction in Direction::HORIZONTAL {
            let (dx, dz) = direction.offset_xz();
            if self.third_floor_grid.get(room_end_x + dx, room_end_y + dz) == CLEAR {
                potential_corridors.push(direction);
            }
        }

        if potential_corridors.is_empty() {
            let (w, h) = (self.third_floor_grid.width, self.third_floor_grid.height);
            self.third_floor_grid.set_area(0, 0, w, h, BLOCKED);
            self.floor_rooms[1].set(room_x, room_y, room_data);
            return;
        }

        let index = random.next_i32_bounded(potential_corridors.len() as i32) as usize;
        let corridor_dir = potential_corridors[index];
        let (cdx, cdz) = corridor_dir.offset_xz();
        recursive_corridor(
            &mut self.third_floor_grid,
            random,
            room_end_x + cdx,
            room_end_y + cdz,
            corridor_dir,
            4,
        );
        while clean_edges(&mut self.third_floor_grid) {}
    }
}

fn recursive_corridor(
    grid: &mut SimpleGrid,
    random: &mut RandomSource,
    x: i32,
    y: i32,
    heading: Direction,
    depth: i32,
) {
    if depth <= 0 {
        return;
    }
    let (hx, hz) = heading.offset_xz();
    grid.set(x, y, CORRIDOR);
    grid.set_if(x + hx, y + hz, CLEAR, CORRIDOR);

    for _ in 0..8 {
        let next_dir = Direction::from_2d_data_value(random.next_i32_bounded(4));
        if next_dir != heading.opposite()
            && (next_dir != Direction::East || !random.next_bool())
        {
            let nx = x + hx;
            let ny = y + hz;
            let (ndx, ndz) = next_dir.offset_xz();
            if grid.get(nx + ndx, ny + ndz) == CLEAR
                && grid.get(nx + ndx * 2, ny + ndz * 2) == CLEAR
            {
                recursive_corridor(grid, random, x + hx + ndx, y + hz + ndz, next_dir, depth - 1);
                break;
            }
        }
    }

    let cw = heading.rotate_y_clockwise();
    let ccw = heading.rotate_y_counter_clockwise();
    let (cwx, cwz) = cw.offset_xz();
    let (ccwx, ccwz) = ccw.offset_xz();
    grid.set_if(x + cwx, y + cwz, CLEAR, ROOM);
    grid.set_if(x + ccwx, y + ccwz, CLEAR, ROOM);
    grid.set_if(x + hx + cwx, y + hz + cwz, CLEAR, ROOM);
    grid.set_if(x + hx + ccwx, y + hz + ccwz, CLEAR, ROOM);
    grid.set_if(x + hx * 2, y + hz * 2, CLEAR, ROOM);
    grid.set_if(x + cwx * 2, y + cwz * 2, CLEAR, ROOM);
    grid.set_if(x + ccwx * 2, y + ccwz * 2, CLEAR, ROOM);
}

fn clean_edges(grid: &mut SimpleGrid) -> bool {
    let mut touched = false;
    for y in 0..grid.height {
        for x in 0..grid.width {
            if grid.get(x, y) != CLEAR {
                continue;
            }
            let mut direct_neighbors = 0;
            direct_neighbors += i32::from(is_house(grid, x + 1, y));
            direct_neighbors += i32::from(is_house(grid, x - 1, y));
            direct_neighbors += i32::from(is_house(grid, x, y + 1));
            direct_neighbors += i32::from(is_house(grid, x, y - 1));
            if direct_neighbors >= 3 {
                grid.set(x, y, ROOM);
                touched = true;
            } else if direct_neighbors == 2 {
                let mut diagonal_neighbors = 0;
                diagonal_neighbors += i32::from(is_house(grid, x + 1, y + 1));
                diagonal_neighbors += i32::from(is_house(grid, x - 1, y + 1));
                diagonal_neighbors += i32::from(is_house(grid, x + 1, y - 1));
                diagonal_neighbors += i32::from(is_house(grid, x - 1, y - 1));
                if diagonal_neighbors <= 1 {
                    grid.set(x, y, ROOM);
                    touched = true;
                }
            }
        }
    }
    touched
}

#[expect(
    clippy::too_many_lines,
    reason = "mirrors vanilla MansionGrid::identifyRooms"
)]
fn identify_rooms(from_grid: &SimpleGrid, room_grid: &mut SimpleGrid, random: &mut RandomSource) {
    let mut room_pos = Vec::new();
    for y in 0..from_grid.height {
        for x in 0..from_grid.width {
            if from_grid.get(x, y) == ROOM {
                room_pos.push((x, y));
            }
        }
    }

    for i in (2..=room_pos.len()).rev() {
        let swap_to = random.next_i32_bounded(i as i32) as usize;
        room_pos.swap(i - 1, swap_to);
    }

    let mut room_id = 10;
    for (x, y) in room_pos {
        if room_grid.get(x, y) != CLEAR {
            continue;
        }
        let mut x0 = x;
        let mut x1 = x;
        let mut y0 = y;
        let mut y1 = y;
        let mut room_type = ROOM_1X1;
        if room_grid.get(x + 1, y) == CLEAR
            && room_grid.get(x, y + 1) == CLEAR
            && room_grid.get(x + 1, y + 1) == CLEAR
            && from_grid.get(x + 1, y) == ROOM
            && from_grid.get(x, y + 1) == ROOM
            && from_grid.get(x + 1, y + 1) == ROOM
        {
            x1 += 1;
            y1 += 1;
            room_type = ROOM_2X2;
        } else if room_grid.get(x - 1, y) == CLEAR
            && room_grid.get(x, y + 1) == CLEAR
            && room_grid.get(x - 1, y + 1) == CLEAR
            && from_grid.get(x - 1, y) == ROOM
            && from_grid.get(x, y + 1) == ROOM
            && from_grid.get(x - 1, y + 1) == ROOM
        {
            x0 -= 1;
            y1 += 1;
            room_type = ROOM_2X2;
        } else if room_grid.get(x - 1, y) == CLEAR
            && room_grid.get(x, y - 1) == CLEAR
            && room_grid.get(x - 1, y - 1) == CLEAR
            && from_grid.get(x - 1, y) == ROOM
            && from_grid.get(x, y - 1) == ROOM
            && from_grid.get(x - 1, y - 1) == ROOM
        {
            x0 -= 1;
            y0 -= 1;
            room_type = ROOM_2X2;
        } else if room_grid.get(x + 1, y) == CLEAR && from_grid.get(x + 1, y) == ROOM {
            x1 += 1;
            room_type = ROOM_1X2;
        } else if room_grid.get(x, y + 1) == CLEAR && from_grid.get(x, y + 1) == ROOM {
            y1 += 1;
            room_type = ROOM_1X2;
        } else if room_grid.get(x - 1, y) == CLEAR && from_grid.get(x - 1, y) == ROOM {
            x0 -= 1;
            room_type = ROOM_1X2;
        } else if room_grid.get(x, y - 1) == CLEAR && from_grid.get(x, y - 1) == ROOM {
            y0 -= 1;
            room_type = ROOM_1X2;
        }

        let mut door_x = if random.next_bool() { x0 } else { x1 };
        let mut door_y = if random.next_bool() { y0 } else { y1 };
        let mut door_flag = ROOM_DOOR_FLAG;
        if !from_grid.edges_to(door_x, door_y, CORRIDOR) {
            door_x = if door_x == x0 { x1 } else { x0 };
            door_y = if door_y == y0 { y1 } else { y0 };
            if !from_grid.edges_to(door_x, door_y, CORRIDOR) {
                door_y = if door_y == y0 { y1 } else { y0 };
                if !from_grid.edges_to(door_x, door_y, CORRIDOR) {
                    door_x = if door_x == x0 { x1 } else { x0 };
                    door_y = if door_y == y0 { y1 } else { y0 };
                    if !from_grid.edges_to(door_x, door_y, CORRIDOR) {
                        door_flag = 0;
                        door_x = x0;
                        door_y = y0;
                    }
                }
            }
        }

        for ry in y0..=y1 {
            for rx in x0..=x1 {
                if rx == door_x && ry == door_y {
                    room_grid.set(rx, ry, ROOM_ORIGIN_FLAG | door_flag | room_type | room_id);
                } else {
                    room_grid.set(rx, ry, room_type | room_id);
                }
            }
        }

        room_id += 1;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RoomCollection {
    FirstFloor,
    UpperFloor,
}

impl RoomCollection {
    fn get_1x1(self, random: &mut RandomSource) -> String {
        match self {
            Self::FirstFloor => format!("1x1_a{}", random.next_i32_bounded(5) + 1),
            Self::UpperFloor => format!("1x1_b{}", random.next_i32_bounded(5) + 1),
        }
    }

    fn get_1x1_secret(self, random: &mut RandomSource) -> String {
        format!("1x1_as{}", random.next_i32_bounded(4) + 1)
    }

    fn get_1x2_side_entrance(self, random: &mut RandomSource, is_stairs_room: bool) -> String {
        match self {
            Self::FirstFloor => format!("1x2_a{}", random.next_i32_bounded(9) + 1),
            Self::UpperFloor => {
                if is_stairs_room {
                    "1x2_c_stairs".to_owned()
                } else {
                    format!("1x2_c{}", random.next_i32_bounded(4) + 1)
                }
            }
        }
    }

    fn get_1x2_front_entrance(self, random: &mut RandomSource, is_stairs_room: bool) -> String {
        match self {
            Self::FirstFloor => format!("1x2_b{}", random.next_i32_bounded(5) + 1),
            Self::UpperFloor => {
                if is_stairs_room {
                    "1x2_d_stairs".to_owned()
                } else {
                    format!("1x2_d{}", random.next_i32_bounded(5) + 1)
                }
            }
        }
    }

    fn get_1x2_secret(self, random: &mut RandomSource) -> String {
        match self {
            Self::FirstFloor => format!("1x2_s{}", random.next_i32_bounded(2) + 1),
            Self::UpperFloor => format!("1x2_se{}", random.next_i32_bounded(1) + 1),
        }
    }

    fn get_2x2(self, random: &mut RandomSource) -> String {
        match self {
            Self::FirstFloor => format!("2x2_a{}", random.next_i32_bounded(4) + 1),
            Self::UpperFloor => format!("2x2_b{}", random.next_i32_bounded(5) + 1),
        }
    }

    fn get_2x2_secret(self, _random: &mut RandomSource) -> String {
        "2x2_s1".to_owned()
    }
}

struct PlacementData {
    rotation: Rotation,
    position: BlockPos,
    wall_type: &'static str,
}

struct MansionPiecePlacer {
    start_x: i32,
    start_y: i32,
}

pub fn generate_mansion(
    origin: BlockPos,
    rotation: Rotation,
    pieces: &mut Vec<StructurePiece>,
    random: &mut RandomSource,
) {
    let grid = MansionGrid::new(random);
    let mut placer = MansionPiecePlacer {
        start_x: 0,
        start_y: 0,
    };
    placer.create_mansion(origin, rotation, pieces, &grid, random);
}

impl MansionPiecePlacer {
    #[expect(
        clippy::too_many_lines,
        reason = "mirrors vanilla MansionPiecePlacer::createMansion"
    )]
    fn create_mansion(
        &mut self,
        origin: BlockPos,
        rotation: Rotation,
        pieces: &mut Vec<StructurePiece>,
        mansion: &MansionGrid,
        random: &mut RandomSource,
    ) {
        let mut data = PlacementData {
            rotation,
            position: origin,
            wall_type: "wall_flat",
        };
        self.entrance(pieces, &mut data);
        let mut second_data = PlacementData {
            rotation: data.rotation,
            position: data.position.above_n(8),
            wall_type: "wall_window",
        };

        self.start_x = mansion.entrance_x + 1;
        self.start_y = mansion.entrance_y + 1;
        let end_x = mansion.entrance_x + 1;
        let end_y = mansion.entrance_y;
        let (start_x, start_y) = (self.start_x, self.start_y);
        self.traverse_outer_walls(
            pieces,
            &mut data,
            &mansion.base_grid,
            Direction::South,
            start_x,
            start_y,
            end_x,
            end_y,
        );
        self.traverse_outer_walls(
            pieces,
            &mut second_data,
            &mansion.base_grid,
            Direction::South,
            start_x,
            start_y,
            end_x,
            end_y,
        );

        let mut third_data = PlacementData {
            rotation: data.rotation,
            position: data.position.above_n(19),
            wall_type: "wall_window",
        };
        let mut done = false;
        for y in 0..mansion.third_floor_grid.height {
            if done {
                break;
            }
            for x in (0..mansion.third_floor_grid.width).rev() {
                if done {
                    break;
                }
                if is_house(&mansion.third_floor_grid, x, y) {
                    third_data.position = third_data.position.relative_n(
                        rotation.rotate(Direction::South),
                        8 + (y - self.start_y) * 8,
                    );
                    third_data.position = third_data
                        .position
                        .relative_n(rotation.rotate(Direction::East), (x - self.start_x) * 8);
                    self.traverse_wall_piece(pieces, &mut third_data);
                    self.traverse_outer_walls(
                        pieces,
                        &mut third_data,
                        &mansion.third_floor_grid,
                        Direction::South,
                        x,
                        y,
                        x,
                        y,
                    );
                    done = true;
                }
            }
        }

        self.create_roof(
            pieces,
            origin.above_n(16),
            rotation,
            &mansion.base_grid,
            Some(&mansion.third_floor_grid),
        );
        self.create_roof(
            pieces,
            origin.above_n(27),
            rotation,
            &mansion.third_floor_grid,
            None,
        );

        let room_collections = [
            RoomCollection::FirstFloor,
            RoomCollection::UpperFloor,
            RoomCollection::UpperFloor,
        ];

        for floor_num in 0..3usize {
            let floor_origin =
                origin.above_n(8 * floor_num as i32 + if floor_num == 2 { 3 } else { 0 });
            let rooms = &mansion.floor_rooms[floor_num];
            let grid = if floor_num == 2 {
                &mansion.third_floor_grid
            } else {
                &mansion.base_grid
            };
            let south_piece = if floor_num == 0 {
                "carpet_south_1"
            } else {
                "carpet_south_2"
            };
            let west_piece = if floor_num == 0 {
                "carpet_west_1"
            } else {
                "carpet_west_2"
            };

            for y in 0..grid.height {
                for x in 0..grid.width {
                    if grid.get(x, y) != CORRIDOR {
                        continue;
                    }
                    let mut pos = floor_origin.relative_n(
                        rotation.rotate(Direction::South),
                        8 + (y - self.start_y) * 8,
                    );
                    pos = pos.relative_n(rotation.rotate(Direction::East), (x - self.start_x) * 8);
                    pieces.push(new_piece("corridor_floor", pos, rotation, Mirror::None));

                    if grid.get(x, y - 1) == CORRIDOR
                        || (rooms.get(x, y - 1) & ROOM_CORRIDOR_FLAG) == ROOM_CORRIDOR_FLAG
                    {
                        pieces.push(new_piece(
                            "carpet_north",
                            pos.relative_n(rotation.rotate(Direction::East), 1).above(),
                            rotation,
                            Mirror::None,
                        ));
                    }
                    if grid.get(x + 1, y) == CORRIDOR
                        || (rooms.get(x + 1, y) & ROOM_CORRIDOR_FLAG) == ROOM_CORRIDOR_FLAG
                    {
                        pieces.push(new_piece(
                            "carpet_east",
                            pos.relative_n(rotation.rotate(Direction::South), 1)
                                .relative_n(rotation.rotate(Direction::East), 5)
                                .above(),
                            rotation,
                            Mirror::None,
                        ));
                    }
                    if grid.get(x, y + 1) == CORRIDOR
                        || (rooms.get(x, y + 1) & ROOM_CORRIDOR_FLAG) == ROOM_CORRIDOR_FLAG
                    {
                        pieces.push(new_piece(
                            south_piece,
                            pos.relative_n(rotation.rotate(Direction::South), 5)
                                .relative_n(rotation.rotate(Direction::West), 1),
                            rotation,
                            Mirror::None,
                        ));
                    }
                    if grid.get(x - 1, y) == CORRIDOR
                        || (rooms.get(x - 1, y) & ROOM_CORRIDOR_FLAG) == ROOM_CORRIDOR_FLAG
                    {
                        pieces.push(new_piece(
                            west_piece,
                            pos.relative_n(rotation.rotate(Direction::West), 1)
                                .relative_n(rotation.rotate(Direction::North), 1),
                            rotation,
                            Mirror::None,
                        ));
                    }
                }
            }

            let wall_piece = if floor_num == 0 {
                "indoors_wall_1"
            } else {
                "indoors_wall_2"
            };
            let door_piece = if floor_num == 0 {
                "indoors_door_1"
            } else {
                "indoors_door_2"
            };
            let mut door_dirs = Vec::new();

            for y in 0..grid.height {
                for x in 0..grid.width {
                    let mut third_floor_start_room = floor_num == 2 && grid.get(x, y) == START_ROOM;
                    if grid.get(x, y) != ROOM && !third_floor_start_room {
                        continue;
                    }
                    let room_data = rooms.get(x, y);
                    let room_type = room_data & ROOM_TYPE_MASK;
                    let room_id = room_data & ROOM_ID_MASK;
                    third_floor_start_room = third_floor_start_room
                        && (room_data & ROOM_CORRIDOR_FLAG) == ROOM_CORRIDOR_FLAG;
                    door_dirs.clear();
                    if (room_data & ROOM_DOOR_FLAG) == ROOM_DOOR_FLAG {
                        for direction in Direction::HORIZONTAL {
                            let (dx, dz) = direction.offset_xz();
                            if grid.get(x + dx, y + dz) == CORRIDOR {
                                door_dirs.push(direction);
                            }
                        }
                    }

                    let door_dir = if door_dirs.is_empty() {
                        if (room_data & ROOM_ORIGIN_FLAG) == ROOM_ORIGIN_FLAG {
                            Some(Direction::Up)
                        } else {
                            None
                        }
                    } else {
                        let index = random.next_i32_bounded(door_dirs.len() as i32) as usize;
                        Some(door_dirs[index])
                    };

                    let mut room_pos = floor_origin.relative_n(
                        rotation.rotate(Direction::South),
                        8 + (y - self.start_y) * 8,
                    );
                    room_pos = room_pos.relative_n(
                        rotation.rotate(Direction::East),
                        -1 + (x - self.start_x) * 8,
                    );

                    if is_house(grid, x - 1, y) && !mansion.is_room_id(x - 1, y, floor_num, room_id)
                    {
                        pieces.push(new_piece(
                            if door_dir == Some(Direction::West) {
                                door_piece
                            } else {
                                wall_piece
                            },
                            room_pos,
                            rotation,
                            Mirror::None,
                        ));
                    }

                    if grid.get(x + 1, y) == CORRIDOR && !third_floor_start_room {
                        let pos = room_pos.relative_n(rotation.rotate(Direction::East), 8);
                        pieces.push(new_piece(
                            if door_dir == Some(Direction::East) {
                                door_piece
                            } else {
                                wall_piece
                            },
                            pos,
                            rotation,
                            Mirror::None,
                        ));
                    }

                    if is_house(grid, x, y + 1) && !mansion.is_room_id(x, y + 1, floor_num, room_id)
                    {
                        let pos = room_pos
                            .relative_n(rotation.rotate(Direction::South), 7)
                            .relative_n(rotation.rotate(Direction::East), 7);
                        pieces.push(new_piece(
                            if door_dir == Some(Direction::South) {
                                door_piece
                            } else {
                                wall_piece
                            },
                            pos,
                            rotation.then(Rotation::Clockwise90),
                            Mirror::None,
                        ));
                    }

                    if grid.get(x, y - 1) == CORRIDOR && !third_floor_start_room {
                        let pos = room_pos
                            .relative_n(rotation.rotate(Direction::North), 1)
                            .relative_n(rotation.rotate(Direction::East), 7);
                        pieces.push(new_piece(
                            if door_dir == Some(Direction::North) {
                                door_piece
                            } else {
                                wall_piece
                            },
                            pos,
                            rotation.then(Rotation::Clockwise90),
                            Mirror::None,
                        ));
                    }

                    let collection = room_collections[floor_num];
                    if room_type == ROOM_1X1 {
                        add_room_1x1(pieces, room_pos, rotation, door_dir, collection, random);
                    } else if room_type == ROOM_1X2
                        && let Some(door_dir) = door_dir
                    {
                        let room_dir = mansion.get_1x2_room_direction(x, y, floor_num, room_id);
                        let is_stairs_room =
                            (room_data & ROOM_STAIRS_FLAG) == ROOM_STAIRS_FLAG;
                        if let Some(room_dir) = room_dir {
                            add_room_1x2(
                                pieces,
                                room_pos,
                                rotation,
                                room_dir,
                                door_dir,
                                collection,
                                is_stairs_room,
                                random,
                            );
                        }
                    } else if room_type == ROOM_2X2
                        && let Some(door_dir) = door_dir
                        && door_dir != Direction::Up
                    {
                        let mut room_dir = door_dir.rotate_y_clockwise();
                        let (rdx, rdz) = room_dir.offset_xz();
                        if !mansion.is_room_id(x + rdx, y + rdz, floor_num, room_id) {
                            room_dir = room_dir.opposite();
                        }
                        add_room_2x2(
                            pieces, room_pos, rotation, room_dir, door_dir, collection, random,
                        );
                    } else if room_type == ROOM_2X2 && door_dir == Some(Direction::Up) {
                        add_room_2x2_secret(pieces, room_pos, rotation, collection, random);
                    }
                }
            }
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors vanilla traverseOuterWalls"
    )]
    fn traverse_outer_walls(
        &self,
        pieces: &mut Vec<StructurePiece>,
        data: &mut PlacementData,
        grid: &SimpleGrid,
        mut grid_direction: Direction,
        start_x: i32,
        start_y: i32,
        end_x: i32,
        end_y: i32,
    ) {
        let mut grid_x = start_x;
        let mut grid_y = start_y;
        let start_direction = grid_direction;

        loop {
            let (dx, dz) = grid_direction.offset_xz();
            if !is_house(grid, grid_x + dx, grid_y + dz) {
                self.traverse_turn(pieces, data);
                grid_direction = grid_direction.rotate_y_clockwise();
                if grid_x != end_x || grid_y != end_y || start_direction != grid_direction {
                    self.traverse_wall_piece(pieces, data);
                }
            } else {
                let ccw = grid_direction.rotate_y_counter_clockwise();
                let (ccwx, ccwz) = ccw.offset_xz();
                if is_house(grid, grid_x + dx + ccwx, grid_y + dz + ccwz) {
                    self.traverse_inner_turn(data);
                    grid_x += dx;
                    grid_y += dz;
                    grid_direction = ccw;
                } else {
                    grid_x += dx;
                    grid_y += dz;
                    if grid_x != end_x || grid_y != end_y || start_direction != grid_direction {
                        self.traverse_wall_piece(pieces, data);
                    }
                }
            }

            if grid_x == end_x && grid_y == end_y && start_direction == grid_direction {
                break;
            }
        }
    }

    #[expect(clippy::too_many_lines, reason = "mirrors vanilla createRoof")]
    fn create_roof(
        &self,
        pieces: &mut Vec<StructurePiece>,
        roof_origin: BlockPos,
        rotation: Rotation,
        grid: &SimpleGrid,
        above_grid: Option<&SimpleGrid>,
    ) {
        let south = rotation.rotate(Direction::South);
        let east = rotation.rotate(Direction::East);
        let west = rotation.rotate(Direction::West);
        let north = rotation.rotate(Direction::North);

        for y in 0..grid.height {
            for x in 0..grid.width {
                let mut position = roof_origin.relative_n(south, 8 + (y - self.start_y) * 8);
                position = position.relative_n(east, (x - self.start_x) * 8);
                let is_above = above_grid.is_some_and(|above| is_house(above, x, y));
                if is_house(grid, x, y) && !is_above {
                    pieces.push(new_piece("roof", position.above_n(3), rotation, Mirror::None));
                    if !is_house(grid, x + 1, y) {
                        let p2 = position.relative_n(east, 6);
                        pieces.push(new_piece("roof_front", p2, rotation, Mirror::None));
                    }
                    if !is_house(grid, x - 1, y) {
                        let p2 = position.relative_n(east, 0).relative_n(south, 7);
                        pieces.push(new_piece(
                            "roof_front",
                            p2,
                            rotation.then(Rotation::Clockwise180),
                            Mirror::None,
                        ));
                    }
                    if !is_house(grid, x, y - 1) {
                        let p2 = position.relative_n(west, 1);
                        pieces.push(new_piece(
                            "roof_front",
                            p2,
                            rotation.then(Rotation::CounterClockwise90),
                            Mirror::None,
                        ));
                    }
                    if !is_house(grid, x, y + 1) {
                        let p2 = position.relative_n(east, 6).relative_n(south, 6);
                        pieces.push(new_piece(
                            "roof_front",
                            p2,
                            rotation.then(Rotation::Clockwise90),
                            Mirror::None,
                        ));
                    }
                }
            }
        }

        if let Some(above_grid) = above_grid {
            for y in 0..grid.height {
                for x in 0..grid.width {
                    let mut position = roof_origin.relative_n(south, 8 + (y - self.start_y) * 8);
                    position = position.relative_n(east, (x - self.start_x) * 8);
                    let is_above = is_house(above_grid, x, y);
                    if !is_house(grid, x, y) || !is_above {
                        continue;
                    }
                    if !is_house(grid, x + 1, y) {
                        let p2 = position.relative_n(east, 7);
                        pieces.push(new_piece("small_wall", p2, rotation, Mirror::None));
                    }
                    if !is_house(grid, x - 1, y) {
                        let p2 = position.relative_n(west, 1).relative_n(south, 6);
                        pieces.push(new_piece(
                            "small_wall",
                            p2,
                            rotation.then(Rotation::Clockwise180),
                            Mirror::None,
                        ));
                    }
                    if !is_house(grid, x, y - 1) {
                        let p2 = position.relative_n(west, 0).relative_n(north, 1);
                        pieces.push(new_piece(
                            "small_wall",
                            p2,
                            rotation.then(Rotation::CounterClockwise90),
                            Mirror::None,
                        ));
                    }
                    if !is_house(grid, x, y + 1) {
                        let p2 = position.relative_n(east, 6).relative_n(south, 7);
                        pieces.push(new_piece(
                            "small_wall",
                            p2,
                            rotation.then(Rotation::Clockwise90),
                            Mirror::None,
                        ));
                    }
                    if !is_house(grid, x + 1, y) {
                        if !is_house(grid, x, y - 1) {
                            let p2 = position.relative_n(east, 7).relative_n(north, 2);
                            pieces.push(new_piece(
                                "small_wall_corner",
                                p2,
                                rotation,
                                Mirror::None,
                            ));
                        }
                        if !is_house(grid, x, y + 1) {
                            let p2 = position.relative_n(east, 8).relative_n(south, 7);
                            pieces.push(new_piece(
                                "small_wall_corner",
                                p2,
                                rotation.then(Rotation::Clockwise90),
                                Mirror::None,
                            ));
                        }
                    }
                    if !is_house(grid, x - 1, y) {
                        if !is_house(grid, x, y - 1) {
                            let p2 = position.relative_n(west, 2).relative_n(north, 1);
                            pieces.push(new_piece(
                                "small_wall_corner",
                                p2,
                                rotation.then(Rotation::CounterClockwise90),
                                Mirror::None,
                            ));
                        }
                        if !is_house(grid, x, y + 1) {
                            let p2 = position.relative_n(west, 1).relative_n(south, 8);
                            pieces.push(new_piece(
                                "small_wall_corner",
                                p2,
                                rotation.then(Rotation::Clockwise180),
                                Mirror::None,
                            ));
                        }
                    }
                }
            }
        }

        for y in 0..grid.height {
            for x in 0..grid.width {
                let mut position = roof_origin.relative_n(south, 8 + (y - self.start_y) * 8);
                position = position.relative_n(east, (x - self.start_x) * 8);
                let is_above = above_grid.is_some_and(|above| is_house(above, x, y));
                if !is_house(grid, x, y) || is_above {
                    continue;
                }
                if !is_house(grid, x + 1, y) {
                    let p2 = position.relative_n(east, 6);
                    if !is_house(grid, x, y + 1) {
                        let p3 = p2.relative_n(south, 6);
                        pieces.push(new_piece("roof_corner", p3, rotation, Mirror::None));
                    } else if is_house(grid, x + 1, y + 1) {
                        let p3 = p2.relative_n(south, 5);
                        pieces.push(new_piece("roof_inner_corner", p3, rotation, Mirror::None));
                    }

                    if !is_house(grid, x, y - 1) {
                        pieces.push(new_piece(
                            "roof_corner",
                            p2,
                            rotation.then(Rotation::CounterClockwise90),
                            Mirror::None,
                        ));
                    } else if is_house(grid, x + 1, y - 1) {
                        let p3 = position.relative_n(east, 9).relative_n(north, 2);
                        pieces.push(new_piece(
                            "roof_inner_corner",
                            p3,
                            rotation.then(Rotation::Clockwise90),
                            Mirror::None,
                        ));
                    }
                }

                if !is_house(grid, x - 1, y) {
                    let p2 = position.relative_n(east, 0).relative_n(south, 0);
                    if !is_house(grid, x, y + 1) {
                        let p3 = p2.relative_n(south, 6);
                        pieces.push(new_piece(
                            "roof_corner",
                            p3,
                            rotation.then(Rotation::Clockwise90),
                            Mirror::None,
                        ));
                    } else if is_house(grid, x - 1, y + 1) {
                        let p3 = p2.relative_n(south, 8).relative_n(west, 3);
                        pieces.push(new_piece(
                            "roof_inner_corner",
                            p3,
                            rotation.then(Rotation::CounterClockwise90),
                            Mirror::None,
                        ));
                    }

                    if !is_house(grid, x, y - 1) {
                        pieces.push(new_piece(
                            "roof_corner",
                            p2,
                            rotation.then(Rotation::Clockwise180),
                            Mirror::None,
                        ));
                    } else if is_house(grid, x - 1, y - 1) {
                        let p3 = p2.relative_n(south, 1);
                        pieces.push(new_piece(
                            "roof_inner_corner",
                            p3,
                            rotation.then(Rotation::Clockwise180),
                            Mirror::None,
                        ));
                    }
                }
            }
        }
    }

    fn entrance(&self, pieces: &mut Vec<StructurePiece>, data: &mut PlacementData) {
        let west = data.rotation.rotate(Direction::West);
        pieces.push(new_piece(
            "entrance",
            data.position.relative_n(west, 9),
            data.rotation,
            Mirror::None,
        ));
        data.position = data
            .position
            .relative_n(data.rotation.rotate(Direction::South), 16);
    }

    fn traverse_wall_piece(&self, pieces: &mut Vec<StructurePiece>, data: &mut PlacementData) {
        pieces.push(new_piece(
            data.wall_type,
            data.position
                .relative_n(data.rotation.rotate(Direction::East), 7),
            data.rotation,
            Mirror::None,
        ));
        data.position = data
            .position
            .relative_n(data.rotation.rotate(Direction::South), 8);
    }

    fn traverse_turn(&self, pieces: &mut Vec<StructurePiece>, data: &mut PlacementData) {
        data.position = data
            .position
            .relative_n(data.rotation.rotate(Direction::South), -1);
        pieces.push(new_piece(
            "wall_corner",
            data.position,
            data.rotation,
            Mirror::None,
        ));
        data.position = data
            .position
            .relative_n(data.rotation.rotate(Direction::South), -7);
        data.position = data
            .position
            .relative_n(data.rotation.rotate(Direction::West), -6);
        data.rotation = data.rotation.then(Rotation::Clockwise90);
    }

    fn traverse_inner_turn(&self, data: &mut PlacementData) {
        data.position = data
            .position
            .relative_n(data.rotation.rotate(Direction::South), 6);
        data.position = data
            .position
            .relative_n(data.rotation.rotate(Direction::East), 8);
        data.rotation = data.rotation.then(Rotation::CounterClockwise90);
    }
}

fn add_room_1x1(
    pieces: &mut Vec<StructurePiece>,
    room_pos: BlockPos,
    rotation: Rotation,
    door_dir: Option<Direction>,
    rooms: RoomCollection,
    random: &mut RandomSource,
) {
    let mut piece_rot = Rotation::None;
    let mut room_type = rooms.get_1x1(random);
    if door_dir != Some(Direction::East) {
        match door_dir {
            Some(Direction::North) => piece_rot = piece_rot.then(Rotation::CounterClockwise90),
            Some(Direction::West) => piece_rot = piece_rot.then(Rotation::Clockwise180),
            Some(Direction::South) => piece_rot = piece_rot.then(Rotation::Clockwise90),
            _ => room_type = rooms.get_1x1_secret(random),
        }
    }

    let orientation =
        zero_position_with_transform(BlockPos::new(1, 0, 0), Mirror::None, piece_rot, 7, 7);
    let piece_rot = piece_rot.then(rotation);
    let orientation = rotation.rotate_pos(orientation);
    let pos = room_pos.offset(orientation.x(), 0, orientation.z());
    pieces.push(new_piece(&room_type, pos, piece_rot, Mirror::None));
}

#[expect(clippy::too_many_arguments, reason = "mirrors vanilla addRoom1x2")]
fn add_room_1x2(
    pieces: &mut Vec<StructurePiece>,
    room_pos: BlockPos,
    rotation: Rotation,
    room_dir: Direction,
    door_dir: Direction,
    rooms: RoomCollection,
    is_stairs_room: bool,
    random: &mut RandomSource,
) {
    let east = rotation.rotate(Direction::East);
    let west = rotation.rotate(Direction::West);
    let south = rotation.rotate(Direction::South);
    let north = rotation.rotate(Direction::North);

    match (door_dir, room_dir) {
        (Direction::East, Direction::South) => {
            let pos = room_pos.relative_n(east, 1);
            let name = rooms.get_1x2_side_entrance(random, is_stairs_room);
            pieces.push(new_piece(&name, pos, rotation, Mirror::None));
        }
        (Direction::East, Direction::North) => {
            let pos = room_pos.relative_n(east, 1).relative_n(south, 6);
            let name = rooms.get_1x2_side_entrance(random, is_stairs_room);
            pieces.push(new_piece(&name, pos, rotation, Mirror::LeftRight));
        }
        (Direction::West, Direction::North) => {
            let pos = room_pos.relative_n(east, 7).relative_n(south, 6);
            let name = rooms.get_1x2_side_entrance(random, is_stairs_room);
            pieces.push(new_piece(
                &name,
                pos,
                rotation.then(Rotation::Clockwise180),
                Mirror::None,
            ));
        }
        (Direction::West, Direction::South) => {
            let pos = room_pos.relative_n(east, 7);
            let name = rooms.get_1x2_side_entrance(random, is_stairs_room);
            pieces.push(new_piece(&name, pos, rotation, Mirror::FrontBack));
        }
        (Direction::South, Direction::East) => {
            let pos = room_pos.relative_n(east, 1);
            let name = rooms.get_1x2_side_entrance(random, is_stairs_room);
            pieces.push(new_piece(
                &name,
                pos,
                rotation.then(Rotation::Clockwise90),
                Mirror::LeftRight,
            ));
        }
        (Direction::South, Direction::West) => {
            let pos = room_pos.relative_n(east, 7);
            let name = rooms.get_1x2_side_entrance(random, is_stairs_room);
            pieces.push(new_piece(
                &name,
                pos,
                rotation.then(Rotation::Clockwise90),
                Mirror::None,
            ));
        }
        (Direction::North, Direction::West) => {
            let pos = room_pos.relative_n(east, 7).relative_n(south, 6);
            let name = rooms.get_1x2_side_entrance(random, is_stairs_room);
            pieces.push(new_piece(
                &name,
                pos,
                rotation.then(Rotation::Clockwise90),
                Mirror::FrontBack,
            ));
        }
        (Direction::North, Direction::East) => {
            let pos = room_pos.relative_n(east, 1).relative_n(south, 6);
            let name = rooms.get_1x2_side_entrance(random, is_stairs_room);
            pieces.push(new_piece(
                &name,
                pos,
                rotation.then(Rotation::CounterClockwise90),
                Mirror::None,
            ));
        }
        (Direction::South, Direction::North) => {
            let pos = room_pos.relative_n(east, 1).relative_n(north, 8);
            let name = rooms.get_1x2_front_entrance(random, is_stairs_room);
            pieces.push(new_piece(&name, pos, rotation, Mirror::None));
        }
        (Direction::North, Direction::South) => {
            let pos = room_pos.relative_n(east, 7).relative_n(south, 14);
            let name = rooms.get_1x2_front_entrance(random, is_stairs_room);
            pieces.push(new_piece(
                &name,
                pos,
                rotation.then(Rotation::Clockwise180),
                Mirror::None,
            ));
        }
        (Direction::West, Direction::East) => {
            let pos = room_pos.relative_n(east, 15);
            let name = rooms.get_1x2_front_entrance(random, is_stairs_room);
            pieces.push(new_piece(
                &name,
                pos,
                rotation.then(Rotation::Clockwise90),
                Mirror::None,
            ));
        }
        (Direction::East, Direction::West) => {
            let pos = room_pos.relative_n(west, 7).relative_n(south, 6);
            let name = rooms.get_1x2_front_entrance(random, is_stairs_room);
            pieces.push(new_piece(
                &name,
                pos,
                rotation.then(Rotation::CounterClockwise90),
                Mirror::None,
            ));
        }
        (Direction::Up, Direction::East) => {
            let pos = room_pos.relative_n(east, 15);
            let name = rooms.get_1x2_secret(random);
            pieces.push(new_piece(
                &name,
                pos,
                rotation.then(Rotation::Clockwise90),
                Mirror::None,
            ));
        }
        (Direction::Up, Direction::South) => {
            let pos = room_pos.relative_n(east, 1).relative_n(north, 0);
            let name = rooms.get_1x2_secret(random);
            pieces.push(new_piece(&name, pos, rotation, Mirror::None));
        }
        _ => {}
    }
}

fn add_room_2x2(
    pieces: &mut Vec<StructurePiece>,
    room_pos: BlockPos,
    rotation: Rotation,
    room_dir: Direction,
    door_dir: Direction,
    rooms: RoomCollection,
    random: &mut RandomSource,
) {
    let mut east = 0;
    let mut south = 0;
    let mut rot = rotation;
    let mut mirror = Mirror::None;

    match (door_dir, room_dir) {
        (Direction::East, Direction::South) => east = -7,
        (Direction::East, Direction::North) => {
            east = -7;
            south = 6;
            mirror = Mirror::LeftRight;
        }
        (Direction::North, Direction::East) => {
            east = 1;
            south = 14;
            rot = rotation.then(Rotation::CounterClockwise90);
        }
        (Direction::North, Direction::West) => {
            east = 7;
            south = 14;
            rot = rotation.then(Rotation::CounterClockwise90);
            mirror = Mirror::LeftRight;
        }
        (Direction::South, Direction::West) => {
            east = 7;
            south = -8;
            rot = rotation.then(Rotation::Clockwise90);
        }
        (Direction::South, Direction::East) => {
            east = 1;
            south = -8;
            rot = rotation.then(Rotation::Clockwise90);
            mirror = Mirror::LeftRight;
        }
        (Direction::West, Direction::North) => {
            east = 15;
            south = 6;
            rot = rotation.then(Rotation::Clockwise180);
        }
        (Direction::West, Direction::South) => {
            east = 15;
            mirror = Mirror::FrontBack;
        }
        _ => {}
    }

    let mut pos = room_pos.relative_n(rotation.rotate(Direction::East), east);
    pos = pos.relative_n(rotation.rotate(Direction::South), south);
    let name = rooms.get_2x2(random);
    pieces.push(new_piece(&name, pos, rot, mirror));
}

fn add_room_2x2_secret(
    pieces: &mut Vec<StructurePiece>,
    room_pos: BlockPos,
    rotation: Rotation,
    rooms: RoomCollection,
    random: &mut RandomSource,
) {
    let pos = room_pos.relative_n(rotation.rotate(Direction::East), 1);
    let name = rooms.get_2x2_secret(random);
    pieces.push(new_piece(&name, pos, rotation, Mirror::None));
}

fn make_location(template_name: &str) -> Identifier {
    format!("minecraft:woodland_mansion/{template_name}")
        .parse()
        .expect("woodland mansion template path should be a valid identifier")
}

fn make_settings(mirror: Mirror, rotation: Rotation) -> StructurePlaceSettings {
    let mut settings = StructurePlaceSettings::new()
        .set_ignore_entities(true)
        .set_rotation(rotation)
        .set_mirror(mirror);
    settings.add_processor(StructureProcessor::BlockIgnore {
        blocks: HolderSet::Entries(vec![Identifier::const_vanilla("structure_block")]),
    });
    settings
}

fn new_piece(
    template_name: &str,
    position: BlockPos,
    rotation: Rotation,
    mirror: Mirror,
) -> StructurePiece {
    let location = make_location(template_name);
    let (base, template) =
        TemplateStructurePiece::new(0, &location, make_settings(mirror, rotation), position);
    StructurePiece::new(
        base,
        PieceKind::WoodlandMansion(WoodlandMansionPiece { template }),
    )
}

#[derive(Debug)]
pub struct WoodlandMansionPiece {
    pub template: TemplateStructurePiece,
}

impl WoodlandMansionPiece {
    pub fn post_process(
        &mut self,
        base: &mut PieceBase,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        chunk_bb: BoundingBox,
        reference_pos: BlockPos,
    ) {
        let rotation = self.template.place_settings.rotation();
        self.template.post_process(
            base,
            region,
            random,
            chunk_bb,
            reference_pos,
            |marker_id, position, region, random, chunk_bb| {
                if marker_id.starts_with("Chest") {
                    let mut chest_state = vanilla_blocks::CHEST.default_state();
                    let facing = match marker_id {
                        "ChestWest" => Some(Direction::West),
                        "ChestEast" => Some(Direction::East),
                        "ChestSouth" => Some(Direction::South),
                        "ChestNorth" => Some(Direction::North),
                        _ => None,
                    };
                    if let Some(facing) = facing {
                        chest_state = chest_state.set_value(
                            &Properties::HORIZONTAL_FACING,
                            rotation.rotate(facing),
                        );
                    }
                    create_chest_at(region, chunk_bb, random, position, Some(chest_state));
                } else if matches!(marker_id, "Mage" | "Warrior") {
                    region.set_block(position, vanilla_blocks::AIR.default_state());
                } else if marker_id == "Group of Allays" {
                    let count = region
                        .with_random(|level_random| level_random.next_i32_bounded(3) + 1);
                    if count > 0 {
                        region.set_block(position, vanilla_blocks::AIR.default_state());
                    }
                }
            },
        );
    }
}
