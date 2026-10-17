use shulkr::world::block::BlockState;

use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util::BlockPosition;

#[derive(Debug, Clone, Copy)]
pub struct Column {
    floor: Option<i32>,
    ceiling: Option<i32>,
}

impl Column {
    pub const fn create(floor: Option<i32>, ceiling: Option<i32>) -> Self {
        Self { floor, ceiling }
    }

    pub const fn floor(self) -> Option<i32> {
        self.floor
    }

    pub const fn ceiling(self) -> Option<i32> {
        self.ceiling
    }

    pub const fn height(self) -> Option<i32> {
        match (self.floor, self.ceiling) {
            (Some(floor), Some(ceiling)) => Some(ceiling - floor - 1),
            _ => None,
        }
    }

    pub const fn with_floor(self, floor: Option<i32>) -> Self {
        Self::create(floor, self.ceiling)
    }

    pub fn scan(
        region: &WorldGenRegion<'_>,
        pos: BlockPosition,
        search_range: i32,
        inside_column: impl Fn(BlockState) -> bool,
        valid_edge: impl Fn(BlockState) -> bool,
    ) -> Option<Self> {
        if !inside_column(region.get_block(pos)) {
            return None;
        }

        let ceiling = scan_direction(region, search_range, &inside_column, &valid_edge, pos, true);
        let floor = scan_direction(
            region,
            search_range,
            &inside_column,
            &valid_edge,
            pos,
            false,
        );
        Some(Self::create(floor, ceiling))
    }
}

fn scan_direction(
    region: &WorldGenRegion<'_>,
    search_range: i32,
    inside_column: &impl Fn(BlockState) -> bool,
    valid_edge: &impl Fn(BlockState) -> bool,
    start: BlockPosition,
    up: bool,
) -> Option<i32> {
    let mut pos = start;
    let mut i = 1;
    while i < search_range && inside_column(region.get_block(pos)) {
        pos = if up { pos.above() } else { pos.below() };
        i += 1;
    }

    if valid_edge(region.get_block(pos)) {
        Some(pos.y())
    } else {
        None
    }
}
