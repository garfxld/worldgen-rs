use std::collections::VecDeque;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::hash::{Hash, Hasher};

use glam::{DVec3, IVec2, IVec3};
use rustc_hash::FxHashSet;

use crate::util::axis::Axis;
use crate::util::direction::Direction;
use crate::util::ident::Identifier;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkPos(pub IVec2);

impl Hash for ChunkPos {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(PackedChunkPos::from(*self).as_raw() as u64);
    }
}

impl ChunkPos {
    const OFFSETS: [(i32, i32); 8] = [
        (-1, -1),
        (0, -1),
        (1, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (0, 1),
        (1, 1),
    ];

    pub const MAX_COORDINATE_VALUE: i32 =
        SectionPos::block_to_section_coord(BlockPos::MAX_HORIZONTAL_COORDINATE);

    #[must_use]
    pub fn neighbors(self) -> [ChunkPos; 8] {
        Self::OFFSETS.map(|(dx, dy)| ChunkPos::new(self.0.x + dx, self.0.y + dy))
    }

    #[must_use]
    pub const fn new(x: i32, y: i32) -> Self {
        Self(IVec2::new(x, y))
    }

    #[must_use]
    pub const fn from_block_pos(pos: BlockPos) -> Self {
        Self::new(
            SectionPos::block_to_section_coord(pos.0.x),
            SectionPos::block_to_section_coord(pos.0.z),
        )
    }

    #[must_use]
    pub fn from_entity_pos(pos: DVec3) -> Self {
        Self::from_block_pos(BlockPos::from(pos))
    }

    #[must_use]
    pub const fn is_valid(x: i32, z: i32) -> bool {
        x.abs().max(z.abs()) <= Self::MAX_COORDINATE_VALUE
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockPos(pub IVec3);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraversalNodeStatus {
    Accept,
    Skip,
    Stop,
}

#[derive(Clone, Debug)]
pub struct SpiralIter {
    directions: [Direction; 4],
    cursor: BlockPos,
    legs: i32,
    leg: i32,
    leg_size: i32,
    leg_index: i32,
    last: BlockPos,
}

impl Iterator for SpiralIter {
    type Item = BlockPos;

    fn next(&mut self) -> Option<Self::Item> {
        let direction_index = (self.leg + 4).rem_euclid(4) as usize;
        self.cursor = self.last.relative(self.directions[direction_index]);
        self.last = self.cursor;

        if self.leg_index >= self.leg_size {
            if self.leg >= self.legs {
                return None;
            }

            self.leg += 1;
            self.leg_index = 0;
            self.leg_size = self.leg / 2 + 1;
        }

        self.leg_index += 1;
        Some(self.cursor)
    }
}

impl From<DVec3> for BlockPos {
    fn from(value: DVec3) -> Self {
        BlockPos(IVec3 {
            x: value.x.floor() as i32,
            y: value.y.floor() as i32,
            z: value.z.floor() as i32,
        })
    }
}

impl BlockPos {
    pub const ZERO: BlockPos = BlockPos(IVec3::new(0, 0, 0));

    pub const MAX_HORIZONTAL_COORDINATE: i32 = (1 << PackedBlockPos::HORIZONTAL_BITS) / 2 - 1;

    #[must_use]
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self(IVec3::new(x, y, z))
    }

    #[must_use]
    pub const fn offset(&self, dx: i32, dy: i32, dz: i32) -> Self {
        Self(IVec3::new(self.0.x + dx, self.0.y + dy, self.0.z + dz))
    }

    #[must_use]
    pub const fn x(&self) -> i32 {
        self.0.x
    }

    #[must_use]
    pub const fn y(&self) -> i32 {
        self.0.y
    }

    #[must_use]
    pub const fn z(&self) -> i32 {
        self.0.z
    }

    #[must_use]
    pub const fn above(&self) -> Self {
        self.offset(0, 1, 0)
    }

    #[must_use]
    pub const fn above_n(&self, n: i32) -> Self {
        self.offset(0, n, 0)
    }

    #[must_use]
    pub const fn below(&self) -> Self {
        self.offset(0, -1, 0)
    }

    #[must_use]
    pub const fn below_n(&self, n: i32) -> Self {
        self.offset(0, -n, 0)
    }

    #[must_use]
    pub const fn north(&self) -> Self {
        self.offset(0, 0, -1)
    }

    #[must_use]
    pub const fn south(&self) -> Self {
        self.offset(0, 0, 1)
    }

    #[must_use]
    pub const fn west(&self) -> Self {
        self.offset(-1, 0, 0)
    }

    #[must_use]
    pub const fn east(&self) -> Self {
        self.offset(1, 0, 0)
    }

    #[must_use]
    pub fn relative(self, direction: Direction) -> Self {
        Self(self.0 + direction.offset_vec())
    }

    #[must_use]
    pub fn breadth_first_traversal<NP, P>(
        start_pos: Self,
        max_depth: i32,
        max_count: i32,
        mut neighbor_provider: NP,
        mut node_processor: P,
    ) -> i32
    where
        NP: FnMut(Self, &mut dyn FnMut(Self)),
        P: FnMut(Self) -> TraversalNodeStatus,
    {
        let mut nodes = VecDeque::from([(start_pos, 0)]);
        let mut visited = FxHashSet::default();
        let mut count = 0;

        while let Some((current_pos, depth)) = nodes.pop_front() {
            if !visited.insert(current_pos) {
                continue;
            }

            let next = node_processor(current_pos);
            if next == TraversalNodeStatus::Skip {
                continue;
            }

            if next == TraversalNodeStatus::Stop {
                break;
            }

            count += 1;
            if count >= max_count {
                return count;
            }

            if depth < max_depth {
                let next_depth = depth + 1;
                neighbor_provider(current_pos, &mut |pos| nodes.push_back((pos, next_depth)));
            }
        }

        count
    }

    #[must_use]
    pub fn relative_n(&self, direction: Direction, n: i32) -> Self {
        if n == 0 {
            *self
        } else {
            Self(self.0 + direction.offset_vec() * n)
        }
    }

    #[must_use]
    pub fn spiral_around(
        center: Self,
        radius: i32,
        first_direction: Direction,
        second_direction: Direction,
    ) -> SpiralIter {
        assert!(radius >= 0, "spiral radius must be non-negative");
        assert!(
            first_direction.get_axis() != second_direction.get_axis(),
            "spiral directions cannot be on the same axis"
        );

        let cursor = center.relative(second_direction);
        SpiralIter {
            directions: [
                first_direction,
                second_direction,
                first_direction.opposite(),
                second_direction.opposite(),
            ],
            cursor,
            legs: 4 * radius,
            leg: -1,
            leg_size: 0,
            leg_index: 0,
            last: cursor,
        }
    }

    #[must_use]
    pub const fn relative_axis(&self, axis: Axis, n: i32) -> Self {
        if n == 0 {
            *self
        } else {
            match axis {
                Axis::X => self.offset(n, 0, 0),
                Axis::Y => self.offset(0, n, 0),
                Axis::Z => self.offset(0, 0, n),
            }
        }
    }

    #[must_use]
    pub const fn at_y(&self, y: i32) -> Self {
        Self::new(self.0.x, y, self.0.z)
    }

    #[must_use]
    pub const fn multiply(&self, factor: i32) -> Self {
        if factor == 1 {
            *self
        } else if factor == 0 {
            Self::ZERO
        } else {
            Self::new(self.0.x * factor, self.0.y * factor, self.0.z * factor)
        }
    }

    #[must_use]
    pub fn get_center(&self) -> (f64, f64, f64) {
        (
            f64::from(self.0.x) + 0.5,
            f64::from(self.0.y) + 0.5,
            f64::from(self.0.z) + 0.5,
        )
    }

    #[must_use]
    pub fn get_bottom_center(&self) -> (f64, f64, f64) {
        (
            f64::from(self.0.x) + 0.5,
            f64::from(self.0.y),
            f64::from(self.0.z) + 0.5,
        )
    }

    #[must_use]
    pub const fn containing(x: f64, y: f64, z: f64) -> Self {
        Self::new(x.floor() as i32, y.floor() as i32, z.floor() as i32)
    }

    #[must_use]
    pub const fn min(a: BlockPos, b: BlockPos) -> Self {
        Self::new(a.0.x.min(b.0.x), a.0.y.min(b.0.y), a.0.z.min(b.0.z))
    }

    #[must_use]
    pub const fn max(a: BlockPos, b: BlockPos) -> Self {
        Self::new(a.0.x.max(b.0.x), a.0.y.max(b.0.y), a.0.z.max(b.0.z))
    }

    #[must_use]
    pub const fn within_manhattan(
        self,
        reach_x: i32,
        reach_y: i32,
        reach_z: i32,
    ) -> ManhattanBlockPos {
        ManhattanBlockPos {
            origin: self,
            reach_x,
            reach_y,
            reach_z,
            max_depth: reach_x + reach_y + reach_z,
            current_depth: 0,
            max_x: 0,
            max_y: 0,
            x: 0,
            y: 0,
            pending_z_mirror: None,
            done: false,
        }
    }

    #[must_use]
    pub fn find_closest_match(
        self,
        horizontal_search_radius: i32,
        vertical_search_radius: i32,
        mut predicate: impl FnMut(BlockPos) -> bool,
    ) -> Option<BlockPos> {
        self.within_manhattan(
            horizontal_search_radius,
            vertical_search_radius,
            horizontal_search_radius,
        )
        .find(|pos| predicate(*pos))
    }
}

#[derive(Debug, Clone)]
pub struct ManhattanBlockPos {
    origin: BlockPos,
    reach_x: i32,
    reach_y: i32,
    reach_z: i32,
    max_depth: i32,
    current_depth: i32,
    max_x: i32,
    max_y: i32,
    x: i32,
    y: i32,
    pending_z_mirror: Option<BlockPos>,
    done: bool,
}

impl Iterator for ManhattanBlockPos {
    type Item = BlockPos;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(pos) = self.pending_z_mirror.take() {
            return Some(pos);
        }
        if self.done {
            return None;
        }

        loop {
            if self.y > self.max_y {
                self.x += 1;
                if self.x > self.max_x {
                    self.current_depth += 1;
                    if self.current_depth > self.max_depth {
                        self.done = true;
                        return None;
                    }

                    self.max_x = self.reach_x.min(self.current_depth);
                    self.x = -self.max_x;
                }

                self.max_y = self.reach_y.min(self.current_depth - self.x.abs());
                self.y = -self.max_y;
            }

            let x = self.x;
            let y = self.y;
            let z = self.current_depth - x.abs() - y.abs();
            self.y += 1;
            if z > self.reach_z {
                continue;
            }

            let pos = self.origin.offset(x, y, z);
            if z != 0 {
                self.pending_z_mirror = Some(self.origin.offset(x, y, -z));
            }
            return Some(pos);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GlobalPos {
    pub dimension: Identifier,
    pub pos: BlockPos,
}

impl GlobalPos {
    #[must_use]
    pub const fn new(dimension: Identifier, pos: BlockPos) -> Self {
        Self { dimension, pos }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SectionPos(pub IVec3);

impl SectionPos {
    const SECTION_BITS: i32 = 4;
    const SECTION_SIZE: i32 = 1 << Self::SECTION_BITS;
    pub(super) const SECTION_MASK: i32 = Self::SECTION_SIZE - 1;

    #[must_use]
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self(IVec3::new(x, y, z))
    }

    #[must_use]
    pub const fn block_to_section_coord(block_coord: i32) -> i32 {
        block_coord >> Self::SECTION_BITS
    }

    #[must_use]
    pub const fn from_block_pos(pos: BlockPos) -> Self {
        Self::new(
            Self::block_to_section_coord(pos.0.x),
            Self::block_to_section_coord(pos.0.y),
            Self::block_to_section_coord(pos.0.z),
        )
    }

    #[must_use]
    pub fn from_entity_pos(pos: DVec3) -> Self {
        Self::from_block_pos(BlockPos::from(pos))
    }

    #[must_use]
    pub const fn x(&self) -> i32 {
        self.0.x
    }

    #[must_use]
    pub const fn y(&self) -> i32 {
        self.0.y
    }

    #[must_use]
    pub const fn z(&self) -> i32 {
        self.0.z
    }

    #[must_use]
    pub const fn relative_to_block_x(&self, relative: PackedSectionBlockPos) -> i32 {
        (self.0.x << Self::SECTION_BITS) + relative.x() as i32
    }

    #[must_use]
    pub const fn relative_to_block_y(&self, relative: PackedSectionBlockPos) -> i32 {
        (self.0.y << Self::SECTION_BITS) + relative.y() as i32
    }

    #[must_use]
    pub const fn relative_to_block_z(&self, relative: PackedSectionBlockPos) -> i32 {
        (self.0.z << Self::SECTION_BITS) + relative.z() as i32
    }

    #[must_use]
    pub const fn section_relative_pos(pos: BlockPos) -> PackedSectionBlockPos {
        PackedSectionBlockPos::from_block_pos(pos)
    }

    #[must_use]
    pub const fn relative_to_block_pos(&self, relative: PackedSectionBlockPos) -> BlockPos {
        BlockPos(IVec3::new(
            self.relative_to_block_x(relative),
            self.relative_to_block_y(relative),
            self.relative_to_block_z(relative),
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackedChunkPos(i64);

impl PackedChunkPos {
    #[must_use]
    pub const fn from_raw(raw: i64) -> Self {
        Self(raw)
    }

    #[must_use]
    pub const fn as_raw(self) -> i64 {
        self.0
    }

    #[must_use]
    pub const fn to_chunk_pos(self) -> ChunkPos {
        ChunkPos(IVec2::new(
            (self.0 & 0xFFFF_FFFF) as i32,
            (self.0 >> 32) as i32,
        ))
    }
}

impl From<ChunkPos> for PackedChunkPos {
    fn from(pos: ChunkPos) -> Self {
        Self((i64::from(pos.0.x) & 0xFFFF_FFFF) | ((i64::from(pos.0.y) & 0xFFFF_FFFF) << 32))
    }
}

impl From<PackedChunkPos> for ChunkPos {
    fn from(pos: PackedChunkPos) -> Self {
        pos.to_chunk_pos()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackedBlockPos(i64);

impl PackedBlockPos {
    pub(super) const HORIZONTAL_BITS: u32 = 26;
    const Y_BITS: u32 = 12;
    const X_OFFSET: u32 = Self::HORIZONTAL_BITS + Self::Y_BITS;
    const Z_OFFSET: u32 = Self::Y_BITS;
    const XZ_MASK: i64 = (1i64 << Self::HORIZONTAL_BITS) - 1;
    const Y_MASK: i64 = (1i64 << Self::Y_BITS) - 1;

    #[must_use]
    pub const fn from_raw(raw: i64) -> Self {
        Self(raw)
    }

    #[must_use]
    pub const fn as_raw(self) -> i64 {
        self.0
    }

    #[must_use]
    pub const fn to_block_pos(self) -> BlockPos {
        let x = self.0 >> Self::X_OFFSET;
        let y = self.0 & Self::Y_MASK;
        let z = (self.0 >> Self::Z_OFFSET) & Self::XZ_MASK;

        let x = (x << (64 - Self::HORIZONTAL_BITS)) >> (64 - Self::HORIZONTAL_BITS);
        let y = (y << (64 - Self::Y_BITS)) >> (64 - Self::Y_BITS);
        let z = (z << (64 - Self::HORIZONTAL_BITS)) >> (64 - Self::HORIZONTAL_BITS);

        BlockPos(IVec3::new(x as i32, y as i32, z as i32))
    }
}

impl From<BlockPos> for PackedBlockPos {
    fn from(pos: BlockPos) -> Self {
        let x = i64::from(pos.0.x);
        let y = i64::from(pos.0.y);
        let z = i64::from(pos.0.z);
        Self(
            ((x & Self::XZ_MASK) << Self::X_OFFSET)
                | ((z & Self::XZ_MASK) << Self::Z_OFFSET)
                | (y & Self::Y_MASK),
        )
    }
}

impl From<PackedBlockPos> for BlockPos {
    fn from(pos: PackedBlockPos) -> Self {
        pos.to_block_pos()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackedSectionPos(i64);

impl PackedSectionPos {
    const XZ_BITS: u32 = 22;
    const Y_BITS: u32 = 20;
    const X_OFFSET: u32 = Self::XZ_BITS + Self::Y_BITS;
    const Z_OFFSET: u32 = Self::Y_BITS;
    const XZ_MASK: i64 = (1i64 << Self::XZ_BITS) - 1;
    const Y_MASK: i64 = (1i64 << Self::Y_BITS) - 1;

    #[must_use]
    pub const fn from_raw(raw: i64) -> Self {
        Self(raw)
    }

    #[must_use]
    pub const fn as_raw(self) -> i64 {
        self.0
    }

    #[must_use]
    pub const fn to_section_pos(self) -> SectionPos {
        let x = self.0 >> Self::X_OFFSET;
        let z = (self.0 >> Self::Z_OFFSET) & Self::XZ_MASK;
        let y = self.0 & Self::Y_MASK;

        let x = (x << (64 - Self::XZ_BITS)) >> (64 - Self::XZ_BITS);
        let y = (y << (64 - Self::Y_BITS)) >> (64 - Self::Y_BITS);
        let z = (z << (64 - Self::XZ_BITS)) >> (64 - Self::XZ_BITS);

        SectionPos(IVec3::new(x as i32, y as i32, z as i32))
    }
}

impl From<SectionPos> for PackedSectionPos {
    fn from(pos: SectionPos) -> Self {
        let x = i64::from(pos.0.x);
        let y = i64::from(pos.0.y);
        let z = i64::from(pos.0.z);
        Self(
            ((x & Self::XZ_MASK) << Self::X_OFFSET)
                | ((z & Self::XZ_MASK) << Self::Z_OFFSET)
                | (y & Self::Y_MASK),
        )
    }
}

impl From<PackedSectionPos> for SectionPos {
    fn from(pos: PackedSectionPos) -> Self {
        pos.to_section_pos()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackedChunkLocalXZ(u8);

impl PackedChunkLocalXZ {
    const COORD_MASK: u8 = 0x0f;

    #[must_use]
    pub const fn from_block_pos(pos: BlockPos) -> Self {
        Self::from_local_unchecked(
            (pos.0.x & SectionPos::SECTION_MASK) as u8,
            (pos.0.z & SectionPos::SECTION_MASK) as u8,
        )
    }

    #[must_use]
    pub const fn from_local_xz(x: u8, z: u8) -> Option<Self> {
        if x < 16 && z < 16 {
            Some(Self::from_local_unchecked(x, z))
        } else {
            None
        }
    }

    #[must_use]
    pub const fn from_raw(raw: u8) -> Self {
        Self(raw)
    }

    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self.0
    }

    #[must_use]
    pub const fn x(self) -> u8 {
        (self.0 >> 4) & Self::COORD_MASK
    }

    #[must_use]
    pub const fn z(self) -> u8 {
        self.0 & Self::COORD_MASK
    }

    const fn from_local_unchecked(x: u8, z: u8) -> Self {
        Self((x << 4) | z)
    }
}

impl From<BlockPos> for PackedChunkLocalXZ {
    fn from(pos: BlockPos) -> Self {
        Self::from_block_pos(pos)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackedSectionBlockPos(u16);

impl PackedSectionBlockPos {
    const COORD_MASK: u16 = 0x0f;
    const RAW_MASK: u16 = 0x0fff;

    #[must_use]
    pub const fn from_block_pos(pos: BlockPos) -> Self {
        Self::from_local_unchecked(
            (pos.0.x & SectionPos::SECTION_MASK) as u8,
            (pos.0.y & SectionPos::SECTION_MASK) as u8,
            (pos.0.z & SectionPos::SECTION_MASK) as u8,
        )
    }

    #[must_use]
    pub const fn from_local_xyz(x: u8, y: u8, z: u8) -> Option<Self> {
        if x < 16 && y < 16 && z < 16 {
            Some(Self::from_local_unchecked(x, y, z))
        } else {
            None
        }
    }

    #[must_use]
    pub const fn from_raw(raw: u16) -> Option<Self> {
        if raw & !Self::RAW_MASK == 0 {
            Some(Self(raw))
        } else {
            None
        }
    }

    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self.0
    }

    #[must_use]
    pub const fn x(self) -> u8 {
        ((self.0 >> 8) & Self::COORD_MASK) as u8
    }

    #[must_use]
    pub const fn y(self) -> u8 {
        (self.0 & Self::COORD_MASK) as u8
    }

    #[must_use]
    pub const fn z(self) -> u8 {
        ((self.0 >> 4) & Self::COORD_MASK) as u8
    }

    #[must_use]
    pub const fn to_block_pos(self, section_pos: SectionPos) -> BlockPos {
        section_pos.relative_to_block_pos(self)
    }

    const fn from_local_unchecked(x: u8, y: u8, z: u8) -> Self {
        Self(((x as u16) << 8) | ((z as u16) << 4) | y as u16)
    }
}

impl From<BlockPos> for PackedSectionBlockPos {
    fn from(pos: BlockPos) -> Self {
        Self::from_block_pos(pos)
    }
}

impl TryFrom<u16> for PackedSectionBlockPos {
    type Error = InvalidPackedSectionBlockPos;

    fn try_from(raw: u16) -> Result<Self, Self::Error> {
        Self::from_raw(raw).ok_or(InvalidPackedSectionBlockPos { raw })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidPackedSectionBlockPos {
    raw: u16,
}

impl InvalidPackedSectionBlockPos {
    #[must_use]
    pub const fn raw(self) -> u16 {
        self.raw
    }
}

impl Display for InvalidPackedSectionBlockPos {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "packed section block position {:#06x} uses reserved bits",
            self.raw
        )
    }
}

impl Error for InvalidPackedSectionBlockPos {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_chunk_pos_roundtrips() {
        for (x, z) in [(0, 0), (5, -5), (-1_000_000, 1_000_000)] {
            let pos = ChunkPos::new(x, z);
            assert_eq!(ChunkPos::from(PackedChunkPos::from(pos)), pos);
        }
    }

    #[test]
    fn packed_block_pos_roundtrips_negative_coordinates() {
        for pos in [
            BlockPos::new(0, 0, 0),
            BlockPos::new(-30_000_000, -64, -30_000_000),
            BlockPos::new(30_000_000, 319, 30_000_000),
        ] {
            assert_eq!(BlockPos::from(PackedBlockPos::from(pos)), pos);
        }
    }

    #[test]
    fn packed_section_pos_roundtrips_negative_coordinates() {
        for pos in [
            SectionPos::new(0, 0, 0),
            SectionPos::new(-2_000_000, -4, -2_000_000),
            SectionPos::new(2_000_000, 20, 2_000_000),
        ] {
            assert_eq!(SectionPos::from(PackedSectionPos::from(pos)), pos);
        }
    }

    #[test]
    fn section_relative_block_pos_roundtrips() {
        let section = SectionPos::new(3, -2, 7);
        for local in [
            BlockPos::new(48, -32, 112),
            BlockPos::new(48 + 15, -32 + 15, 112 + 15),
        ] {
            let packed = SectionPos::section_relative_pos(local);
            assert_eq!(packed.to_block_pos(section), local);
        }
    }

    #[test]
    fn block_pos_directional_offsets_are_unit_steps() {
        let origin = BlockPos::ZERO;
        assert_eq!(origin.above(), BlockPos::new(0, 1, 0));
        assert_eq!(origin.below(), BlockPos::new(0, -1, 0));
        assert_eq!(origin.north(), BlockPos::new(0, 0, -1));
        assert_eq!(origin.south(), BlockPos::new(0, 0, 1));
        assert_eq!(origin.west(), BlockPos::new(-1, 0, 0));
        assert_eq!(origin.east(), BlockPos::new(1, 0, 0));
    }

    #[test]
    fn chunk_pos_neighbors_excludes_self() {
        let center = ChunkPos::new(0, 0);
        let neighbors = center.neighbors();
        assert_eq!(neighbors.len(), 8);
        assert!(!neighbors.contains(&center));
    }

    #[test]
    fn within_manhattan_yields_origin_first() {
        let mut iter = BlockPos::ZERO.within_manhattan(1, 1, 1);
        assert_eq!(iter.next(), Some(BlockPos::ZERO));
    }
}
