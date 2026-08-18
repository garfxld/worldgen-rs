use crate::chunk::heightmap::HeightmapType;
use crate::registry::blocks::properties::BoolProperty;
use crate::registry::blocks::properties::Properties;
use crate::registry::blocks::shapes;
use crate::registry::blocks::vanilla_blocks;
use crate::util::direction::Direction;
use crate::util::geometry::BoundingBox;
use crate::util::ident::{BlockState, Identifier};
use crate::util::mirror::Mirror;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::util::rotation::Rotation;
use crate::worldgen::block_predicate::HolderSet;
use crate::worldgen::feature::rule_test::RuleTest;
use crate::worldgen::region::WorldGenRegion;
use crate::worldgen::structure::structure_piece::{PieceBase, PieceKind, StructurePiece};
use crate::worldgen::structure::template_structure_piece::TemplateStructurePiece;
use crate::worldgen::structure::templatesystem::structure_place_settings::StructurePlaceSettings;
use crate::worldgen::structure::templatesystem::structure_processor::{
    PosRuleTest, ProcessorRule, StructureProcessor,
};

const FEATURES_CANNOT_REPLACE: Identifier = Identifier::const_vanilla("features_cannot_replace");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalPlacement {
    OnLandSurface,
    PartlyBuried,
    OnOceanFloor,
    InMountain,
    Underground,
    InNether,
}

impl VerticalPlacement {
    pub const fn parse(value: &str) -> Self {
        match value {
            "partly_buried" => Self::PartlyBuried,
            "on_ocean_floor" => Self::OnOceanFloor,
            "in_mountain" => Self::InMountain,
            "underground" => Self::Underground,
            "in_nether" => Self::InNether,
            _ => Self::OnLandSurface,
        }
    }

