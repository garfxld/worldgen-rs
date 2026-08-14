use glam::{DVec3, IVec3};

use crate::util::axis::Axis;

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

    pub const fn new(min_x: f64, min_y: f64, min_z: f64, max_x: f64, max_y: f64, max_z: f64) -> Self {
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
