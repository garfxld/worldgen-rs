use glam::IVec3;

use crate::util::axis::Axis;
use crate::util::pos::BlockPos;

#[derive(Clone, Copy, Debug)]
#[derive_const(PartialEq)]
pub enum Direction {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

impl Direction {
    pub const fn offset(self) -> (i32, i32, i32) {
        match self {
            Direction::Down => (0, -1, 0),
            Direction::Up => (0, 1, 0),
            Direction::North => (0, 0, -1),
            Direction::South => (0, 0, 1),
            Direction::West => (-1, 0, 0),
            Direction::East => (1, 0, 0),
        }
    }

    pub const fn offset_xz(self) -> (i32, i32) {
        match self {
            Direction::Down | Direction::Up => (0, 0),
            Direction::North => (0, -1),
            Direction::South => (0, 1),
            Direction::West => (-1, 0),
            Direction::East => (1, 0),
        }
    }

    pub const fn offset_vec(self) -> IVec3 {
        let (x, y, z) = self.offset();
        IVec3::new(x, y, z)
    }

    pub fn relative(self, pos: BlockPos) -> BlockPos {
        BlockPos(pos.0 + self.offset_vec())
    }

    pub const fn get_axis(self) -> Axis {
        match self {
            Direction::Down | Direction::Up => Axis::Y,
            Direction::North | Direction::South => Axis::Z,
            Direction::West | Direction::East => Axis::X,
        }
    }

    pub const fn opposite(self) -> Direction {
        match self {
            Direction::Down => Direction::Up,
            Direction::Up => Direction::Down,
            Direction::North => Direction::South,
            Direction::South => Direction::North,
            Direction::West => Direction::East,
            Direction::East => Direction::West,
        }
    }

    pub fn from_yaw(yaw: f32) -> Direction {
        let adjusted = yaw.rem_euclid(360.0);
        match adjusted {
            y if !(45.0..315.0).contains(&y) => Direction::South,
            y if y < 135.0 => Direction::West,
            y if y < 225.0 => Direction::North,
            _ => Direction::East,
        }
    }

    pub const fn to_yaw(self) -> f32 {
        match self {
            Direction::North => 180.0,
            Direction::South | Direction::Up | Direction::Down => 0.0,
            Direction::West => 90.0,
            Direction::East => 270.0,
        }
    }

    pub const fn axis(self) -> Axis {
        self.get_axis()
    }

    pub const fn positive_for_axis(axis: Axis) -> Direction {
        match axis {
            Axis::X => Direction::East,
            Axis::Y => Direction::Up,
            Axis::Z => Direction::South,
        }
    }

    pub const fn is_horizontal(self) -> bool {
        matches!(
            self,
            Direction::North | Direction::South | Direction::East | Direction::West
        )
    }

    pub const fn rotate_y_clockwise(self) -> Direction {
        match self {
            Direction::North => Direction::East,
            Direction::East => Direction::South,
            Direction::South => Direction::West,
            Direction::West => Direction::North,
            other => other,
        }
    }

    pub const fn rotate_y_counter_clockwise(self) -> Direction {
        match self {
            Direction::North => Direction::West,
            Direction::West => Direction::South,
            Direction::South => Direction::East,
            Direction::East => Direction::North,
            other => other,
        }
    }

    pub const UPDATE_SHAPE_ORDER: [Direction; 6] = [
        Direction::West,
        Direction::East,
        Direction::North,
        Direction::South,
        Direction::Down,
        Direction::Up,
    ];

    pub const FLOW_NEIGHBOR_CHECK: [Direction; 5] = [
        Direction::Up,
        Direction::North,
        Direction::South,
        Direction::West,
        Direction::East,
    ];

    pub const HORIZONTAL: [Direction; 4] = [
        Direction::North,
        Direction::East,
        Direction::South,
        Direction::West,
    ];

    pub const ALL: [Direction; 6] = [
        Direction::Down,
        Direction::Up,
        Direction::North,
        Direction::South,
        Direction::West,
        Direction::East,
    ];