    pub const fn heightmap(self) -> HeightmapType {
        match self {
            Self::OnOceanFloor => HeightmapType::OceanFloorWg,
            _ => HeightmapType::WorldSurfaceWg,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PortalProperties {
    pub cold: bool,
    pub mossiness: f32,
    pub air_pocket: bool,
    pub overgrown: bool,
    pub vines: bool,
    pub replace_with_blackstone: bool,
}

fn block_replace_rule(source: &Identifier, probability: f32, target: BlockState) -> ProcessorRule {
    ProcessorRule {
        input_predicate: RuleTest::RandomBlockMatch {
            block: source.clone(),
            probability,
        },
        location_predicate: RuleTest::AlwaysTrue,
        position_predicate: PosRuleTest::AlwaysTrue,
        output_state: target,
    }
}

fn block_replace_rule_always(source: &Identifier, target: BlockState) -> ProcessorRule {
    ProcessorRule {
        input_predicate: RuleTest::BlockMatch {
            block: source.clone(),
        },
        location_predicate: RuleTest::AlwaysTrue,
        position_predicate: PosRuleTest::AlwaysTrue,
        output_state: target,
    }
}

fn lava_processor_rule(
    vertical_placement: VerticalPlacement,
    properties: PortalProperties,
) -> ProcessorRule {
    let lava = Identifier::const_vanilla("lava");
    if vertical_placement == VerticalPlacement::OnOceanFloor {
        block_replace_rule_always(&lava, vanilla_blocks::MAGMA_BLOCK.default_state())
    } else if properties.cold {
        block_replace_rule_always(&lava, vanilla_blocks::NETHERRACK.default_state())
    } else {
        block_replace_rule(&lava, 0.2, vanilla_blocks::MAGMA_BLOCK.default_state())
    }
}

fn make_settings(
    mirror: Mirror,
    rotation: Rotation,
    vertical_placement: VerticalPlacement,
    pivot: BlockPos,
    properties: PortalProperties,
) -> StructurePlaceSettings {
    let ignored = if properties.air_pocket {
        vec![Identifier::const_vanilla("structure_block")]
    } else {
        vec![
            Identifier::const_vanilla("structure_block"),
            Identifier::const_vanilla("air"),
        ]
    };

    let mut rules = vec![
        block_replace_rule(
            &Identifier::const_vanilla("gold_block"),
            0.3,
            vanilla_blocks::AIR.default_state(),
        ),
        lava_processor_rule(vertical_placement, properties),
    ];
    if !properties.cold {
        rules.push(block_replace_rule(
            &Identifier::const_vanilla("netherrack"),
            0.07,
            vanilla_blocks::MAGMA_BLOCK.default_state(),
        ));
    }

    let mut settings = StructurePlaceSettings::new()
        .set_rotation(rotation)
        .set_mirror(mirror)
        .set_rotation_pivot(pivot);
    settings.add_processor(StructureProcessor::BlockIgnore {
        blocks: HolderSet::Entries(ignored),
    });
    settings.add_processor(StructureProcessor::Rule { rules });
    settings.add_processor(StructureProcessor::BlockAge {
        mossiness: properties.mossiness,
    });
    settings.add_processor(StructureProcessor::ProtectedBlocks {
        value: HolderSet::Tag(FEATURES_CANNOT_REPLACE),
    });
    settings.add_processor(StructureProcessor::LavaSubmergedBlock);
    if properties.replace_with_blackstone {
        settings.add_processor(StructureProcessor::BlackstoneReplace);
    }
    settings
}

#[derive(Debug)]
pub struct RuinedPortalPiece {
    pub template: TemplateStructurePiece,
    vertical_placement: VerticalPlacement,
    properties: PortalProperties,
}

impl RuinedPortalPiece {
    #[expect(
        clippy::new_ret_no_self,
        reason = "mirrors the vanilla piece constructor"
    )]
    pub fn new(
        template_position: BlockPos,
        vertical_placement: VerticalPlacement,
        properties: PortalProperties,
        template_location: &Identifier,
        rotation: Rotation,
        mirror: Mirror,
        pivot: BlockPos,
    ) -> StructurePiece {
        let (base, template) = TemplateStructurePiece::new(
            0,
            template_location,
            make_settings(mirror, rotation, vertical_placement, pivot, properties),
            template_position,
        );
        StructurePiece::new(
            base,
            PieceKind::RuinedPortal(Self {
                template,
                vertical_placement,
                properties,
            }),
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
        let bounding_box = self.template.template.get_bounding_box(
            &self.template.place_settings,
            self.template.template_position,
        );
        if !chunk_bb.is_inside(bounding_box.center()) {
            return;
        }

        let chunk_bb = chunk_bb.encapsulate_box(bounding_box);
        self.template.post_process(
            base,
            region,
            random,
            chunk_bb,
            reference_pos,
            |_, _, _, _, _| {},
        );
        self.spread_netherrack(base, random, region);
        self.add_netherrack_drip_columns_below_portal(base, random, region);

        if self.properties.vines || self.properties.overgrown {
            let box_ = base.bounding_box;
            for y in box_.min_y()..=box_.max_y() {
                for x in box_.min_x()..=box_.max_x() {
                    for z in box_.min_z()..=box_.max_z() {
                        let pos = BlockPos::new(x, y, z);
                        if self.properties.vines {
                            maybe_add_vines(random, region, pos);
                        }
                        if self.properties.overgrown {
                            self.maybe_add_leaves_above(random, region, pos);
                        }
                    }
                }
            }
        }
    }

    fn maybe_add_leaves_above(
        &self,
        random: &mut RandomSource,
        region: &WorldGenRegion<'_>,
        pos: BlockPos,
    ) {
        if random.next_f32() < 0.5
            && region.get_block(pos).get_block() == &vanilla_blocks::NETHERRACK
            && region.get_block(pos.above()).is_air()
        {
            region.set_block(
                pos.above(),
                vanilla_blocks::JUNGLE_LEAVES
                    .default_state()
                    .set_value(&Properties::PERSISTENT, true),
            );
        }
    }

    fn add_netherrack_drip_columns_below_portal(
        &self,
        base: &PieceBase,
        random: &mut RandomSource,
        region: &WorldGenRegion<'_>,
    ) {
        let box_ = base.bounding_box;
        for x in box_.min_x() + 1..box_.max_x() {
            for z in box_.min_z() + 1..box_.max_z() {
                let pos = BlockPos::new(x, box_.min_y(), z);
                if region.get_block(pos).get_block() == &vanilla_blocks::NETHERRACK {
                    self.add_netherrack_drip_column(random, region, pos.below());
                }
            }
        }
    }

    fn add_netherrack_drip_column(
        &self,
        random: &mut RandomSource,
        region: &WorldGenRegion<'_>,
        pos: BlockPos,
    ) {
        let mut current_pos = pos;
        self.place_netherrack_or_magma(random, region, current_pos);
        let mut remaining_cap = 8;
        while remaining_cap > 0 && random.next_f32() < 0.5 {
            current_pos = current_pos.below();
            remaining_cap -= 1;
            self.place_netherrack_or_magma(random, region, current_pos);
        }
    }

    fn spread_netherrack(
        &self,
        base: &PieceBase,
        random: &mut RandomSource,
        region: &WorldGenRegion<'_>,
    ) {
        let follow_ground_surface = self.vertical_placement == VerticalPlacement::OnLandSurface
            || self.vertical_placement == VerticalPlacement::OnOceanFloor;
        let box_ = base.bounding_box;
        let center = box_.center();
        let center_x = center.x();
        let center_z = center.z();
        let probability_by_distance: [f32; 14] = [
            1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.9, 0.9, 0.8, 0.7, 0.6, 0.4, 0.2,
        ];
        let max_distance = probability_by_distance.len() as i32;
        let average_width = (box_.x_span() + box_.z_span()) / 2;
        let distance_adjustment = random.next_i32_bounded(1.max(8 - average_width / 2));

        for x in center_x - max_distance..=center_x + max_distance {
            for z in center_z - max_distance..=center_z + max_distance {
                let distance = (x - center_x).abs() + (z - center_z).abs();
                let adjusted_distance = 0.max(distance + distance_adjustment);
                if adjusted_distance >= max_distance {
                    continue;
                }
                let probability = probability_by_distance[adjusted_distance as usize];
                if random.next_f64() >= f64::from(probability) {
                    continue;
                }
                let surface_y = region.get_height(self.vertical_placement.heightmap(), x, z) - 1;
                let y = if follow_ground_surface {
                    surface_y
                } else {
                    box_.min_y().min(surface_y)
                };
                let pos = BlockPos::new(x, y, z);
                if (y - box_.min_y()).abs() <= 3
                    && self.can_block_be_replaced_by_netherrack_or_magma(region, pos)
                {
                    self.place_netherrack_or_magma(random, region, pos);
                    if self.properties.overgrown {
                        self.maybe_add_leaves_above(random, region, pos);
                    }
                    self.add_netherrack_drip_column(random, region, pos.below());
                }
            }
        }
    }

    fn can_block_be_replaced_by_netherrack_or_magma(
        &self,
        region: &WorldGenRegion<'_>,
        pos: BlockPos,
    ) -> bool {
        let state = region.get_block(pos);
        let block = state.get_block();
        block != &vanilla_blocks::AIR
            && block != &vanilla_blocks::OBSIDIAN
            && !block.has_tag(&FEATURES_CANNOT_REPLACE)
            && (self.vertical_placement == VerticalPlacement::InNether
                || block != &vanilla_blocks::LAVA)
    }

    fn place_netherrack_or_magma(
        &self,
        random: &mut RandomSource,
        region: &WorldGenRegion<'_>,
        pos: BlockPos,
    ) {
        if !self.properties.cold && random.next_f32() < 0.07 {
            region.set_block(pos, vanilla_blocks::MAGMA_BLOCK.default_state());
        } else {
            region.set_block(pos, vanilla_blocks::NETHERRACK.default_state());
        }
    }
}

fn maybe_add_vines(random: &mut RandomSource, region: &WorldGenRegion<'_>, pos: BlockPos) {
    let state = region.get_block(pos);
    if state.is_air() || state.get_block() == &vanilla_blocks::VINE {
        return;
    }
    let direction = Direction::get_random_horizontal(random);
    let neighbour_pos = direction.relative(pos);
    if !region.get_block(neighbour_pos).is_air() {
        return;
    }
    if !shapes::is_face_sturdy(
        state.get_static_collision_shape(),
        direction,
        shapes::SupportType::Full,
    ) {
        return;
    }
    let vine_property = vine_property_for_face(direction.opposite());
    region.set_block(
        neighbour_pos,
        vanilla_blocks::VINE
            .default_state()
            .set_value(vine_property, true),
    );
}

const fn vine_property_for_face(direction: Direction) -> &'static BoolProperty {
    match direction {
        Direction::North => &Properties::NORTH,
        Direction::South => &Properties::SOUTH,
        Direction::West => &Properties::WEST,
        Direction::East => &Properties::EAST,
        Direction::Up => &Properties::UP,
        Direction::Down => &Properties::DOWN,
    }
}
