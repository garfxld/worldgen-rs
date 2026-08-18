use crate::registry::blocks::properties::{Properties, StairsShape};
use crate::registry::blocks::vanilla_blocks;
use crate::util::direction::Direction;
use crate::util::geometry::BoundingBox;
use crate::util::rng::RandomSource;
use crate::worldgen::region::WorldGenRegion;
use crate::worldgen::structure::scattered_feature_piece::ScatteredFeature;
use crate::worldgen::structure::structure_piece::{PieceBase, PieceKind, StructurePiece, air};

#[derive(Debug)]
pub struct SwampHutPiece {
    pub scattered: ScatteredFeature,
}

impl SwampHutPiece {
    #[expect(
        clippy::new_ret_no_self,
        reason = "mirrors the vanilla piece constructor"
    )]
    pub fn new(random: &mut RandomSource, west: i32, north: i32) -> StructurePiece {
        let direction = Direction::get_random_horizontal(random);
        let base = ScatteredFeature::make_base(west, 64, north, 7, 7, 9, direction);
        StructurePiece::new(
            base,
            PieceKind::SwampHut(Self {
                scattered: ScatteredFeature::new(7, 7, 9),
            }),
        )
    }

    pub fn post_process(
        &mut self,
        base: &mut PieceBase,
        region: &WorldGenRegion<'_>,
        chunk_bb: BoundingBox,
    ) {
        if !self
            .scattered
            .update_average_ground_height(base, region, chunk_bb, 0)
        {
            return;
        }

        let planks = vanilla_blocks::SPRUCE_PLANKS.default_state();
        let log = vanilla_blocks::OAK_LOG.default_state();
        let fence = vanilla_blocks::OAK_FENCE.default_state();
        let air = air();

        base.generate_box(region, chunk_bb, 1, 1, 1, 5, 1, 7, planks, planks, false);
        base.generate_box(region, chunk_bb, 1, 4, 2, 5, 4, 7, planks, planks, false);
        base.generate_box(region, chunk_bb, 2, 1, 0, 4, 1, 0, planks, planks, false);
        base.generate_box(region, chunk_bb, 2, 2, 2, 3, 3, 2, planks, planks, false);
        base.generate_box(region, chunk_bb, 1, 2, 3, 1, 3, 6, planks, planks, false);
        base.generate_box(region, chunk_bb, 5, 2, 3, 5, 3, 6, planks, planks, false);
        base.generate_box(region, chunk_bb, 2, 2, 7, 4, 3, 7, planks, planks, false);
        base.generate_box(region, chunk_bb, 1, 0, 2, 1, 3, 2, log, log, false);
        base.generate_box(region, chunk_bb, 5, 0, 2, 5, 3, 2, log, log, false);
        base.generate_box(region, chunk_bb, 1, 0, 7, 1, 3, 7, log, log, false);
        base.generate_box(region, chunk_bb, 5, 0, 7, 5, 3, 7, log, log, false);
        base.place_block(region, fence, 2, 3, 2, chunk_bb);
        base.place_block(region, fence, 3, 3, 7, chunk_bb);
        base.place_block(region, air, 1, 3, 4, chunk_bb);
        base.place_block(region, air, 5, 3, 4, chunk_bb);
        base.place_block(region, air, 5, 3, 5, chunk_bb);
        base.place_block(
            region,
            vanilla_blocks::POTTED_RED_MUSHROOM.default_state(),
            1,
            3,
            5,
            chunk_bb,
        );
        base.place_block(
            region,
            vanilla_blocks::CRAFTING_TABLE.default_state(),
            3,
            2,
            6,
            chunk_bb,
        );
        base.place_block(
            region,
            vanilla_blocks::CAULDRON.default_state(),
            4,
            2,
            6,
            chunk_bb,
        );
        base.place_block(region, fence, 1, 2, 1, chunk_bb);
        base.place_block(region, fence, 5, 2, 1, chunk_bb);

        let stairs = vanilla_blocks::SPRUCE_STAIRS.default_state();
        let north_stairs = stairs.set_value(&Properties::HORIZONTAL_FACING, Direction::North);
        let east_stairs = stairs.set_value(&Properties::HORIZONTAL_FACING, Direction::East);
        let west_stairs = stairs.set_value(&Properties::HORIZONTAL_FACING, Direction::West);
        let south_stairs = stairs.set_value(&Properties::HORIZONTAL_FACING, Direction::South);
        base.generate_box(
            region,
            chunk_bb,
            0,
            4,
            1,
            6,
            4,
            1,
            north_stairs,
            north_stairs,
            false,
        );
        base.generate_box(
            region,
            chunk_bb,
            0,
            4,
            2,
            0,
            4,
            7,
            east_stairs,
            east_stairs,
            false,
        );
        base.generate_box(
            region,
            chunk_bb,
            6,
            4,
            2,
            6,
            4,
            7,
            west_stairs,
            west_stairs,
            false,
        );
        base.generate_box(
            region,
            chunk_bb,
            0,
            4,
            8,
            6,
            4,
            8,
            south_stairs,
            south_stairs,
            false,
        );
        base.place_block(
            region,
            north_stairs.set_value(&Properties::STAIRS_SHAPE, StairsShape::OuterRight),
            0,
            4,
            1,
            chunk_bb,
        );
        base.place_block(
            region,
            north_stairs.set_value(&Properties::STAIRS_SHAPE, StairsShape::OuterLeft),
            6,
            4,
            1,
            chunk_bb,
        );
        base.place_block(
            region,
            south_stairs.set_value(&Properties::STAIRS_SHAPE, StairsShape::OuterLeft),
            0,
            4,
            8,
            chunk_bb,
        );
        base.place_block(
            region,
            south_stairs.set_value(&Properties::STAIRS_SHAPE, StairsShape::OuterRight),
            6,
            4,
            8,
            chunk_bb,
        );

        let mut z = 2;
        while z <= 7 {
            let mut x = 1;
            while x <= 5 {
                base.fill_column_down(region, log, x, -1, z, chunk_bb);
                x += 4;
            }
            z += 5;
        }
    }
}
