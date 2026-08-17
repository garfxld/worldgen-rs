use crate::registry::blocks::properties::{AttachFace, Properties, RedstoneSide};
use crate::registry::blocks::vanilla_blocks;
use crate::util::direction::Direction;
use crate::util::geometry::BoundingBox;
use crate::util::ident::BlockState;
use crate::util::rng::RandomSource;
use crate::worldgen::region::WorldGenRegion;
use crate::worldgen::structure::scattered_feature_piece::ScatteredFeature;
use crate::worldgen::structure::structure_piece::{PieceBase, PieceKind, StructurePiece, air};

pub const WIDTH: i32 = 12;
pub const DEPTH: i32 = 15;

#[derive(Debug)]
pub struct JungleTemplePiece {
    pub scattered: ScatteredFeature,
    placed_main_chest: bool,
    placed_hidden_chest: bool,
    placed_trap_1: bool,
    placed_trap_2: bool,
}

fn moss_stone_selector(random: &mut RandomSource) -> BlockState {
    if random.next_f32() < 0.4 {
        vanilla_blocks::COBBLESTONE.default_state()
    } else {
        vanilla_blocks::MOSSY_COBBLESTONE.default_state()
    }
}

impl JungleTemplePiece {
    #[expect(clippy::new_ret_no_self, reason = "mirrors the vanilla piece constructor")]
    pub fn new(random: &mut RandomSource, west: i32, north: i32) -> StructurePiece {
        let direction = Direction::get_random_horizontal(random);
        let base = ScatteredFeature::make_base(west, 64, north, WIDTH, 10, DEPTH, direction);
        StructurePiece::new(
            base,
            PieceKind::JungleTemple(Self {
                scattered: ScatteredFeature::new(WIDTH, 10, DEPTH),
                placed_main_chest: false,
                placed_hidden_chest: false,
                placed_trap_1: false,
                placed_trap_2: false,
            }),
        )
    }

