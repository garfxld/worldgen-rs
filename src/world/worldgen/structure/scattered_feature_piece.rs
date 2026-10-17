use crate::world::chunk::heightmap::HeightmapType;
use crate::world::worldgen::region::WorldGenRegion;
use crate::world::worldgen::structure::structure_piece::PieceBase;
use shulkr::util::BlockPosition;
use shulkr::util::BoundingBox;
use shulkr::util::Direction;

#[derive(Debug)]
pub struct ScatteredFeature {
    pub width: i32,
    pub height: i32,
    pub depth: i32,
    pub height_position: i32,
}

impl ScatteredFeature {
    pub const fn new(width: i32, height: i32, depth: i32) -> Self {
        Self {
            width,
            height,
            depth,
            height_position: -1,
        }
    }

    pub const fn make_base(
        west: i32,
        floor: i32,
        north: i32,
        width: i32,
        height: i32,
        depth: i32,
        direction: Direction,
    ) -> PieceBase {
        let mut base = PieceBase::new(
            0,
            PieceBase::make_bounding_box(west, floor, north, direction, width, height, depth),
        );
        base.set_orientation(Some(direction));
        base
    }

    pub fn update_average_ground_height(
        &mut self,
        base: &mut PieceBase,
        region: &WorldGenRegion<'_>,
        chunk_bb: BoundingBox,
        offset: i32,
    ) -> bool {
        if self.height_position >= 0 {
            return true;
        }

        let mut total = 0;
        let mut count = 0;
        let box_ = base.bounding_box;
        for z in box_.min_z()..=box_.max_z() {
            for x in box_.min_x()..=box_.max_x() {
                if chunk_bb.is_inside(BlockPosition::new(x, 64, z)) {
                    total += region.get_height(HeightmapType::MotionBlockingNoLeaves, x, z);
                    count += 1;
                }
            }
        }

        if count == 0 {
            return false;
        }

        self.height_position = total / count;
        base.move_by(0, self.height_position - box_.min_y() + offset, 0);
        true
    }

    pub fn update_height_position_to_lowest_ground_height(
        &mut self,
        base: &mut PieceBase,
        region: &WorldGenRegion<'_>,
        offset: i32,
    ) -> bool {
        if self.height_position >= 0 {
            return true;
        }

        let mut lowest_ground_height = region.max_y() + 1;
        let mut found = false;
        let box_ = base.bounding_box;
        for z in box_.min_z()..=box_.max_z() {
            for x in box_.min_x()..=box_.max_x() {
                lowest_ground_height = lowest_ground_height.min(region.get_height(
                    HeightmapType::MotionBlockingNoLeaves,
                    x,
                    z,
                ));
                found = true;
            }
        }

        if !found {
            return false;
        }

        self.height_position = lowest_ground_height;
        base.move_by(0, self.height_position - box_.min_y() + offset, 0);
        true
    }
}
