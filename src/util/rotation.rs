use crate::util::rng::RandomSource;
use glam::IVec3;
use shulkr::util::BlockPosition;
use shulkr::util::BoundingBox;
use shulkr::util::Direction;
use shulkr::world::block::Rotation;

pub trait RotationExt {
    fn get_random(rng: &mut RandomSource) -> Self;

    fn get_shuffled(rng: &mut RandomSource) -> [Rotation; 4];

    fn rotate(self, dir: Direction) -> Direction;

    fn rotate_pos(self, pos: BlockPosition) -> BlockPosition;

    fn rotate_index(self, rotation: i32, steps: i32) -> i32;

    fn transform_pos(self, pos: IVec3, pivot: IVec3) -> IVec3;

    fn rotate_size(self, size: IVec3) -> IVec3;

    fn transform_pos_mirrored(self, pos: IVec3, pivot: IVec3, mirror_front_back: bool) -> IVec3;

    fn get_bounding_box_full(
        self,
        pos: IVec3,
        size: IVec3,
        pivot: IVec3,
        mirror_front_back: bool,
    ) -> BoundingBox;

    fn get_bounding_box_with_pivot(self, pos: IVec3, size: IVec3, pivot: IVec3) -> BoundingBox;

    fn get_bounding_box(self, pos: IVec3, size: IVec3) -> BoundingBox;
}

impl RotationExt for Rotation {
    fn get_random(rng: &mut RandomSource) -> Self {
        Rotation::ALL[rng.next_i32_bounded(4) as usize]
    }

    fn get_shuffled(rng: &mut RandomSource) -> [Rotation; 4] {
        let mut rotations = Rotation::ALL;
        for i in (1..4).rev() {
            let j = rng.next_i32_bounded((i + 1) as i32) as usize;
            rotations.swap(i, j);
        }
        rotations
    }

    fn rotate(self, dir: Direction) -> Direction {
        match self {
            Self::None => dir,
            Self::Clockwise90 => dir.rotate_y_clockwise(),
            Self::Clockwise180 => dir.rotate_y_clockwise().rotate_y_clockwise(),
            Self::CounterClockwise90 => dir.rotate_y_counter_clockwise(),
        }
    }

    fn rotate_pos(self, pos: BlockPosition) -> BlockPosition {
        match self {
            Self::None => pos,
            Self::Clockwise90 => BlockPosition::new(-pos.z(), pos.y(), pos.x()),
            Self::Clockwise180 => BlockPosition::new(-pos.x(), pos.y(), -pos.z()),
            Self::CounterClockwise90 => BlockPosition::new(pos.z(), pos.y(), -pos.x()),
        }
    }

    fn rotate_index(self, rotation: i32, steps: i32) -> i32 {
        match self {
            Self::None => rotation,
            Self::Clockwise90 => (rotation + steps / 4) % steps,
            Self::Clockwise180 => (rotation + steps / 2) % steps,
            Self::CounterClockwise90 => (rotation + steps * 3 / 4) % steps,
        }
    }

    fn transform_pos(self, pos: IVec3, pivot: IVec3) -> IVec3 {
        let (x, y, z) = (pos.x, pos.y, pos.z);
        let (px, pz) = (pivot.x, pivot.z);
        match self {
            Self::None => IVec3::new(x, y, z),
            Self::Clockwise90 => IVec3::new(px + pz - z, y, pz - px + x),
            Self::Clockwise180 => IVec3::new(px + px - x, y, pz + pz - z),
            Self::CounterClockwise90 => IVec3::new(px - pz + z, y, px + pz - x),
        }
    }

    fn rotate_size(self, size: IVec3) -> IVec3 {
        match self {
            Self::Clockwise90 | Self::CounterClockwise90 => IVec3::new(size.z, size.y, size.x),
            Self::None | Self::Clockwise180 => size,
        }
    }