    #[expect(
        clippy::too_many_lines,
        reason = "mirrors vanilla JungleTemplePiece::postProcess"
    )]
    pub fn post_process(
        &mut self,
        base: &mut PieceBase,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        chunk_bb: BoundingBox,
    ) {
        if !self
            .scattered
            .update_average_ground_height(base, region, chunk_bb, 0)
        {
            return;
        }

        let width = self.scattered.width;
        let depth = self.scattered.depth;
        let piece = &*base;
        let stones = |x0: i32,
                          y0: i32,
                          z0: i32,
                          x1: i32,
                          y1: i32,
                          z1: i32,
                          random: &mut RandomSource| {
            piece.generate_box_selected(
                region,
                chunk_bb,
                x0,
                y0,
                z0,
                x1,
                y1,
                z1,
                false,
                random,
                |random, _, _, _, _| moss_stone_selector(random),
            );
        };

        stones(0, -4, 0, width - 1, 0, depth - 1, random);
        stones(2, 1, 2, 9, 2, 2, random);
        stones(2, 1, 12, 9, 2, 12, random);
        stones(2, 1, 3, 2, 2, 11, random);
        stones(9, 1, 3, 9, 2, 11, random);
        stones(1, 3, 1, 10, 6, 1, random);
        stones(1, 3, 13, 10, 6, 13, random);
        stones(1, 3, 2, 1, 6, 12, random);
        stones(10, 3, 2, 10, 6, 12, random);
        stones(2, 3, 2, 9, 3, 12, random);
        stones(2, 6, 2, 9, 6, 12, random);
        stones(3, 7, 3, 8, 7, 11, random);
        stones(4, 8, 4, 7, 8, 10, random);

        let air_box = |x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32| {
            piece.generate_air_box(region, chunk_bb, x0, y0, z0, x1, y1, z1);
        };
        let place = |state: BlockState, x: i32, y: i32, z: i32| {
            piece.place_block(region, state, x, y, z, chunk_bb);
        };

        air_box(3, 1, 3, 8, 2, 11);
        air_box(4, 3, 6, 7, 3, 9);
        air_box(2, 4, 2, 9, 5, 12);
        air_box(4, 6, 5, 7, 6, 9);
        air_box(5, 7, 6, 6, 7, 8);
        air_box(5, 1, 2, 6, 2, 2);
        air_box(5, 2, 12, 6, 2, 12);
        air_box(5, 5, 1, 6, 5, 1);
        air_box(5, 5, 13, 6, 5, 13);
        place(air(), 1, 5, 5);
        place(air(), 10, 5, 5);
        place(air(), 1, 5, 9);
        place(air(), 10, 5, 9);

        let mut z = 0;
        while z <= 14 {
            stones(2, 4, z, 2, 5, z, random);
            stones(4, 4, z, 4, 5, z, random);
            stones(7, 4, z, 7, 5, z, random);
            stones(9, 4, z, 9, 5, z, random);
            z += 14;
        }

        stones(5, 6, 0, 6, 6, 0, random);

        let mut x = 0;
        while x <= 11 {
            let mut z = 2;
            while z <= 12 {
                stones(x, 4, z, x, 5, z, random);
                z += 2;
            }
            stones(x, 6, 5, x, 6, 5, random);
            stones(x, 6, 9, x, 6, 9, random);
            x += 11;
        }

        stones(2, 7, 2, 2, 9, 2, random);
        stones(9, 7, 2, 9, 9, 2, random);
        stones(2, 7, 12, 2, 9, 12, random);
        stones(9, 7, 12, 9, 9, 12, random);
        stones(4, 9, 4, 4, 9, 4, random);
        stones(7, 9, 4, 7, 9, 4, random);
        stones(4, 9, 10, 4, 9, 10, random);
        stones(7, 9, 10, 7, 9, 10, random);
        stones(5, 9, 7, 6, 9, 7, random);

        let stairs = vanilla_blocks::COBBLESTONE_STAIRS.default_state();
        let east_stairs = stairs.set_value(&Properties::HORIZONTAL_FACING, Direction::East);
        let west_stairs = stairs.set_value(&Properties::HORIZONTAL_FACING, Direction::West);
        let south_stairs = stairs.set_value(&Properties::HORIZONTAL_FACING, Direction::South);
        let north_stairs = stairs.set_value(&Properties::HORIZONTAL_FACING, Direction::North);
        place(north_stairs, 5, 9, 6);
        place(north_stairs, 6, 9, 6);
        place(south_stairs, 5, 9, 8);
        place(south_stairs, 6, 9, 8);
        place(north_stairs, 4, 0, 0);
        place(north_stairs, 5, 0, 0);
        place(north_stairs, 6, 0, 0);
        place(north_stairs, 7, 0, 0);
        place(north_stairs, 4, 1, 8);
        place(north_stairs, 4, 2, 9);
        place(north_stairs, 4, 3, 10);
        place(north_stairs, 7, 1, 8);
        place(north_stairs, 7, 2, 9);
        place(north_stairs, 7, 3, 10);
        stones(4, 1, 9, 4, 1, 9, random);
        stones(7, 1, 9, 7, 1, 9, random);
        stones(4, 1, 10, 7, 2, 10, random);
        stones(5, 4, 5, 6, 4, 5, random);
        place(east_stairs, 4, 4, 5);
        place(west_stairs, 7, 4, 5);

        for i in 0..4 {
            place(south_stairs, 5, -i, 6 + i);
            place(south_stairs, 6, -i, 6 + i);
            air_box(5, -i, 7 + i, 6, -i, 9 + i);
        }

        air_box(1, -3, 12, 10, -1, 13);
        air_box(1, -3, 1, 3, -1, 13);
        air_box(1, -3, 1, 9, -1, 5);

        let mut z = 1;
        while z <= 13 {
            stones(1, -3, z, 1, -2, z, random);
            z += 2;
        }

        let mut z = 2;
        while z <= 12 {
            stones(1, -1, z, 3, -1, z, random);
            z += 2;
        }

        stones(2, -2, 1, 5, -2, 1, random);
        stones(7, -2, 1, 9, -2, 1, random);
        stones(6, -3, 1, 6, -3, 1, random);
        stones(6, -1, 1, 6, -1, 1, random);

        let hook = vanilla_blocks::TRIPWIRE_HOOK.default_state();
        let hook = |facing: Direction| {
            hook.set_value(&Properties::HORIZONTAL_FACING, facing)
                .set_value(&Properties::ATTACHED, true)
        };
        let tripwire = vanilla_blocks::TRIPWIRE
            .default_state()
            .set_value(&Properties::ATTACHED, true);
        let tripwire_ew = tripwire
            .set_value(&Properties::EAST, true)
            .set_value(&Properties::WEST, true);
        let tripwire_ns = tripwire
            .set_value(&Properties::NORTH, true)
            .set_value(&Properties::SOUTH, true);

        place(hook(Direction::East), 1, -3, 8);
        place(hook(Direction::West), 4, -3, 8);
        place(tripwire_ew, 2, -3, 8);
        place(tripwire_ew, 3, -3, 8);

        let wire = vanilla_blocks::REDSTONE_WIRE.default_state();
        let redstone_wire_ns = wire
            .set_value(&Properties::NORTH_REDSTONE, RedstoneSide::Side)
            .set_value(&Properties::SOUTH_REDSTONE, RedstoneSide::Side);
        place(redstone_wire_ns, 5, -3, 7);
        place(redstone_wire_ns, 5, -3, 6);
        place(redstone_wire_ns, 5, -3, 5);
        place(redstone_wire_ns, 5, -3, 4);
        place(redstone_wire_ns, 5, -3, 3);
        place(redstone_wire_ns, 5, -3, 2);
        place(
            wire.set_value(&Properties::NORTH_REDSTONE, RedstoneSide::Side)
                .set_value(&Properties::WEST_REDSTONE, RedstoneSide::Side),
            5,
            -3,
            1,
        );
        place(
            wire.set_value(&Properties::EAST_REDSTONE, RedstoneSide::Side)
                .set_value(&Properties::WEST_REDSTONE, RedstoneSide::Side),
            4,
            -3,
            1,
        );
        let mossy = vanilla_blocks::MOSSY_COBBLESTONE.default_state();
        place(mossy, 3, -3, 1);
        if !self.placed_trap_1 {
            self.placed_trap_1 = piece.create_dispenser(
                region,
                chunk_bb,
                random,
                3,
                -2,
                1,
                Direction::North,
            );
        }

        let vine = vanilla_blocks::VINE.default_state();
        place(vine.set_value(&Properties::SOUTH, true), 3, -2, 2);
        place(hook(Direction::North), 7, -3, 1);
        place(hook(Direction::South), 7, -3, 5);
        place(tripwire_ns, 7, -3, 2);
        place(tripwire_ns, 7, -3, 3);
        place(tripwire_ns, 7, -3, 4);
        place(
            wire.set_value(&Properties::EAST_REDSTONE, RedstoneSide::Side)
                .set_value(&Properties::WEST_REDSTONE, RedstoneSide::Side),
            8,
            -3,
            6,
        );
        place(
            wire.set_value(&Properties::WEST_REDSTONE, RedstoneSide::Side)
                .set_value(&Properties::SOUTH_REDSTONE, RedstoneSide::Side),
            9,
            -3,
            6,
        );
        place(
            wire.set_value(&Properties::NORTH_REDSTONE, RedstoneSide::Side)
                .set_value(&Properties::SOUTH_REDSTONE, RedstoneSide::Up),
            9,
            -3,
            5,
        );
        place(mossy, 9, -3, 4);
        place(redstone_wire_ns, 9, -2, 4);
        if !self.placed_trap_2 {
            self.placed_trap_2 =
                piece.create_dispenser(region, chunk_bb, random, 9, -2, 3, Direction::West);
        }

        place(vine.set_value(&Properties::EAST, true), 8, -1, 3);
        place(vine.set_value(&Properties::EAST, true), 8, -2, 3);
        if !self.placed_main_chest {
            self.placed_main_chest = piece.create_chest(region, chunk_bb, random, 8, -3, 3);
        }

        place(mossy, 9, -3, 2);
        place(mossy, 8, -3, 1);
        place(mossy, 4, -3, 5);
        place(mossy, 5, -2, 5);
        place(mossy, 5, -1, 5);
        place(mossy, 6, -3, 5);
        place(mossy, 7, -2, 5);
        place(mossy, 7, -1, 5);
        place(mossy, 8, -3, 5);
        stones(9, -1, 1, 9, -1, 5, random);
        air_box(8, -3, 8, 10, -1, 10);
        let chiseled = vanilla_blocks::CHISELED_STONE_BRICKS.default_state();
        place(chiseled, 8, -2, 11);
        place(chiseled, 9, -2, 11);
        place(chiseled, 10, -2, 11);
        let lever = vanilla_blocks::LEVER
            .default_state()
            .set_value(&Properties::HORIZONTAL_FACING, Direction::North)
            .set_value(&Properties::ATTACH_FACE, AttachFace::Wall);
        place(lever, 8, -2, 12);
        place(lever, 9, -2, 12);
        place(lever, 10, -2, 12);
        stones(8, -3, 8, 8, -3, 10, random);
        stones(10, -3, 8, 10, -3, 10, random);
        place(mossy, 10, -2, 9);
        place(redstone_wire_ns, 8, -2, 9);
        place(redstone_wire_ns, 8, -2, 10);
        place(
            wire.set_value(&Properties::NORTH_REDSTONE, RedstoneSide::Side)
                .set_value(&Properties::SOUTH_REDSTONE, RedstoneSide::Side)
                .set_value(&Properties::EAST_REDSTONE, RedstoneSide::Side)
                .set_value(&Properties::WEST_REDSTONE, RedstoneSide::Side),
            10,
            -1,
            9,
        );
        let piston = vanilla_blocks::STICKY_PISTON.default_state();
        place(piston.set_value(&Properties::FACING, Direction::Up), 9, -2, 8);
        place(
            piston.set_value(&Properties::FACING, Direction::West),
            10,
            -2,
            8,
        );
        place(
            piston.set_value(&Properties::FACING, Direction::West),
            10,
            -1,
            8,
        );
        place(
            vanilla_blocks::REPEATER
                .default_state()
                .set_value(&Properties::HORIZONTAL_FACING, Direction::North),
            10,
            -2,
            10,
        );
        if !self.placed_hidden_chest {
            self.placed_hidden_chest = piece.create_chest(region, chunk_bb, random, 9, -3, 10);
        }
    }
}