    pub fn ordered_by_nearest(yaw: f32, pitch: f32) -> [Direction; 6] {
        let pitch_rad = pitch.to_radians();
        let yaw_rad = (-yaw).to_radians();

        let pitch_sin = pitch_rad.sin();
        let pitch_cos = pitch_rad.cos();
        let yaw_sin = yaw_rad.sin();
        let yaw_cos = yaw_rad.cos();

        let x_pos = yaw_sin > 0.0;
        let y_pos = pitch_sin < 0.0;
        let z_pos = yaw_cos > 0.0;

        let x_yaw = if x_pos { yaw_sin } else { -yaw_sin };
        let y_mag = if y_pos { -pitch_sin } else { pitch_sin };
        let z_yaw = if z_pos { yaw_cos } else { -yaw_cos };
        let x_mag = x_yaw * pitch_cos;
        let z_mag = z_yaw * pitch_cos;

        let axis_x = if x_pos {
            Direction::East
        } else {
            Direction::West
        };
        let axis_y = if y_pos {
            Direction::Up
        } else {
            Direction::Down
        };
        let axis_z = if z_pos {
            Direction::South
        } else {
            Direction::North
        };

        if x_yaw > z_yaw {
            if y_mag > x_mag {
                Self::make_direction_array(axis_y, axis_x, axis_z)
            } else if z_mag > y_mag {
                Self::make_direction_array(axis_x, axis_z, axis_y)
            } else {
                Self::make_direction_array(axis_x, axis_y, axis_z)
            }
        } else if y_mag > z_mag {
            Self::make_direction_array(axis_y, axis_z, axis_x)
        } else if x_mag > y_mag {
            Self::make_direction_array(axis_z, axis_x, axis_y)
        } else {
            Self::make_direction_array(axis_z, axis_y, axis_x)
        }
    }

    const fn make_direction_array(
        axis1: Direction,
        axis2: Direction,
        axis3: Direction,
    ) -> [Direction; 6] {
        [
            axis1,
            axis2,
            axis3,
            axis3.opposite(),
            axis2.opposite(),
            axis1.opposite(),
        ]
    }

    pub const fn as_str(&self) -> &str {
        match self {
            Direction::Down => "down",
            Direction::Up => "up",
            Direction::North => "north",
            Direction::South => "south",
            Direction::West => "west",
            Direction::East => "east",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horizontal_order_matches_vanilla_plane_order() {
        assert_eq!(
            Direction::HORIZONTAL,
            [
                Direction::North,
                Direction::East,
                Direction::South,
                Direction::West,
            ]
        );
    }

    #[test]
    fn all_order_matches_vanilla_values_order() {
        assert_eq!(
            Direction::ALL,
            [
                Direction::Down,
                Direction::Up,
                Direction::North,
                Direction::South,
                Direction::West,
                Direction::East,
            ]
        );
    }

    #[test]
    fn opposite_is_involutive() {
        for direction in Direction::ALL {
            assert_eq!(direction.opposite().opposite(), direction);
        }
    }

    #[test]
    fn rotate_y_clockwise_is_involution_of_counter_clockwise() {
        for direction in Direction::ALL {
            assert_eq!(
                direction.rotate_y_clockwise().rotate_y_counter_clockwise(),
                direction
            );
        }
    }

    #[test]
    fn positive_for_axis_matches_vanilla_axis_direction_positive() {
        assert_eq!(Direction::positive_for_axis(Axis::X), Direction::East);
        assert_eq!(Direction::positive_for_axis(Axis::Y), Direction::Up);
        assert_eq!(Direction::positive_for_axis(Axis::Z), Direction::South);
    }

    #[test]
    fn get_axis_matches_offset_axis() {
        for direction in Direction::ALL {
            let (x, y, _z) = direction.offset();
            let axis_from_offset = if x != 0 {
                Axis::X
            } else if y != 0 {
                Axis::Y
            } else {
                Axis::Z
            };
            assert_eq!(direction.get_axis(), axis_from_offset);
        }
    }
}