    fn transform_pos_mirrored(self, pos: IVec3, pivot: IVec3, mirror_front_back: bool) -> IVec3 {
        let mx = if mirror_front_back { -pos.x } else { pos.x };
        let mirrored_pos = IVec3::new(mx, pos.y, pos.z);
        self.transform_pos(mirrored_pos, pivot)
    }

    fn get_bounding_box_full(
        self,
        pos: IVec3,
        size: IVec3,
        pivot: IVec3,
        mirror_front_back: bool,
    ) -> BoundingBox {
        let c1 = self.transform_pos_mirrored(IVec3::ZERO, pivot, mirror_front_back);
        let c2 = self.transform_pos_mirrored(
            IVec3::new(size.x - 1, size.y - 1, size.z - 1),
            pivot,
            mirror_front_back,
        );
        let (min, max) = (c1.min(c2) + pos, c1.max(c2) + pos);
        BoundingBox::new(min.x, min.y, min.z, max.x, max.y, max.z)
    }

    fn get_bounding_box_with_pivot(self, pos: IVec3, size: IVec3, pivot: IVec3) -> BoundingBox {
        self.get_bounding_box_full(pos, size, pivot, false)
    }

    fn get_bounding_box(self, pos: IVec3, size: IVec3) -> BoundingBox {
        self.get_bounding_box_full(pos, size, IVec3::ZERO, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotate_direction_matches_vanilla() {
        assert_eq!(Rotation::None.rotate(Direction::North), Direction::North);
        assert_eq!(
            Rotation::Clockwise90.rotate(Direction::North),
            Direction::East
        );
        assert_eq!(
            Rotation::Clockwise180.rotate(Direction::North),
            Direction::South
        );
        assert_eq!(
            Rotation::CounterClockwise90.rotate(Direction::North),
            Direction::West
        );
    }

    #[test]
    fn composing_opposite_rotations_is_identity() {
        assert_eq!(
            Rotation::Clockwise90.then(Rotation::CounterClockwise90),
            Rotation::None
        );
        assert_eq!(
            Rotation::Clockwise180.then(Rotation::Clockwise180),
            Rotation::None
        );
    }

    #[test]
    fn vertical_directions_are_unaffected_by_rotation() {
        assert_eq!(Rotation::Clockwise90.rotate(Direction::Up), Direction::Up);
        assert_eq!(
            Rotation::Clockwise180.rotate(Direction::Down),
            Direction::Down
        );
    }

    #[test]
    fn transform_pos_around_zero_pivot() {
        assert_eq!(
            Rotation::Clockwise90.transform_pos(IVec3::new(3, 5, 7), IVec3::ZERO),
            IVec3::new(-7, 5, 3)
        );
        assert_eq!(
            Rotation::Clockwise180.transform_pos(IVec3::new(3, 5, 7), IVec3::ZERO),
            IVec3::new(-3, 5, -7)
        );
    }

    #[test]
    fn rotate_size_swaps_x_and_z_only_on_quarter_turns() {
        assert_eq!(
            Rotation::None.rotate_size(IVec3::new(6, 10, 8)),
            IVec3::new(6, 10, 8)
        );
        assert_eq!(
            Rotation::Clockwise90.rotate_size(IVec3::new(6, 10, 8)),
            IVec3::new(8, 10, 6)
        );
        assert_eq!(
            Rotation::Clockwise180.rotate_size(IVec3::new(6, 10, 8)),
            IVec3::new(6, 10, 8)
        );
    }

    #[test]
    fn bounding_box_none_covers_full_size_from_origin() {
        let bb = Rotation::None.get_bounding_box(IVec3::new(0, 0, 0), IVec3::new(6, 10, 6));
        assert_eq!((bb.min_x(), bb.min_y(), bb.min_z()), (0, 0, 0));
        assert_eq!((bb.max_x(), bb.max_y(), bb.max_z()), (5, 9, 5));
    }
}
