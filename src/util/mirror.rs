use shulkr::util::Axis;
use shulkr::util::Direction;
use shulkr::world::block::{Mirror, Rotation};

pub trait MirrorExt {
    fn mirror_index(self, rotation: i32, steps: i32) -> i32;

    fn get_rotation(self, direction: Direction) -> Rotation;

    fn mirror(self, direction: Direction) -> Direction;
}

impl MirrorExt for Mirror {
    fn mirror_index(self, rotation: i32, steps: i32) -> i32 {
        let half_steps = steps / 2;
        let corrected = if rotation > half_steps {
            rotation - steps
        } else {
            rotation
        };
        match self {
            Self::LeftRight => (half_steps - corrected + steps) % steps,
            Self::FrontBack => (steps - corrected) % steps,
            Self::None => rotation,
        }
    }

    fn get_rotation(self, direction: Direction) -> Rotation {
        let axis = direction.get_axis();
        match self {
            Self::LeftRight if matches!(axis, Axis::Z) => Rotation::Clockwise180,
            Self::FrontBack if matches!(axis, Axis::X) => Rotation::Clockwise180,
            _ => Rotation::None,
        }
    }

    fn mirror(self, direction: Direction) -> Direction {
        match self {
            Self::FrontBack if matches!(direction.get_axis(), Axis::X) => direction.opposite(),
            Self::LeftRight if matches!(direction.get_axis(), Axis::Z) => direction.opposite(),
            _ => direction,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirror_flips_only_the_matching_axis() {
        assert_eq!(Mirror::FrontBack.mirror(Direction::East), Direction::West);
        assert_eq!(Mirror::FrontBack.mirror(Direction::North), Direction::North);
        assert_eq!(Mirror::LeftRight.mirror(Direction::North), Direction::South);
        assert_eq!(Mirror::LeftRight.mirror(Direction::East), Direction::East);
        assert_eq!(Mirror::None.mirror(Direction::East), Direction::East);
    }

    #[test]
    fn get_rotation_matches_vanilla() {
        assert_eq!(
            Mirror::LeftRight.get_rotation(Direction::North),
            Rotation::Clockwise180
        );
        assert_eq!(
            Mirror::LeftRight.get_rotation(Direction::East),
            Rotation::None
        );
        assert_eq!(
            Mirror::FrontBack.get_rotation(Direction::West),
            Rotation::Clockwise180
        );
    }

    #[test]
    fn mirror_index_matches_vanilla_for_16_steps() {
        assert_eq!(Mirror::None.mirror_index(3, 16), 3);
        assert_eq!(Mirror::LeftRight.mirror_index(0, 16), 8);
        assert_eq!(Mirror::LeftRight.mirror_index(12, 16), 12);
        assert_eq!(Mirror::FrontBack.mirror_index(4, 16), 12);
        assert_eq!(Mirror::FrontBack.mirror_index(0, 16), 0);
    }
}
