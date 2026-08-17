use crate::registry::blocks::properties::Properties;
use crate::registry::blocks::vanilla_blocks;
use crate::registry::blocks::transform;
use crate::util::direction::Direction;
use crate::util::geometry::BoundingBox;
use crate::util::ident::BlockState;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::util::rng::legacy_random::LegacyRandomSource;
use crate::util::rotation::Rotation;
use crate::worldgen::region::WorldGenRegion;
use crate::worldgen::structure::scattered_feature_piece::ScatteredFeature;
use crate::worldgen::structure::structure_piece::{PieceBase, PieceKind, StructurePiece, air};

pub const WIDTH: i32 = 21;
pub const DEPTH: i32 = 21;

#[derive(Debug)]
pub struct DesertPyramidPiece {
    pub scattered: ScatteredFeature,
    has_placed_chest: [bool; 4],
    potential_suspicious_sand_world_positions: Vec<BlockPos>,
    random_collapsed_roof_pos: BlockPos,
}

impl DesertPyramidPiece {
    #[expect(clippy::new_ret_no_self, reason = "mirrors the vanilla piece constructor")]
    pub fn new(random: &mut RandomSource, west: i32, north: i32) -> StructurePiece {
        let direction = Direction::get_random_horizontal(random);
        let base = ScatteredFeature::make_base(west, 64, north, WIDTH, 15, DEPTH, direction);
        StructurePiece::new(
            base,
            PieceKind::DesertPyramid(Self {
                scattered: ScatteredFeature::new(WIDTH, 15, DEPTH),
                has_placed_chest: [false; 4],
                potential_suspicious_sand_world_positions: Vec::new(),
                random_collapsed_roof_pos: BlockPos::new(0, 0, 0),
            }),
        )
    }

    pub fn potential_suspicious_sand_world_positions(&self) -> &[BlockPos] {
        &self.potential_suspicious_sand_world_positions
    }

    pub const fn random_collapsed_roof_pos(&self) -> BlockPos {
        self.random_collapsed_roof_pos
    }

