use std::sync::LazyLock;

use crate::chunk::heightmap::HeightmapType;
use crate::registry::blocks::properties::Properties;
use crate::registry::blocks::vanilla_blocks;
use crate::util::geometry::BoundingBox;
use crate::util::ident::{BlockState, Identifier};
use crate::util::mirror::Mirror;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::util::rotation::Rotation;
use crate::util::value_providers::int::IntProvider;
use crate::worldgen::block_predicate::HolderSet;
use crate::worldgen::feature::rule_test::RuleTest;
use crate::worldgen::region::WorldGenRegion;
use crate::worldgen::structure::pieces::structure_pieces_builder::StructurePiecesBuilder;
use crate::worldgen::structure::structure_piece::{PieceBase, PieceKind, StructurePiece};
use crate::worldgen::structure::template_structure_piece::TemplateStructurePiece;
use crate::worldgen::structure::templatesystem::structure_place_settings::StructurePlaceSettings;
use crate::worldgen::structure::templatesystem::structure_processor::{
    PosRuleTest, ProcessorRule, StructureProcessor,
};
use crate::worldgen::structure::templatesystem::structure_template::transform_pos;

const ICE_TAG: Identifier = Identifier::const_vanilla("ice");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Temperature {
    Warm,
    Cold,
}

const WARM_RUINS: &[&str] = &[
    "underwater_ruin/warm_1",
    "underwater_ruin/warm_2",
    "underwater_ruin/warm_3",
    "underwater_ruin/warm_4",
    "underwater_ruin/warm_5",
    "underwater_ruin/warm_6",
    "underwater_ruin/warm_7",
    "underwater_ruin/warm_8",
];

const RUINS_BRICK: &[&str] = &[
    "underwater_ruin/brick_1",
    "underwater_ruin/brick_2",
    "underwater_ruin/brick_3",
    "underwater_ruin/brick_4",
    "underwater_ruin/brick_5",
    "underwater_ruin/brick_6",
    "underwater_ruin/brick_7",
    "underwater_ruin/brick_8",
];

const RUINS_CRACKED: &[&str] = &[
    "underwater_ruin/cracked_1",
    "underwater_ruin/cracked_2",
    "underwater_ruin/cracked_3",
    "underwater_ruin/cracked_4",
    "underwater_ruin/cracked_5",
    "underwater_ruin/cracked_6",
    "underwater_ruin/cracked_7",
    "underwater_ruin/cracked_8",
];

const RUINS_MOSSY: &[&str] = &[
    "underwater_ruin/mossy_1",
    "underwater_ruin/mossy_2",
    "underwater_ruin/mossy_3",
    "underwater_ruin/mossy_4",
    "underwater_ruin/mossy_5",
    "underwater_ruin/mossy_6",
    "underwater_ruin/mossy_7",
    "underwater_ruin/mossy_8",
];

const BIG_RUINS_BRICK: &[&str] = &[
    "underwater_ruin/big_brick_1",
    "underwater_ruin/big_brick_2",
    "underwater_ruin/big_brick_3",
    "underwater_ruin/big_brick_8",
];

const BIG_RUINS_MOSSY: &[&str] = &[
    "underwater_ruin/big_mossy_1",
    "underwater_ruin/big_mossy_2",
    "underwater_ruin/big_mossy_3",
    "underwater_ruin/big_mossy_8",
];

const BIG_RUINS_CRACKED: &[&str] = &[
    "underwater_ruin/big_cracked_1",
    "underwater_ruin/big_cracked_2",
    "underwater_ruin/big_cracked_3",
    "underwater_ruin/big_cracked_8",
];

const BIG_WARM_RUINS: &[&str] = &[
    "underwater_ruin/big_warm_4",
    "underwater_ruin/big_warm_5",
    "underwater_ruin/big_warm_6",
    "underwater_ruin/big_warm_7",
];

fn location(path: &str) -> Identifier {
    format!("minecraft:{path}")
        .parse()
        .expect("ocean ruin template path should be a valid identifier")
}

fn archy_rule_processor(candidate: &Identifier, replacement: BlockState) -> StructureProcessor {
    StructureProcessor::Capped {
        delegate: Box::new(StructureProcessor::Rule {
            rules: vec![ProcessorRule {
                input_predicate: RuleTest::BlockMatch {
                    block: candidate.clone(),
                },
                location_predicate: RuleTest::AlwaysTrue,
                position_predicate: PosRuleTest::AlwaysTrue,
                output_state: replacement,
            }],
        }),
        limit: IntProvider::Constant(5),
    }
}

static WARM_SUSPICIOUS_BLOCK_PROCESSOR: LazyLock<StructureProcessor> = LazyLock::new(|| {
    archy_rule_processor(
        &Identifier::const_vanilla("sand"),
        vanilla_blocks::SUSPICIOUS_SAND.default_state(),
    )
});

static COLD_SUSPICIOUS_BLOCK_PROCESSOR: LazyLock<StructureProcessor> = LazyLock::new(|| {
    archy_rule_processor(
        &Identifier::const_vanilla("gravel"),
        vanilla_blocks::SUSPICIOUS_GRAVEL.default_state(),
    )
});

