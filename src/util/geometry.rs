use std::marker::PhantomData;
use std::ops::{Add, Div, Neg, Sub};

use glam::{DVec3, IVec3};

use crate::util::axis::Axis;
use crate::util::pos::BlockPos;

const fn ordered_pair(a: f64, b: f64) -> (f64, f64) {
    if a <= b { (a, b) } else { (b, a) }
}

const fn ordered_pair_i32(a: i32, b: i32) -> (i32, i32) {
    if a <= b { (a, b) } else { (b, a) }
}

pub trait Space {
    const ZERO_SPAN_IS_EMPTY: bool;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlockLocal;

impl Space for BlockLocal {
    const ZERO_SPAN_IS_EMPTY: bool = true;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct World;

impl Space for World {
    const ZERO_SPAN_IS_EMPTY: bool = true;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Structure;

impl Space for Structure {
    const ZERO_SPAN_IS_EMPTY: bool = false;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Aabb<T, I> {
    min: T,
    max: T,
    p: PhantomData<I>,
}

pub type BlockLocalAabb = Aabb<DVec3, BlockLocal>;
pub type WorldAabb = Aabb<DVec3, World>;
pub type BoundingBox = Aabb<IVec3, Structure>;

pub trait AabbVector: Copy + Add<Output = Self> + Sub<Output = Self> {
    type Scalar: Copy
        + PartialOrd
        + Add<Output = Self::Scalar>
        + Sub<Output = Self::Scalar>
        + Div<Output = Self::Scalar>
        + Neg<Output = Self::Scalar>
        + From<u8>;

    fn new(x: Self::Scalar, y: Self::Scalar, z: Self::Scalar) -> Self;
    fn x(self) -> Self::Scalar;
    fn y(self) -> Self::Scalar;
    fn z(self) -> Self::Scalar;

    #[must_use]
    fn min(self, other: Self) -> Self;

    #[must_use]
    fn max(self, other: Self) -> Self;
}

impl AabbVector for DVec3 {
    type Scalar = f64;

    fn new(x: Self::Scalar, y: Self::Scalar, z: Self::Scalar) -> Self {
        DVec3::new(x, y, z)
    }

    fn x(self) -> Self::Scalar {
        self.x
    }

    fn y(self) -> Self::Scalar {
        self.y
    }

    fn z(self) -> Self::Scalar {
        self.z
    }

    fn min(self, other: Self) -> Self {
        self.min(other)
    }

    fn max(self, other: Self) -> Self {
        self.max(other)
    }
}

impl AabbVector for IVec3 {
    type Scalar = i32;

    fn new(x: Self::Scalar, y: Self::Scalar, z: Self::Scalar) -> Self {
        IVec3::new(x, y, z)
    }

    fn x(self) -> Self::Scalar {
        self.x
    }

    fn y(self) -> Self::Scalar {
        self.y
    }

    fn z(self) -> Self::Scalar {
        self.z
    }

    fn min(self, other: Self) -> Self {
        self.min(other)
    }

    fn max(self, other: Self) -> Self {
        self.max(other)
    }
}

impl<T: AabbVector, I> Aabb<T, I> {
    #[must_use]
    pub fn min(&self, axis: Axis) -> T::Scalar {
        match axis {
            Axis::X => self.min.x(),
            Axis::Y => self.min.y(),
            Axis::Z => self.min.z(),
        }
    }

    #[must_use]
    pub fn max(&self, axis: Axis) -> T::Scalar {
        match axis {
            Axis::X => self.max.x(),
            Axis::Y => self.max.y(),
            Axis::Z => self.max.z(),
        }
    }

    #[must_use]
    pub fn from_min_max(min: T, max: T) -> Self {
        Self {
            min: min.min(max),
            max: min.max(max),
            p: PhantomData,
        }
    }

    #[must_use]
    pub fn translate(self, delta: T) -> Self {
        Self {
            min: self.min + delta,
            max: self.max + delta,
            p: PhantomData,
        }
    }

    #[must_use]
    pub fn inflate(self, amount: T::Scalar) -> Self {
        self.inflate_xyz(amount, amount, amount)
    }