    #[expect(
        clippy::too_many_lines,
        reason = "mirrors vanilla DesertPyramidPiece::postProcess"
    )]
    pub fn post_process(
        &mut self,
        base: &mut PieceBase,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        chunk_bb: BoundingBox,
    ) {
        let offset = -random.next_i32_bounded(3);
        if !self
            .scattered
            .update_height_position_to_lowest_ground_height(base, region, offset)
        {
            return;
        }

        let width = self.scattered.width;
        let depth = self.scattered.depth;
        let sandstone = vanilla_blocks::SANDSTONE.default_state();
        let cut = vanilla_blocks::CUT_SANDSTONE.default_state();
        let chiseled = vanilla_blocks::CHISELED_SANDSTONE.default_state();
        let orange = vanilla_blocks::ORANGE_TERRACOTTA.default_state();
        let blue = vanilla_blocks::BLUE_TERRACOTTA.default_state();
        let air = air();

        let piece = &*base;
        let place = |state: BlockState, x: i32, y: i32, z: i32| {
            piece.place_block(region, state, x, y, z, chunk_bb);
        };
        let gbox = |x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32, e: BlockState, f: BlockState| {
            piece.generate_box(region, chunk_bb, x0, y0, z0, x1, y1, z1, e, f, false);
        };

        gbox(0, -4, 0, width - 1, 0, depth - 1, sandstone, sandstone);

        for pos in 1..=9 {
            gbox(
                pos,
                pos,
                pos,
                width - 1 - pos,
                pos,
                depth - 1 - pos,
                sandstone,
                sandstone,
            );
            gbox(
                pos + 1,
                pos,
                pos + 1,
                width - 2 - pos,
                pos,
                depth - 2 - pos,
                air,
                air,
            );
        }

        for x in 0..width {
            for z in 0..depth {
                piece.fill_column_down(region, sandstone, x, -5, z, chunk_bb);
            }
        }

        let stairs = vanilla_blocks::SANDSTONE_STAIRS.default_state();
        let north_stairs = stairs.set_value(&Properties::HORIZONTAL_FACING, Direction::North);
        let south_stairs = stairs.set_value(&Properties::HORIZONTAL_FACING, Direction::South);
        let east_stairs = stairs.set_value(&Properties::HORIZONTAL_FACING, Direction::East);
        let west_stairs = stairs.set_value(&Properties::HORIZONTAL_FACING, Direction::West);

        gbox(0, 0, 0, 4, 9, 4, sandstone, air);
        gbox(1, 10, 1, 3, 10, 3, sandstone, sandstone);
        place(north_stairs, 2, 10, 0);
        place(south_stairs, 2, 10, 4);
        place(east_stairs, 0, 10, 2);
        place(west_stairs, 4, 10, 2);
        gbox(width - 5, 0, 0, width - 1, 9, 4, sandstone, air);
        gbox(width - 4, 10, 1, width - 2, 10, 3, sandstone, sandstone);
        place(north_stairs, width - 3, 10, 0);
        place(south_stairs, width - 3, 10, 4);
        place(east_stairs, width - 5, 10, 2);
        place(west_stairs, width - 1, 10, 2);
        gbox(8, 0, 0, 12, 4, 4, sandstone, air);
        gbox(9, 1, 0, 11, 3, 4, air, air);
        place(cut, 9, 1, 1);
        place(cut, 9, 2, 1);
        place(cut, 9, 3, 1);
        place(cut, 10, 3, 1);
        place(cut, 11, 3, 1);
        place(cut, 11, 2, 1);
        place(cut, 11, 1, 1);
        gbox(4, 1, 1, 8, 3, 3, sandstone, air);
        gbox(4, 1, 2, 8, 2, 2, air, air);
        gbox(12, 1, 1, 16, 3, 3, sandstone, air);
        gbox(12, 1, 2, 16, 2, 2, air, air);
        gbox(5, 4, 5, width - 6, 4, depth - 6, sandstone, sandstone);
        gbox(9, 4, 9, 11, 4, 11, air, air);
        gbox(8, 1, 8, 8, 3, 8, cut, cut);
        gbox(12, 1, 8, 12, 3, 8, cut, cut);
        gbox(8, 1, 12, 8, 3, 12, cut, cut);
        gbox(12, 1, 12, 12, 3, 12, cut, cut);
        gbox(1, 1, 5, 4, 4, 11, sandstone, sandstone);
        gbox(width - 5, 1, 5, width - 2, 4, 11, sandstone, sandstone);
        gbox(6, 7, 9, 6, 7, 11, sandstone, sandstone);
        gbox(width - 7, 7, 9, width - 7, 7, 11, sandstone, sandstone);
        gbox(5, 5, 9, 5, 7, 11, cut, cut);
        gbox(width - 6, 5, 9, width - 6, 7, 11, cut, cut);
        place(air, 5, 5, 10);
        place(air, 5, 6, 10);
        place(air, 6, 6, 10);
        place(air, width - 6, 5, 10);
        place(air, width - 6, 6, 10);
        place(air, width - 7, 6, 10);
        gbox(2, 4, 4, 2, 6, 4, air, air);
        gbox(width - 3, 4, 4, width - 3, 6, 4, air, air);
        place(north_stairs, 2, 4, 5);
        place(north_stairs, 2, 3, 4);
        place(north_stairs, width - 3, 4, 5);
        place(north_stairs, width - 3, 3, 4);
        gbox(1, 1, 3, 2, 2, 3, sandstone, sandstone);
        gbox(width - 3, 1, 3, width - 2, 2, 3, sandstone, sandstone);
        place(sandstone, 1, 1, 2);
        place(sandstone, width - 2, 1, 2);
        place(vanilla_blocks::SANDSTONE_SLAB.default_state(), 1, 2, 2);
        place(vanilla_blocks::SANDSTONE_SLAB.default_state(), width - 2, 2, 2);
        place(west_stairs, 2, 1, 2);
        place(east_stairs, width - 3, 1, 2);
        gbox(4, 3, 5, 4, 3, 17, sandstone, sandstone);
        gbox(width - 5, 3, 5, width - 5, 3, 17, sandstone, sandstone);
        gbox(3, 1, 5, 4, 2, 16, air, air);
        gbox(width - 6, 1, 5, width - 5, 2, 16, air, air);

        let mut z = 5;
        while z <= 17 {
            place(cut, 4, 1, z);
            place(chiseled, 4, 2, z);
            place(cut, width - 5, 1, z);
            place(chiseled, width - 5, 2, z);
            z += 2;
        }

        place(orange, 10, 0, 7);
        place(orange, 10, 0, 8);
        place(orange, 9, 0, 9);
        place(orange, 11, 0, 9);
        place(orange, 8, 0, 10);
        place(orange, 12, 0, 10);
        place(orange, 7, 0, 10);
        place(orange, 13, 0, 10);
        place(orange, 9, 0, 11);
        place(orange, 11, 0, 11);
        place(orange, 10, 0, 12);
        place(orange, 10, 0, 13);
        place(blue, 10, 0, 10);

        let mut x = 0;
        while x < width {
            place(cut, x, 2, 1);
            place(orange, x, 2, 2);
            place(cut, x, 2, 3);
            place(cut, x, 3, 1);
            place(orange, x, 3, 2);
            place(cut, x, 3, 3);
            place(orange, x, 4, 1);
            place(chiseled, x, 4, 2);
            place(orange, x, 4, 3);
            place(cut, x, 5, 1);
            place(orange, x, 5, 2);
            place(cut, x, 5, 3);
            place(orange, x, 6, 1);
            place(chiseled, x, 6, 2);
            place(orange, x, 6, 3);
            place(orange, x, 7, 1);
            place(orange, x, 7, 2);
            place(orange, x, 7, 3);
            place(cut, x, 8, 1);
            place(cut, x, 8, 2);
            place(cut, x, 8, 3);
            x += width - 1;
        }

        let mut x = 2;
        while x <= width - 3 {
            place(cut, x - 1, 2, 0);
            place(orange, x, 2, 0);
            place(cut, x + 1, 2, 0);
            place(cut, x - 1, 3, 0);
            place(orange, x, 3, 0);
            place(cut, x + 1, 3, 0);
            place(orange, x - 1, 4, 0);
            place(chiseled, x, 4, 0);
            place(orange, x + 1, 4, 0);
            place(cut, x - 1, 5, 0);
            place(orange, x, 5, 0);
            place(cut, x + 1, 5, 0);
            place(orange, x - 1, 6, 0);
            place(chiseled, x, 6, 0);
            place(orange, x + 1, 6, 0);
            place(orange, x - 1, 7, 0);
            place(orange, x, 7, 0);
            place(orange, x + 1, 7, 0);
            place(cut, x - 1, 8, 0);
            place(cut, x, 8, 0);
            place(cut, x + 1, 8, 0);
            x += width - 3 - 2;
        }

        gbox(8, 4, 0, 12, 6, 0, cut, cut);
        place(air, 8, 6, 0);
        place(air, 12, 6, 0);
        place(orange, 9, 5, 0);
        place(chiseled, 10, 5, 0);
        place(orange, 11, 5, 0);
        gbox(8, -14, 8, 12, -11, 12, cut, cut);
        gbox(8, -10, 8, 12, -10, 12, chiseled, chiseled);
        gbox(8, -9, 8, 12, -9, 12, cut, cut);
        gbox(8, -8, 8, 12, -1, 12, sandstone, sandstone);
        gbox(9, -11, 9, 11, -1, 11, air, air);
        place(vanilla_blocks::STONE_PRESSURE_PLATE.default_state(), 10, -11, 10);
        gbox(9, -13, 9, 11, -13, 11, vanilla_blocks::TNT.default_state(), air);
        place(air, 8, -11, 10);
        place(air, 8, -10, 10);
        place(chiseled, 7, -10, 10);
        place(cut, 7, -11, 10);
        place(air, 12, -11, 10);
        place(air, 12, -10, 10);
        place(chiseled, 13, -10, 10);
        place(cut, 13, -11, 10);
        place(air, 10, -11, 8);
        place(air, 10, -10, 8);
        place(chiseled, 10, -10, 7);
        place(cut, 10, -11, 7);
        place(air, 10, -11, 12);
        place(air, 10, -10, 12);
        place(chiseled, 10, -10, 13);
        place(cut, 10, -11, 13);

        for direction in Direction::HORIZONTAL {
            let index = direction.get_2d_data_value() as usize;
            if !self.has_placed_chest[index] {
                let (xo, _, zo) = direction.offset();
                self.has_placed_chest[index] = piece.create_chest(
                    region,
                    chunk_bb,
                    random,
                    10 + xo * 2,
                    -11,
                    10 + zo * 2,
                );
            }
        }

        self.add_cellar(base, region, chunk_bb);
    }

    fn add_cellar(
        &mut self,
        base: &PieceBase,
        region: &WorldGenRegion<'_>,
        chunk_bb: BoundingBox,
    ) {
        let room_center = BlockPos::new(16, -4, 13);
        self.add_cellar_stairs(room_center, base, region, chunk_bb);
        self.add_cellar_room(room_center, base, region, chunk_bb);
    }

    fn add_cellar_stairs(
        &mut self,
        room_center: BlockPos,
        base: &PieceBase,
        region: &WorldGenRegion<'_>,
        chunk_bb: BoundingBox,
    ) {
        let (x, y, z) = (room_center.x(), room_center.y(), room_center.z());
        let stairs = vanilla_blocks::SANDSTONE_STAIRS.default_state();
        let turned = transform::rotate(stairs, Rotation::CounterClockwise90);
        base.place_block(region, turned, 13, -1, 17, chunk_bb);
        base.place_block(region, turned, 14, -2, 17, chunk_bb);
        base.place_block(region, turned, 15, -3, 17, chunk_bb);

        let sand = vanilla_blocks::SAND.default_state();
        let sandstone = vanilla_blocks::SANDSTONE.default_state();
        let variant = region.with_random(RandomSource::next_bool);
        base.place_block(region, sand, x - 4, y + 4, z + 4, chunk_bb);
        base.place_block(region, sand, x - 3, y + 4, z + 4, chunk_bb);
        base.place_block(region, sand, x - 2, y + 4, z + 4, chunk_bb);
        base.place_block(region, sand, x - 1, y + 4, z + 4, chunk_bb);
        base.place_block(region, sand, x, y + 4, z + 4, chunk_bb);
        base.place_block(region, sand, x - 2, y + 3, z + 4, chunk_bb);
        base.place_block(
            region,
            if variant { sand } else { sandstone },
            x - 1,
            y + 3,
            z + 4,
            chunk_bb,
        );
        base.place_block(
            region,
            if variant { sandstone } else { sand },
            x,
            y + 3,
            z + 4,
            chunk_bb,
        );
        base.place_block(region, sand, x - 1, y + 2, z + 4, chunk_bb);
        base.place_block(region, sandstone, x, y + 2, z + 4, chunk_bb);
        base.place_block(region, sand, x, y + 1, z + 4, chunk_bb);
    }

    fn add_cellar_room(
        &mut self,
        room_center: BlockPos,
        base: &PieceBase,
        region: &WorldGenRegion<'_>,
        chunk_bb: BoundingBox,
    ) {
        let (x, y, z) = (room_center.x(), room_center.y(), room_center.z());
        let cut = vanilla_blocks::CUT_SANDSTONE.default_state();
        let chiseled = vanilla_blocks::CHISELED_SANDSTONE.default_state();

        let gbox = |x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32, state: BlockState| {
            base.generate_box(region, chunk_bb, x0, y0, z0, x1, y1, z1, state, state, true);
        };
        gbox(x - 3, y + 1, z - 3, x - 3, y + 1, z + 2, cut);
        gbox(x + 3, y + 1, z - 3, x + 3, y + 1, z + 2, cut);
        gbox(x - 3, y + 1, z - 3, x + 3, y + 1, z - 2, cut);
        gbox(x - 3, y + 1, z + 3, x + 3, y + 1, z + 3, cut);
        gbox(x - 3, y + 2, z - 3, x - 3, y + 2, z + 2, chiseled);
        gbox(x + 3, y + 2, z - 3, x + 3, y + 2, z + 2, chiseled);
        gbox(x - 3, y + 2, z - 3, x + 3, y + 2, z - 2, chiseled);
        gbox(x - 3, y + 2, z + 3, x + 3, y + 2, z + 3, chiseled);
        gbox(x - 3, -1, z - 3, x - 3, -1, z + 2, cut);
        gbox(x + 3, -1, z - 3, x + 3, -1, z + 2, cut);
        gbox(x - 3, -1, z - 3, x + 3, -1, z - 2, cut);
        gbox(x - 3, -1, z + 3, x + 3, -1, z + 3, cut);

        self.place_sand_box(base, x - 2, y + 1, z - 2, x + 2, y + 3, z + 2);
        self.place_collapsed_roof(base, region, chunk_bb, x - 2, y + 4, z - 2, x + 2, z + 2);

        let orange = vanilla_blocks::ORANGE_TERRACOTTA.default_state();
        let blue = vanilla_blocks::BLUE_TERRACOTTA.default_state();
        let place = |state: BlockState, px: i32, py: i32, pz: i32| {
            base.place_block(region, state, px, py, pz, chunk_bb);
        };
        place(blue, x, y, z);
        place(orange, x + 1, y, z - 1);
        place(orange, x + 1, y, z + 1);
        place(orange, x - 1, y, z - 1);
        place(orange, x - 1, y, z + 1);
        place(orange, x + 2, y, z);
        place(orange, x - 2, y, z);
        place(orange, x, y, z + 2);
        place(orange, x, y, z - 2);
        place(orange, x + 3, y, z);
        self.place_sand(base, x + 3, y + 1, z);
        self.place_sand(base, x + 3, y + 2, z);
        place(cut, x + 4, y + 1, z);
        place(chiseled, x + 4, y + 2, z);
        place(orange, x - 3, y, z);
        self.place_sand(base, x - 3, y + 1, z);
        self.place_sand(base, x - 3, y + 2, z);
        place(cut, x - 4, y + 1, z);
        place(chiseled, x - 4, y + 2, z);
        place(orange, x, y, z + 3);
        self.place_sand(base, x, y + 1, z + 3);
        self.place_sand(base, x, y + 2, z + 3);
        place(orange, x, y, z - 3);
        self.place_sand(base, x, y + 1, z - 3);
        self.place_sand(base, x, y + 2, z - 3);
        place(cut, x, y + 1, z - 4);
        place(chiseled, x, -2, z - 4);
    }

    fn place_sand(&mut self, base: &PieceBase, x: i32, y: i32, z: i32) {
        self.potential_suspicious_sand_world_positions
            .push(base.world_pos(x, y, z));
    }

    #[expect(clippy::too_many_arguments, reason = "mirrors vanilla placeSandBox")]
    fn place_sand_box(
        &mut self,
        base: &PieceBase,
        x0: i32,
        y0: i32,
        z0: i32,
        x1: i32,
        y1: i32,
        z1: i32,
    ) {
        for y in y0..=y1 {
            for x in x0..=x1 {
                for z in z0..=z1 {
                    self.place_sand(base, x, y, z);
                }
            }
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors vanilla placeCollapsedRoof"
    )]
    fn place_collapsed_roof(
        &mut self,
        base: &PieceBase,
        region: &WorldGenRegion<'_>,
        chunk_bb: BoundingBox,
        x0: i32,
        y0: i32,
        z0: i32,
        x1: i32,
        z1: i32,
    ) {
        for x in x0..=x1 {
            for z in z0..=z1 {
                let state = if region.with_random(RandomSource::next_f32) < 0.33 {
                    vanilla_blocks::SANDSTONE.default_state()
                } else {
                    vanilla_blocks::SAND.default_state()
                };
                base.place_block(region, state, x, y0, z, chunk_bb);
            }
        }

        let origin = base.world_pos(x0, y0, z0);
        let mut random = LegacyRandomSource::from_seed(region.seed() as u64)
            .next_positional()
            .at(origin.x(), origin.y(), origin.z());
        let roof_x = random.next_i32_between(x0, x1);
        let roof_z = random.next_i32_between(z0, z1);
        self.random_collapsed_roof_pos = BlockPos::new(
            base.world_x(roof_x, roof_z),
            base.world_y(y0),
            base.world_z(roof_x, roof_z),
        );
    }
}