fn make_settings(
    rotation: Rotation,
    integrity: f32,
    biome_type: Temperature,
) -> StructurePlaceSettings {
    let mut settings = StructurePlaceSettings::new()
        .set_rotation(rotation)
        .set_mirror(Mirror::None);
    settings.add_processor(StructureProcessor::BlockRot {
        rottable_blocks: None,
        integrity,
    });
    settings.add_processor(StructureProcessor::BlockIgnore {
        blocks: HolderSet::Entries(vec![
            Identifier::const_vanilla("structure_block"),
            Identifier::const_vanilla("air"),
        ]),
    });
    settings.add_processor(match biome_type {
        Temperature::Cold => COLD_SUSPICIOUS_BLOCK_PROCESSOR.clone(),
        Temperature::Warm => WARM_SUSPICIOUS_BLOCK_PROCESSOR.clone(),
    });
    settings
}

fn pick(pool: &[&str], random: &mut RandomSource) -> Identifier {
    location(pool[random.next_i32_bounded(pool.len() as i32) as usize])
}

pub fn add_pieces(
    position: BlockPos,
    rotation: Rotation,
    builder: &mut StructurePiecesBuilder,
    random: &mut RandomSource,
    biome_temp: Temperature,
    large_probability: f32,
    cluster_probability: f32,
) {
    let is_large = random.next_f32() <= large_probability;
    let base_integrity = if is_large { 0.9 } else { 0.8 };
    add_piece(
        position,
        rotation,
        builder,
        random,
        biome_temp,
        is_large,
        base_integrity,
    );
    if is_large && random.next_f32() <= cluster_probability {
        add_cluster_ruins(random, rotation, position, biome_temp, builder);
    }
}

fn add_cluster_ruins(
    random: &mut RandomSource,
    rotation: Rotation,
    p: BlockPos,
    biome_temp: Temperature,
    builder: &mut StructurePiecesBuilder,
) {
    let parent_pos = BlockPos::new(p.x(), 90, p.z());
    let parent_corner_offset = transform_pos(
        BlockPos::new(15, 0, 15),
        Mirror::None,
        rotation,
        BlockPos::new(0, 0, 0),
    );
    let parent_corner = parent_pos.offset(
        parent_corner_offset.x(),
        parent_corner_offset.y(),
        parent_corner_offset.z(),
    );
    let parent_bb = BoundingBox::new(parent_pos.0, parent_corner.0);
    let parent_bottom_left = BlockPos::new(
        parent_pos.x().min(parent_corner.x()),
        parent_pos.y(),
        parent_pos.z().min(parent_corner.z()),
    );
    let mut all_positions = all_positions(random, parent_bottom_left);
    let ruins = random.next_i32_between(4, 8);

    for _ in 0..ruins {
        if all_positions.is_empty() {
            continue;
        }
        let idx = random.next_i32_bounded(all_positions.len() as i32) as usize;
        let pos = all_positions.remove(idx);
        let next_rotation = Rotation::get_random(random);
        let next_corner_offset = transform_pos(
            BlockPos::new(5, 0, 6),
            Mirror::None,
            next_rotation,
            BlockPos::new(0, 0, 0),
        );
        let next_corner = pos.offset(
            next_corner_offset.x(),
            next_corner_offset.y(),
            next_corner_offset.z(),
        );
        let next_bb = BoundingBox::new(pos.0, next_corner.0);
        if !next_bb.intersects(parent_bb) {
            add_piece(pos, next_rotation, builder, random, biome_temp, false, 0.8);
        }
    }
}

fn all_positions(random: &mut RandomSource, origin: BlockPos) -> Vec<BlockPos> {
    vec![
        origin.offset(-16 + random.next_i32_between(1, 8), 0, 16 + random.next_i32_between(1, 7)),
        origin.offset(-16 + random.next_i32_between(1, 8), 0, random.next_i32_between(1, 7)),
        origin.offset(-16 + random.next_i32_between(1, 8), 0, -16 + random.next_i32_between(4, 8)),
        origin.offset(random.next_i32_between(1, 7), 0, 16 + random.next_i32_between(1, 7)),
        origin.offset(random.next_i32_between(1, 7), 0, -16 + random.next_i32_between(4, 6)),
        origin.offset(16 + random.next_i32_between(1, 7), 0, 16 + random.next_i32_between(3, 8)),
        origin.offset(16 + random.next_i32_between(1, 7), 0, random.next_i32_between(1, 7)),
        origin.offset(16 + random.next_i32_between(1, 7), 0, -16 + random.next_i32_between(4, 8)),
    ]
}