    #[must_use]
    pub fn inflate_xyz(self, x: T::Scalar, y: T::Scalar, z: T::Scalar) -> Self {
        let delta = T::new(x, y, z);
        Self::from_min_max(self.min - delta, self.max + delta)
    }

    #[must_use]
    pub fn encapsulating(a: &Self, b: &Self) -> Self {
        Self {
            min: a.min.min(b.min),
            max: a.max.max(b.max),
            p: PhantomData,
        }
    }
}

impl<I> Aabb<DVec3, I> {
    #[must_use]
    pub const fn min_corner(&self) -> DVec3 {
        self.min
    }

    #[must_use]
    pub const fn max_corner(&self) -> DVec3 {
        self.max
    }

    #[must_use]
    pub const fn min_x(&self) -> f64 {
        self.min.x
    }

    #[must_use]
    pub const fn min_y(&self) -> f64 {
        self.min.y
    }

    #[must_use]
    pub const fn min_z(&self) -> f64 {
        self.min.z
    }

    #[must_use]
    pub const fn max_x(&self) -> f64 {
        self.max.x
    }

    #[must_use]
    pub const fn max_y(&self) -> f64 {
        self.max.y
    }

    #[must_use]
    pub const fn max_z(&self) -> f64 {
        self.max.z
    }

    #[must_use]
    pub fn distance_to_sqr(self, point: DVec3) -> f64 {
        let dx = f64::max(f64::max(self.min.x - point.x, point.x - self.max.x), 0.0);
        let dy = f64::max(f64::max(self.min.y - point.y, point.y - self.max.y), 0.0);
        let dz = f64::max(f64::max(self.min.z - point.z, point.z - self.max.z), 0.0);
        dx * dx + dy * dy + dz * dz
    }

    #[must_use]
    pub const fn closest_point_to(self, point: DVec3) -> DVec3 {
        DVec3::new(
            point.x.clamp(self.min.x, self.max.x),
            point.y.clamp(self.min.y, self.max.y),
            point.z.clamp(self.min.z, self.max.z),
        )
    }
}

impl<I> Aabb<IVec3, I> {
    #[must_use]
    #[inline]
    pub const fn min_corner(&self) -> IVec3 {
        self.min
    }

    #[must_use]
    #[inline]
    pub const fn max_corner(&self) -> IVec3 {
        self.max
    }

    #[must_use]
    #[inline]
    pub const fn min_x(&self) -> i32 {
        self.min.x
    }

    #[must_use]
    #[inline]
    pub const fn min_y(&self) -> i32 {
        self.min.y
    }

    #[must_use]
    #[inline]
    pub const fn min_z(&self) -> i32 {
        self.min.z
    }

    #[must_use]
    #[inline]
    pub const fn max_x(&self) -> i32 {
        self.max.x
    }

    #[must_use]
    #[inline]
    pub const fn max_y(&self) -> i32 {
        self.max.y
    }

    #[must_use]
    #[inline]
    pub const fn max_z(&self) -> i32 {
        self.max.z
    }
}

impl<T: AabbVector, I> Aabb<T, I> {
    #[must_use]
    pub fn deflate(self, amount: T::Scalar) -> Self {
        self.inflate(-amount)
    }
}

impl<T: AabbVector, I: Space> Aabb<T, I> {
    #[inline]
    fn axis_overlaps(min1: T::Scalar, max1: T::Scalar, min2: T::Scalar, max2: T::Scalar) -> bool {
        if I::ZERO_SPAN_IS_EMPTY {
            min1 < max2 && max1 > min2
        } else {
            min1 <= max2 && max1 >= min2
        }
    }

    #[inline]
    fn axis_contains(min: T::Scalar, max: T::Scalar, v: T::Scalar) -> bool {
        if I::ZERO_SPAN_IS_EMPTY {
            v >= min && v < max
        } else {
            v >= min && v <= max
        }
    }

    pub fn is_empty(&self) -> bool {
        if I::ZERO_SPAN_IS_EMPTY {
            self.min.x() >= self.max.x()
                || self.min.y() >= self.max.y()
                || self.min.z() >= self.max.z()
        } else {
            self.min.x() > self.max.x()
                || self.min.y() > self.max.y()
                || self.min.z() > self.max.z()
        }
    }

