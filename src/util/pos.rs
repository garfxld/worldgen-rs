use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::hash::{Hash, Hasher};

use glam::{IVec2, IVec3};
use shulkr::util::{BlockPosition, Key};

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
        SectionPos::block_to_section_coord((1 << PackedBlockPos::HORIZONTAL_BITS) / 2 - 1);

    pub fn neighbors(self) -> [ChunkPos; 8] {
        Self::OFFSETS.map(|(dx, dy)| ChunkPos::new(self.0.x + dx, self.0.y + dy))
    }

    pub const fn new(x: i32, y: i32) -> Self {
        Self(IVec2::new(x, y))
    }

    pub const fn from_block_pos(pos: BlockPosition) -> Self {
        Self::new(
            SectionPos::block_to_section_coord(pos.x()),
            SectionPos::block_to_section_coord(pos.z()),
        )
    }

    pub const fn is_valid(x: i32, z: i32) -> bool {
        x.abs().max(z.abs()) <= Self::MAX_COORDINATE_VALUE
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GlobalPos {
    pub dimension: Key,
    pub pos: BlockPosition,
}

impl GlobalPos {
    pub const fn new(dimension: Key, pos: BlockPosition) -> Self {
        Self { dimension, pos }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SectionPos(pub IVec3);

impl SectionPos {
    const SECTION_BITS: i32 = 4;
    const SECTION_SIZE: i32 = 1 << Self::SECTION_BITS;
    pub(super) const SECTION_MASK: i32 = Self::SECTION_SIZE - 1;

    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        Self(IVec3::new(x, y, z))
    }

    pub const fn block_to_section_coord(block_coord: i32) -> i32 {
        block_coord >> Self::SECTION_BITS
    }

    pub const fn from_block_pos(pos: BlockPosition) -> Self {
        Self::new(
            Self::block_to_section_coord(pos.x()),
            Self::block_to_section_coord(pos.y()),
            Self::block_to_section_coord(pos.z()),
        )
    }

    pub const fn x(&self) -> i32 {
        self.0.x
    }

    pub const fn y(&self) -> i32 {
        self.0.y
    }

    pub const fn z(&self) -> i32 {
        self.0.z
    }

    pub const fn relative_to_block_x(&self, relative: PackedSectionBlockPos) -> i32 {
        (self.0.x << Self::SECTION_BITS) + relative.x() as i32
    }

    pub const fn relative_to_block_y(&self, relative: PackedSectionBlockPos) -> i32 {
        (self.0.y << Self::SECTION_BITS) + relative.y() as i32
    }

    pub const fn relative_to_block_z(&self, relative: PackedSectionBlockPos) -> i32 {
        (self.0.z << Self::SECTION_BITS) + relative.z() as i32
    }

    pub const fn section_relative_pos(pos: BlockPosition) -> PackedSectionBlockPos {
        PackedSectionBlockPos::from_block_pos(pos)
    }

    pub const fn relative_to_block_pos(&self, relative: PackedSectionBlockPos) -> BlockPosition {
        BlockPosition::new(
            self.relative_to_block_x(relative),
            self.relative_to_block_y(relative),
            self.relative_to_block_z(relative),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackedChunkPos(i64);

impl PackedChunkPos {
    pub const fn from_raw(raw: i64) -> Self {
        Self(raw)
    }

    pub const fn as_raw(self) -> i64 {
        self.0
    }

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

    pub const fn from_raw(raw: i64) -> Self {
        Self(raw)
    }

    pub const fn as_raw(self) -> i64 {
        self.0
    }

    pub const fn to_block_pos(self) -> BlockPosition {
        let x = self.0 >> Self::X_OFFSET;
        let y = self.0 & Self::Y_MASK;
        let z = (self.0 >> Self::Z_OFFSET) & Self::XZ_MASK;

        let x = (x << (64 - Self::HORIZONTAL_BITS)) >> (64 - Self::HORIZONTAL_BITS);
        let y = (y << (64 - Self::Y_BITS)) >> (64 - Self::Y_BITS);
        let z = (z << (64 - Self::HORIZONTAL_BITS)) >> (64 - Self::HORIZONTAL_BITS);

        BlockPosition::new(x as i32, y as i32, z as i32)
    }
}

impl From<BlockPosition> for PackedBlockPos {
    fn from(pos: BlockPosition) -> Self {
        let x = i64::from(pos.x());
        let y = i64::from(pos.y());
        let z = i64::from(pos.z());
        Self(
            ((x & Self::XZ_MASK) << Self::X_OFFSET)
                | ((z & Self::XZ_MASK) << Self::Z_OFFSET)
                | (y & Self::Y_MASK),
        )
    }
}

impl From<PackedBlockPos> for BlockPosition {
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

    pub const fn from_raw(raw: i64) -> Self {
        Self(raw)
    }

    pub const fn as_raw(self) -> i64 {
        self.0
    }

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
        let x = i64::from(pos.x());
        let y = i64::from(pos.y());
        let z = i64::from(pos.z());
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

    pub const fn from_block_pos(pos: BlockPosition) -> Self {
        Self::from_local_unchecked(
            (pos.x() & SectionPos::SECTION_MASK) as u8,
            (pos.z() & SectionPos::SECTION_MASK) as u8,
        )
    }

    pub const fn from_raw(raw: u8) -> Self {
        Self(raw)
    }

    pub const fn x(self) -> u8 {
        (self.0 >> 4) & Self::COORD_MASK
    }

    pub const fn z(self) -> u8 {
        self.0 & Self::COORD_MASK
    }

    const fn from_local_unchecked(x: u8, z: u8) -> Self {
        Self((x << 4) | z)
    }
}

impl From<BlockPosition> for PackedChunkLocalXZ {
    fn from(pos: BlockPosition) -> Self {
        Self::from_block_pos(pos)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackedSectionBlockPos(u16);

impl PackedSectionBlockPos {
    const COORD_MASK: u16 = 0x0f;
    const RAW_MASK: u16 = 0x0fff;

    pub const fn from_block_pos(pos: BlockPosition) -> Self {
        Self::from_local_unchecked(
            (pos.x() & SectionPos::SECTION_MASK) as u8,
            (pos.y() & SectionPos::SECTION_MASK) as u8,
            (pos.z() & SectionPos::SECTION_MASK) as u8,
        )
    }

    pub const fn from_raw(raw: u16) -> Option<Self> {
        if raw & !Self::RAW_MASK == 0 {
            Some(Self(raw))
        } else {
            None
        }
    }

    pub const fn x(self) -> u8 {
        ((self.0 >> 8) & Self::COORD_MASK) as u8
    }

    pub const fn y(self) -> u8 {
        (self.0 & Self::COORD_MASK) as u8
    }

    pub const fn z(self) -> u8 {
        ((self.0 >> 4) & Self::COORD_MASK) as u8
    }

    pub const fn to_block_pos(self, section_pos: SectionPos) -> BlockPosition {
        section_pos.relative_to_block_pos(self)
    }

    const fn from_local_unchecked(x: u8, y: u8, z: u8) -> Self {
        Self(((x as u16) << 8) | ((z as u16) << 4) | y as u16)
    }
}

impl From<BlockPosition> for PackedSectionBlockPos {
    fn from(pos: BlockPosition) -> Self {
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
            BlockPosition::new(0, 0, 0),
            BlockPosition::new(-30_000_000, -64, -30_000_000),
            BlockPosition::new(30_000_000, 319, 30_000_000),
        ] {
            assert_eq!(BlockPosition::from(PackedBlockPos::from(pos)), pos);
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
            BlockPosition::new(48, -32, 112),
            BlockPosition::new(48 + 15, -32 + 15, 112 + 15),
        ] {
            let packed = SectionPos::section_relative_pos(local);
            assert_eq!(packed.to_block_pos(section), local);
        }
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
        let mut iter = BlockPosition::ZERO.within_manhattan(1, 1, 1);
        assert_eq!(iter.next(), Some(BlockPosition::ZERO));
    }
}