fn add_piece(
    position: BlockPos,
    rotation: Rotation,
    builder: &mut StructurePiecesBuilder,
    random: &mut RandomSource,
    biome_temp: Temperature,
    is_large: bool,
    base_integrity: f32,
) {
    match biome_temp {
        Temperature::Warm => {
            let start = if is_large {
                pick(BIG_WARM_RUINS, random)
            } else {
                pick(WARM_RUINS, random)
            };
            builder.add_piece(OceanRuinPiece::new(
                &start,
                position,
                rotation,
                base_integrity,
                biome_temp,
                is_large,
            ));
        }
        Temperature::Cold => {
            let (bricks, cracked, mossy) = if is_large {
                (BIG_RUINS_BRICK, BIG_RUINS_CRACKED, BIG_RUINS_MOSSY)
            } else {
                (RUINS_BRICK, RUINS_CRACKED, RUINS_MOSSY)
            };
            let idx = random.next_i32_bounded(bricks.len() as i32) as usize;
            builder.add_piece(OceanRuinPiece::new(
                &location(bricks[idx]),
                position,
                rotation,
                base_integrity,
                biome_temp,
                is_large,
            ));
            builder.add_piece(OceanRuinPiece::new(
                &location(cracked[idx]),
                position,
                rotation,
                0.7,
                biome_temp,
                is_large,
            ));
            builder.add_piece(OceanRuinPiece::new(
                &location(mossy[idx]),
                position,
                rotation,
                0.5,
                biome_temp,
                is_large,
            ));
        }
    }
}

#[derive(Debug)]
pub struct OceanRuinPiece {
    pub template: TemplateStructurePiece,
    is_large: bool,
}

impl OceanRuinPiece {
    #[expect(clippy::new_ret_no_self, reason = "mirrors the vanilla piece constructor")]
    fn new(
        template_location: &Identifier,
        position: BlockPos,
        rotation: Rotation,
        integrity: f32,
        biome_type: Temperature,
        is_large: bool,
    ) -> StructurePiece {
        let (base, template) = TemplateStructurePiece::new(
            0,
            template_location,
            make_settings(rotation, integrity, biome_type),
            position,
        );
        StructurePiece::new(
            base,
            PieceKind::OceanRuin(Self { template, is_large }),
        )
    }

    pub fn post_process(
        &mut self,
        base: &mut PieceBase,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        chunk_bb: BoundingBox,
        reference_pos: BlockPos,
    ) {
        let position = self.template.template_position;
        let height = region.get_height(HeightmapType::OceanFloorWg, position.x(), position.z());
        self.template.template_position = BlockPos::new(position.x(), height, position.z());

        let size = self.template.template.size();
        let corner_offset = transform_pos(
            BlockPos::new(size.x - 1, 0, size.z - 1),
            Mirror::None,
            self.template.place_settings.rotation(),
            BlockPos::new(0, 0, 0),
        );
        let corner = self.template.template_position.offset(
            corner_offset.x(),
            corner_offset.y(),
            corner_offset.z(),
        );
        let new_y = ground_height(self.template.template_position, region, corner);
        self.template.template_position = BlockPos::new(
            self.template.template_position.x(),
            new_y,
            self.template.template_position.z(),
        );

        let is_large = self.is_large;
        self.template.post_process(
            base,
            region,
            random,
            chunk_bb,
            reference_pos,
            |marker_id, position, region, random, _chunk_bb| match marker_id {
                "chest" => {
                    let waterlogged = region.get_block(position).get_fluid_state().is_water();
                    region.set_block(
                        position,
                        vanilla_blocks::CHEST
                            .default_state()
                            .set_value(&Properties::WATERLOGGED, waterlogged),
                    );
                    let _ = is_large;
                    random.next_i64();
                }
                "drowned" => {
                    if position.y() > region.sea_level() {
                        region.set_block(position, vanilla_blocks::AIR.default_state());
                    } else {
                        region.set_block(position, vanilla_blocks::WATER.default_state());
                    }
                }
                _ => {}
            },
        );
    }
}

fn ground_height(pos: BlockPos, region: &WorldGenRegion<'_>, corner: BlockPos) -> i32 {
    let mut new_y = pos.y();
    let mut min_y = 512;
    let top_y = new_y - 1;
    let mut area = 0;

    let (x0, x1) = (pos.x().min(corner.x()), pos.x().max(corner.x()));
    let (z0, z1) = (pos.z().min(corner.z()), pos.z().max(corner.z()));

    for x in x0..=x1 {
        for z in z0..=z1 {
            let mut floor_y = pos.y() - 1;
            let mut temp_pos = BlockPos::new(x, floor_y, z);
            let mut temp_state = region.get_block(temp_pos);
            while (temp_state.is_air()
                || temp_state.get_fluid_state().is_water()
                || temp_state.get_block().has_tag(&ICE_TAG))
                && floor_y > region.min_y() + 1
            {
                floor_y -= 1;
                temp_pos = BlockPos::new(x, floor_y, z);
                temp_state = region.get_block(temp_pos);
            }

            min_y = min_y.min(floor_y);
            if floor_y < top_y - 2 {
                area += 1;
            }
        }
    }

    let width = (pos.x() - corner.x()).abs();
    if top_y - min_y > 2 && area > width - 2 {
        new_y = min_y + 1;
    }

    new_y
}