    #[must_use]
    pub fn intersects(self, other: Self) -> bool {
        self.intersects_bounds(other.min, other.max)
    }

    #[must_use]
    pub fn intersects_bounds(self, min: T, max: T) -> bool {
        Self::axis_overlaps(self.min.x(), self.max.x(), min.x(), max.x())
            && Self::axis_overlaps(self.min.y(), self.max.y(), min.y(), max.y())
            && Self::axis_overlaps(self.min.z(), self.max.z(), min.z(), max.z())
    }

    #[must_use]
    pub fn intersects_xz(
        self,
        min_x: T::Scalar,
        min_z: T::Scalar,
        max_x: T::Scalar,
        max_z: T::Scalar,
    ) -> bool {
        Self::axis_overlaps(self.min.x(), self.max.x(), min_x, max_x)
            && Self::axis_overlaps(self.min.z(), self.max.z(), min_z, max_z)
    }

    #[must_use]
    pub fn contains(self, pos: T) -> bool {
        self.contains_xyz(pos.x(), pos.y(), pos.z())
    }

    #[must_use]
    pub fn contains_xyz(self, x: T::Scalar, y: T::Scalar, z: T::Scalar) -> bool {
        Self::axis_contains(self.min.x(), self.max.x(), x)
            && Self::axis_contains(self.min.y(), self.max.y(), y)
            && Self::axis_contains(self.min.z(), self.max.z(), z)
    }
}

impl<T: AabbVector, I: Space> Aabb<T, I> {
    #[inline]
    fn span(raw: T::Scalar) -> T::Scalar {
        if I::ZERO_SPAN_IS_EMPTY {
            raw
        } else {
            raw + T::Scalar::from(1u8)
        }
    }

    #[must_use]
    pub fn width(&self) -> T::Scalar {
        Self::span(self.max.x() - self.min.x())
    }

    #[must_use]
    pub fn height(&self) -> T::Scalar {
        Self::span(self.max.y() - self.min.y())
    }

    #[must_use]
    pub fn depth(&self) -> T::Scalar {
        Self::span(self.max.z() - self.min.z())
    }

    #[must_use]
    pub fn center(&self) -> T {
        let two = T::Scalar::from(2u8);
        T::new(
            self.min.x() + Self::span(self.max.x() - self.min.x()) / two,
            self.min.y() + Self::span(self.max.y() - self.min.y()) / two,
            self.min.z() + Self::span(self.max.z() - self.min.z()) / two,
        )
    }
}

impl<I: Space> Aabb<IVec3, I> {
    #[must_use]
    pub fn contains_blockpos(self, pos: BlockPos) -> bool {
        self.contains(pos.0)
    }
}

impl<I: Space> Aabb<DVec3, I> {
    pub const FULL_BLOCK: Self = Self::new(0.0, 0.0, 0.0, 1.0, 1.0, 1.0);
    pub const EMPTY: Self = Self::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);

    #[must_use]
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
            p: PhantomData,
        }
    }

    #[must_use]
    pub fn of_size(center: DVec3, size_x: f64, size_y: f64, size_z: f64) -> Self {
        let half_x = size_x / 2.0;
        let half_y = size_y / 2.0;
        let half_z = size_z / 2.0;
        Self::new(
            center.x - half_x,
            center.y - half_y,
            center.z - half_z,
            center.x + half_x,
            center.y + half_y,
            center.z + half_z,
        )
    }

    #[must_use]
    pub fn size(self) -> f64 {
        (self.width() + self.height() + self.depth()) / 3.0
    }
}

impl Aabb<DVec3, BlockLocal> {
    #[must_use]
    pub fn at_block(self, pos: BlockPos) -> Aabb<DVec3, World> {
        let offset = DVec3::new(f64::from(pos.x()), f64::from(pos.y()), f64::from(pos.z()));
        Aabb {
            min: self.min + offset,
            max: self.max + offset,
            p: PhantomData,
        }
    }
}

impl Aabb<IVec3, Structure> {
    #[must_use]
    pub const fn new(pos1: IVec3, pos2: IVec3) -> Self {
        let (min_x, max_x) = ordered_pair_i32(pos1.x, pos2.x);
        let (min_y, max_y) = ordered_pair_i32(pos1.y, pos2.y);
        let (min_z, max_z) = ordered_pair_i32(pos1.z, pos2.z);
        Self {
            min: IVec3::new(min_x, min_y, min_z),
            max: IVec3::new(max_x, max_y, max_z),
            p: PhantomData,
        }
    }

