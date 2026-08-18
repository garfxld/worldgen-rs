use glam::{DVec3, IVec3};

use crate::util::axis::Axis;
use crate::util::pos::BlockPos;

const fn ordered_pair(a: f64, b: f64) -> (f64, f64) {
    if a <= b { (a, b) } else { (b, a) }
}

const fn ordered_pair_i32(a: i32, b: i32) -> (i32, i32) {
    if a <= b { (a, b) } else { (b, a) }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlockLocalAabb {
    min: DVec3,
    max: DVec3,
}

impl BlockLocalAabb {
    pub const FULL_BLOCK: Self = Self::new(0.0, 0.0, 0.0, 1.0, 1.0, 1.0);

    pub const fn new(
        min_x: f64,
        min_y: f64,
        min_z: f64,
        max_x: f64,
        max_y: f64,
        max_z: f64,
    ) -> Self {
        let (min_x, max_x) = ordered_pair(min_x, max_x);
        let (min_y, max_y) = ordered_pair(min_y, max_y);
        let (min_z, max_z) = ordered_pair(min_z, max_z);
        Self {
            min: DVec3::new(min_x, min_y, min_z),
            max: DVec3::new(max_x, max_y, max_z),
        }
    }

    pub const fn min(self, axis: Axis) -> f64 {
        match axis {
            Axis::X => self.min.x,
            Axis::Y => self.min.y,
            Axis::Z => self.min.z,
        }
    }

    pub const fn max(self, axis: Axis) -> f64 {
        match axis {
            Axis::X => self.max.x,
            Axis::Y => self.max.y,
            Axis::Z => self.max.z,
        }
    }

    pub const fn min_x(self) -> f64 {
        self.min.x
    }

    pub const fn min_y(self) -> f64 {
        self.min.y
    }

    pub const fn min_z(self) -> f64 {
        self.min.z
    }

    pub const fn max_x(self) -> f64 {
        self.max.x
    }

    pub const fn max_y(self) -> f64 {
        self.max.y
    }

    pub const fn max_z(self) -> f64 {
        self.max.z
    }

    pub fn translate(self, delta: DVec3) -> Self {
        Self {
            min: self.min + delta,
            max: self.max + delta,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.min.x >= self.max.x || self.min.y >= self.max.y || self.min.z >= self.max.z
    }

    pub const fn of(box_: BoundingBox) -> Self {
        Self {
            min: DVec3::new(box_.min.x as f64, box_.min.y as f64, box_.min.z as f64),
            max: DVec3::new(
                (box_.max.x + 1) as f64,
                (box_.max.y + 1) as f64,
                (box_.max.z + 1) as f64,
            ),
        }
    }

    pub fn inflate(self, x_add: f64, y_add: f64, z_add: f64) -> Self {
        Self::new(
            self.min.x - x_add,
            self.min.y - y_add,
            self.min.z - z_add,
            self.max.x + x_add,
            self.max.y + y_add,
            self.max.z + z_add,
        )
    }

    pub fn deflate(self, amount: f64) -> Self {
        self.inflate(-amount, -amount, -amount)
    }

    pub fn height(self) -> f64 {
        self.max.y - self.min.y
    }

    pub fn size(self) -> f64 {
        let x_size = self.max.x - self.min.x;
        let y_size = self.max.y - self.min.y;
        let z_size = self.max.z - self.min.z;
        (x_size + y_size + z_size) / 3.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BoundingBox {
    min: IVec3,
    max: IVec3,
}

impl BoundingBox {
    pub const fn new(pos1: IVec3, pos2: IVec3) -> Self {
        let (min_x, max_x) = ordered_pair_i32(pos1.x, pos2.x);
        let (min_y, max_y) = ordered_pair_i32(pos1.y, pos2.y);
        let (min_z, max_z) = ordered_pair_i32(pos1.z, pos2.z);
        Self {
            min: IVec3::new(min_x, min_y, min_z),
            max: IVec3::new(max_x, max_y, max_z),
        }
    }

    pub const fn from_coords(
        min_x: i32,
        min_y: i32,
        min_z: i32,
        max_x: i32,
        max_y: i32,
        max_z: i32,
    ) -> Self {
        Self::new(
            IVec3::new(min_x, min_y, min_z),
            IVec3::new(max_x, max_y, max_z),
        )
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors vanilla BoundingBox::orientBox"
    )]
    pub const fn orient_box(
        foot_x: i32,
        foot_y: i32,
        foot_z: i32,
        off_x: i32,
        off_y: i32,
        off_z: i32,
        width: i32,
        height: i32,
        depth: i32,
        direction: crate::util::direction::Direction,
    ) -> Self {
        use crate::util::direction::Direction;
        match direction {
            Direction::North => Self::from_coords(
                foot_x + off_x,
                foot_y + off_y,
                foot_z - depth + 1 + off_z,
                foot_x + width - 1 + off_x,
                foot_y + height - 1 + off_y,
                foot_z + off_z,
            ),
            Direction::West => Self::from_coords(
                foot_x - depth + 1 + off_z,
                foot_y + off_y,
                foot_z + off_x,
                foot_x + off_z,
                foot_y + height - 1 + off_y,
                foot_z + width - 1 + off_x,
            ),
            Direction::East => Self::from_coords(
                foot_x + off_z,
                foot_y + off_y,
                foot_z + off_x,
                foot_x + depth - 1 + off_z,
                foot_y + height - 1 + off_y,
                foot_z + width - 1 + off_x,
            ),
            _ => Self::from_coords(
                foot_x + off_x,
                foot_y + off_y,
                foot_z + off_z,
                foot_x + width - 1 + off_x,
                foot_y + height - 1 + off_y,
                foot_z + depth - 1 + off_z,
            ),
        }
    }

    pub const fn min_x(self) -> i32 {
        self.min.x
    }

    pub const fn min_y(self) -> i32 {
        self.min.y
    }

    pub const fn min_z(self) -> i32 {
        self.min.z
    }

    pub const fn max_x(self) -> i32 {
        self.max.x
    }

    pub const fn max_y(self) -> i32 {
        self.max.y
    }

    pub const fn max_z(self) -> i32 {
        self.max.z
    }

    pub const fn from_point(pos: BlockPos) -> Self {
        Self::new(pos.0, pos.0)
    }

    pub const fn encapsulate(self, pos: BlockPos) -> Self {
        Self {
            min: IVec3::new(
                self.min.x.min(pos.0.x),
                self.min.y.min(pos.0.y),
                self.min.z.min(pos.0.z),
            ),
            max: IVec3::new(
                self.max.x.max(pos.0.x),
                self.max.y.max(pos.0.y),
                self.max.z.max(pos.0.z),
            ),
        }
    }

    pub const fn inflated_by_xyz(self, dx: i32, dy: i32, dz: i32) -> Self {
        Self {
            min: IVec3::new(self.min.x - dx, self.min.y - dy, self.min.z - dz),
            max: IVec3::new(self.max.x + dx, self.max.y + dy, self.max.z + dz),
        }
    }

    pub const fn moved(self, dx: i32, dy: i32, dz: i32) -> Self {
        Self {
            min: IVec3::new(self.min.x + dx, self.min.y + dy, self.min.z + dz),
            max: IVec3::new(self.max.x + dx, self.max.y + dy, self.max.z + dz),
        }
    }

    pub fn for_all_corners(self, mut consumer: impl FnMut(BlockPos)) {
        for z in [self.min.z, self.max.z] {
            for y in [self.min.y, self.max.y] {
                for x in [self.min.x, self.max.x] {
                    consumer(BlockPos::new(x, y, z));
                }
            }
        }
    }

    pub const fn infinite() -> Self {
        Self {
            min: IVec3::new(i32::MIN, i32::MIN, i32::MIN),
            max: IVec3::new(i32::MAX, i32::MAX, i32::MAX),
        }
    }

    pub const fn inflated_by(self, amount: i32) -> Self {
        self.inflated_by_xyz(amount, amount, amount)
    }

    pub const fn intersects(self, other: Self) -> bool {
        self.max.x >= other.min.x
            && self.min.x <= other.max.x
            && self.max.z >= other.min.z
            && self.min.z <= other.max.z
            && self.max.y >= other.min.y
            && self.min.y <= other.max.y
    }

    pub const fn intersects_xz(self, min_x: i32, min_z: i32, max_x: i32, max_z: i32) -> bool {
        self.max.x >= min_x && self.min.x <= max_x && self.max.z >= min_z && self.min.z <= max_z
    }

    pub const fn encapsulate_box(self, other: Self) -> Self {
        Self {
            min: IVec3::new(
                if self.min.x < other.min.x {
                    self.min.x
                } else {
                    other.min.x
                },
                if self.min.y < other.min.y {
                    self.min.y
                } else {
                    other.min.y
                },
                if self.min.z < other.min.z {
                    self.min.z
                } else {
                    other.min.z
                },
            ),
            max: IVec3::new(
                if self.max.x > other.max.x {
                    self.max.x
                } else {
                    other.max.x
                },
                if self.max.y > other.max.y {
                    self.max.y
                } else {
                    other.max.y
                },
                if self.max.z > other.max.z {
                    self.max.z
                } else {
                    other.max.z
                },
            ),
        }
    }

    pub const fn x_span(self) -> i32 {
        self.max.x - self.min.x + 1
    }

    pub const fn y_span(self) -> i32 {
        self.max.y - self.min.y + 1
    }

    pub const fn z_span(self) -> i32 {
        self.max.z - self.min.z + 1
    }

    pub const fn center(self) -> BlockPos {
        BlockPos::new(
            self.min.x + (self.max.x - self.min.x + 1) / 2,
            self.min.y + (self.max.y - self.min.y + 1) / 2,
            self.min.z + (self.max.z - self.min.z + 1) / 2,
        )
    }

    pub const fn is_inside(self, pos: BlockPos) -> bool {
        pos.0.x >= self.min.x
            && pos.0.x <= self.max.x
            && pos.0.y >= self.min.y
            && pos.0.y <= self.max.y
            && pos.0.z >= self.min.z
            && pos.0.z <= self.max.z
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_local_constructor_normalizes_endpoints() {
        let aabb = BlockLocalAabb::new(1.0, 1.0, 1.0, 0.0, 0.0, 0.0);
        assert_eq!(aabb.min_x(), 0.0);
        assert_eq!(aabb.max_x(), 1.0);
    }

    #[test]
    fn block_local_is_empty_uses_exclusive_max() {
        assert!(!BlockLocalAabb::FULL_BLOCK.is_empty());
        assert!(BlockLocalAabb::new(0.5, 0.0, 0.0, 0.5, 1.0, 1.0).is_empty());
    }

    #[test]
    fn block_local_translate_shifts_both_corners() {
        let translated = BlockLocalAabb::FULL_BLOCK.translate(DVec3::new(2.0, 0.0, -1.0));
        assert_eq!(translated.min_x(), 2.0);
        assert_eq!(translated.max_x(), 3.0);
        assert_eq!(translated.min_z(), -1.0);
        assert_eq!(translated.max_z(), 0.0);
    }

    #[test]
    fn bounding_box_constructor_accepts_either_corner_order() {
        let a = BoundingBox::new(IVec3::new(5, 5, 5), IVec3::ZERO);
        let b = BoundingBox::new(IVec3::ZERO, IVec3::new(5, 5, 5));
        assert_eq!(a, b);
        assert_eq!((a.min_x(), a.min_y(), a.min_z()), (0, 0, 0));
        assert_eq!((a.max_x(), a.max_y(), a.max_z()), (5, 5, 5));
    }
}