    #[must_use]
    pub const fn from_corners(a: BlockPos, b: BlockPos) -> Self {
        Self::new(a.0, b.0)
    }

    #[must_use]
    pub fn distance_to_sqr(self, point: DVec3) -> f64 {
        let min_x = f64::from(self.min_x());
        let min_y = f64::from(self.min_y());
        let min_z = f64::from(self.min_z());
        let max_x = f64::from(self.max_x());
        let max_y = f64::from(self.max_y());
        let max_z = f64::from(self.max_z());

        let dx = f64::max(f64::max(min_x - point.x, point.x - max_x), 0.0);
        let dy = f64::max(f64::max(min_y - point.y, point.y - max_y), 0.0);
        let dz = f64::max(f64::max(min_z - point.z, point.z - max_z), 0.0);
        dx * dx + dy * dy + dz * dz
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructors_normalize_endpoints_like_vanilla() {
        let aabb = WorldAabb::new(3.0, 4.0, 5.0, 1.0, 2.0, 0.0);
        assert_eq!(aabb.min_corner(), DVec3::new(1.0, 2.0, 0.0));
        assert_eq!(aabb.max_corner(), DVec3::new(3.0, 4.0, 5.0));
    }

    #[test]
    fn inflate_and_deflate_are_inverse_on_symmetric_boxes() {
        let aabb = WorldAabb::new(0.0, 0.0, 0.0, 1.0, 1.0, 1.0);
        let deflated = aabb.deflate(0.25);
        assert_eq!(deflated.min_corner(), DVec3::splat(0.25));
        assert_eq!(deflated.max_corner(), DVec3::splat(0.75));
        assert_eq!(deflated.inflate(0.25), aabb);
    }

    #[test]
    fn bounding_box_inflate_shrinks_with_negative_amount() {
        let bbox = BoundingBox::new(IVec3::ZERO, IVec3::splat(5)).inflate(-4);
        assert_eq!(bbox.min_corner(), IVec3::splat(1));
        assert_eq!(bbox.max_corner(), IVec3::splat(4));
    }

    #[test]
    fn contains_uses_exclusive_max_edge() {
        let aabb = WorldAabb::new(0.0, 0.0, 0.0, 1.0, 1.0, 1.0);
        assert!(aabb.contains_xyz(0.0, 0.5, 0.5));
        assert!(!aabb.contains_xyz(1.0, 0.5, 0.5));
    }

    #[test]
    fn structure_bounding_box_contains_is_inclusive_at_max() {
        let bbox = BoundingBox::new(IVec3::ZERO, IVec3::splat(4));
        assert!(bbox.contains_blockpos(BlockPos::new(4, 4, 4)));
        assert!(!bbox.contains_blockpos(BlockPos::new(5, 4, 4)));
    }

    #[test]
    fn distance_to_sqr_is_zero_when_point_inside() {
        let aabb = WorldAabb::new(1.0, 2.0, 3.0, 4.0, 6.0, 8.0);
        assert!((aabb.distance_to_sqr(DVec3::new(2.0, 3.0, 4.0))).abs() < 1e-12);
    }

    #[test]
    fn closest_point_to_clamps_into_box() {
        let aabb = WorldAabb::new(1.0, 2.0, 3.0, 4.0, 6.0, 8.0);
        let closest = aabb.closest_point_to(DVec3::new(0.0, 4.0, 10.0));
        assert_eq!(closest, DVec3::new(1.0, 4.0, 8.0));
    }

    #[test]
    fn block_local_aabb_translates_to_world_space() {
        let local = BlockLocalAabb::new(0.0, 0.25, 0.0, 1.0, 0.75, 1.0);
        let world = local.at_block(BlockPos::new(10, 64, -5));
        assert_eq!(world.min_corner(), DVec3::new(10.0, 64.25, -5.0));
        assert_eq!(world.max_corner(), DVec3::new(11.0, 64.75, -4.0));
    }
}
