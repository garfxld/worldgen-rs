use glam::IVec3;
use simdnbt::owned::{NbtCompound, NbtList, NbtTag};

use crate::registry::REGISTRY;
use crate::registry::blocks::shapes::is_shape_full_block;
use crate::registry::blocks::transform;
use crate::registry::blocks::vanilla_blocks;
use crate::util::direction::Direction;
use crate::util::geometry::BoundingBox;
use crate::util::ident::{BlockState, Identifier};
use crate::util::mirror::Mirror;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::util::rotation::Rotation;
use crate::worldgen::region::WorldGenRegion;
use crate::worldgen::structure::templatesystem::structure_place_settings::StructurePlaceSettings;

#[derive(Debug, Clone, PartialEq)]
pub struct StructureBlockInfo {
    pub pos: BlockPos,
    pub state: BlockState,
    pub nbt: Option<NbtCompound>,
}

#[derive(Debug, Clone)]
pub struct StructureEntityInfo {
    pub pos: (f64, f64, f64),
    pub block_pos: BlockPos,
    pub nbt: NbtCompound,
}

#[derive(Debug, Default)]
pub struct Palette {
    blocks: Vec<StructureBlockInfo>,
}

impl Palette {
    pub fn blocks(&self) -> &[StructureBlockInfo] {
        &self.blocks
    }

    pub fn blocks_of(&self, block: &Identifier) -> impl Iterator<Item = &StructureBlockInfo> {
        self.blocks.iter().filter(move |info| {
            REGISTRY
                .blocks
                .by_state_id(info.state)
                .is_some_and(|actual| actual.key == *block)
        })
    }
}

#[derive(Debug, Default)]
pub struct StructureTemplate {
    size: IVec3,
    palettes: Vec<Palette>,
    entity_info_list: Vec<StructureEntityInfo>,
}

impl StructureTemplate {
    pub fn empty() -> Self {
        Self::default()
    }

    pub const fn size(&self) -> IVec3 {
        self.size
    }

    pub const fn entities(&self) -> &Vec<StructureEntityInfo> {
        &self.entity_info_list
    }

    pub const fn size_rotated(&self, rotation: Rotation) -> IVec3 {
        match rotation {
            Rotation::CounterClockwise90 | Rotation::Clockwise90 => {
                IVec3::new(self.size.z, self.size.y, self.size.x)
            }
            Rotation::None | Rotation::Clockwise180 => self.size,
        }
    }

    pub fn load(nbt: &NbtCompound) -> Self {
        let mut template = Self::default();

        let size = nbt
            .list("size")
            .and_then(NbtList::ints)
            .unwrap_or_default();
        template.size = IVec3::new(
            size.first().copied().unwrap_or_default(),
            size.get(1).copied().unwrap_or_default(),
            size.get(2).copied().unwrap_or_default(),
        );

        let block_list: Vec<&NbtCompound> = nbt
            .list("blocks")
            .and_then(NbtList::compounds)
            .map(|compounds| compounds.iter().collect())
            .unwrap_or_default();

        if let Some(palette_list_list) = nbt.list("palettes") {
            let variants = palette_list_list.lists().unwrap_or(&[]);
            for variant in variants {
                let entries = variant.compounds().unwrap_or(&[]);
                template
                    .palettes
                    .push(load_palette(entries, &block_list));
            }
        } else {
            let entries = nbt
                .list("palette")
                .and_then(NbtList::compounds)
                .unwrap_or(&[]);
            template.palettes.push(load_palette(entries, &block_list));
        }

        if let Some(entities) = nbt
            .list("entities")
            .and_then(NbtList::compounds)
        {
            for entity in entities {
                let pos = entity
                    .list("pos")
                    .and_then(NbtList::doubles)
                    .unwrap_or_default();
                let block_pos = entity
                    .list("blockPos")
                    .and_then(NbtList::ints)
                    .unwrap_or_default();
                let Some(entity_nbt) = entity.compound("nbt") else {
                    continue;
                };
                template.entity_info_list.push(StructureEntityInfo {
                    pos: (
                        pos.first().copied().unwrap_or_default(),
                        pos.get(1).copied().unwrap_or_default(),
                        pos.get(2).copied().unwrap_or_default(),
                    ),
                    block_pos: BlockPos::new(
                        block_pos.first().copied().unwrap_or_default(),
                        block_pos.get(1).copied().unwrap_or_default(),
                        block_pos.get(2).copied().unwrap_or_default(),
                    ),
                    nbt: entity_nbt.clone(),
                });
            }
        }

        template
    }

    pub fn filter_blocks(
        &self,
        position: BlockPos,
        settings: &StructurePlaceSettings,
        block: &Identifier,
        absolute: bool,
        random: &mut RandomSource,
    ) -> Vec<StructureBlockInfo> {
        let mut result = Vec::new();
        if self.palettes.is_empty() {
            return result;
        }
        let bounding_box = settings.bounding_box();
        let palette = &self.palettes[settings.random_palette(self.palettes.len(), position, random)];

        for info in palette.blocks_of(block) {
            let pos = if absolute {
                offset(calculate_relative_position(settings, info.pos), position)
            } else {
                info.pos
            };
            if bounding_box.is_none_or(|bounds| bounds.is_inside(pos)) {
                result.push(StructureBlockInfo {
                    pos,
                    state: transform::rotate(info.state, settings.rotation()),
                    nbt: info.nbt.clone(),
                });
            }
        }

        result
    }

    pub fn place_in_world(
        &self,
        region: &WorldGenRegion<'_>,
        position: BlockPos,
        reference_pos: BlockPos,
        settings: &StructurePlaceSettings,
        random: &mut RandomSource,
    ) -> bool {
        if self.palettes.is_empty() {
            return false;
        }

        let palette_index = settings.random_palette(self.palettes.len(), position, random);
        let blocks = self.palettes[palette_index].blocks();
        if blocks.is_empty() || self.size.x < 1 || self.size.y < 1 || self.size.z < 1 {
            return false;
        }

        let bounding_box = settings.bounding_box();
        let mut to_fill: Vec<BlockPos> = Vec::new();
        let mut locked_fluids: Vec<BlockPos> = Vec::new();
        let processed = process_block_infos(
            region,
            position,
            reference_pos,
            settings,
            blocks,
            random,
        );

        for info in &processed {
            if !bounding_box.is_none_or(|bounds| bounds.is_inside(info.pos)) {
                continue;
            }
            let previous_fluid_state = settings
                .should_apply_waterlogging()
                .then(|| region.get_block(info.pos).get_fluid_state());
            let state = transform::rotate(
                transform::mirror(info.state, settings.mirror()),
                settings.rotation(),
            );

            if region.set_block(info.pos, state)
                && let Some(previous_fluid_state) = previous_fluid_state
            {
                {
                    if state.get_fluid_state().is_source() {
                        locked_fluids.push(info.pos);
                    } else if can_place_liquid(state) {
                        place_liquid(region, info.pos, state);
                        if !previous_fluid_state.is_source() {
                            to_fill.push(info.pos);
                        }
                    }
                }
            }
        }

        let neighbours = [
            Direction::Up,
            Direction::North,
            Direction::East,
            Direction::South,
            Direction::West,
        ];
        let mut filled = true;
        while filled && !to_fill.is_empty() {
            filled = false;
            let mut index = 0;
            while index < to_fill.len() {
                let pos = to_fill[index];
                let mut source = region.get_block(pos).get_fluid_state().is_source();
                for direction in neighbours {
                    if source {
                        break;
                    }
                    let neighbour_pos = direction.relative(pos);
                    let neighbour = region.get_block(neighbour_pos).get_fluid_state();
                    if neighbour.is_source() && !locked_fluids.contains(&neighbour_pos) {
                        source = true;
                    }
                }
                if source {
                    let state = region.get_block(pos);
                    if can_place_liquid(state) {
                        place_liquid(region, pos, state);
                        filled = true;
                        to_fill.remove(index);
                        continue;
                    }
                }
                index += 1;
            }
        }

        true
    }

    pub const fn zero_position_with_transform(
        &self,
        zero_pos: BlockPos,
        mirror: Mirror,
        rotation: Rotation,
    ) -> BlockPos {
        zero_position_with_transform(zero_pos, mirror, rotation, self.size.x, self.size.z)
    }

    pub const fn bounding_box(
        &self,
        settings: &StructurePlaceSettings,
        position: BlockPos,
    ) -> BoundingBox {
        bounding_box_of(
            position,
            settings.rotation(),
            settings.rotation_pivot(),
            settings.mirror(),
            self.size,
        )
    }
}

fn load_palette(entries: &[NbtCompound], block_list: &[&NbtCompound]) -> Palette {
    let states: Vec<BlockState> = entries.iter().map(parse_block_state).collect();

    let mut full_blocks = Vec::new();
    let mut block_entities = Vec::new();
    let mut other_blocks = Vec::new();

    for block in block_list {
        let pos = block
            .list("pos")
            .and_then(NbtList::ints)
            .unwrap_or_default();
        let state = states
            .get(block.int("state").unwrap_or_default().max(0) as usize)
            .copied()
            .unwrap_or_else(|| REGISTRY.blocks.get_default_state_id(&vanilla_blocks::AIR));
        let info = StructureBlockInfo {
            pos: BlockPos::new(
                pos.first().copied().unwrap_or_default(),
                pos.get(1).copied().unwrap_or_default(),
                pos.get(2).copied().unwrap_or_default(),
            ),
            state,
            nbt: block.compound("nbt").cloned(),
        };
        add_to_lists(
            info,
            &mut full_blocks,
            &mut block_entities,
            &mut other_blocks,
        );
    }

    Palette {
        blocks: build_info_list(full_blocks, block_entities, other_blocks),
    }
}

fn parse_block_state(entry: &NbtCompound) -> BlockState {
    let air = REGISTRY.blocks.get_default_state_id(&vanilla_blocks::AIR);
    let Some(name) = entry.string("Name") else {
        return air;
    };
    let Ok(key) = name.to_string().parse::<Identifier>() else {
        return air;
    };
    let Some(block) = REGISTRY.blocks.by_key(&key) else {
        return air;
    };

    let mut properties: Vec<(String, String)> = Vec::new();
    if let Some(property_tags) = entry.compound("Properties") {
        for (property, value) in property_tags.iter() {
            if let NbtTag::String(value) = value {
                properties.push((property.to_string(), value.to_string()));
            }
        }
    }

    REGISTRY
        .blocks
        .state_id_from_block_defaulted_properties(
            block,
            properties
                .iter()
                .map(|(property, value)| (property.as_str(), value.as_str())),
        )
        .unwrap_or_else(|| REGISTRY.blocks.get_default_state_id(block))
}

fn add_to_lists(
    info: StructureBlockInfo,
    full_blocks: &mut Vec<StructureBlockInfo>,
    block_entities: &mut Vec<StructureBlockInfo>,
    other_blocks: &mut Vec<StructureBlockInfo>,
) {
    if info.nbt.is_some() {
        block_entities.push(info);
    } else if is_full_collision_block(info.state) {
        full_blocks.push(info);
    } else {
        other_blocks.push(info);
    }
}

fn is_full_collision_block(state: BlockState) -> bool {
    REGISTRY
        .blocks
        .by_state_id(state)
        .is_some_and(|block| !block.config.dynamic_shape)
        && is_shape_full_block(REGISTRY.blocks.get_static_collision_shape(state))
}

fn build_info_list(
    mut full_blocks: Vec<StructureBlockInfo>,
    mut block_entities: Vec<StructureBlockInfo>,
    mut other_blocks: Vec<StructureBlockInfo>,
) -> Vec<StructureBlockInfo> {
    let key = |info: &StructureBlockInfo| (info.pos.y(), info.pos.x(), info.pos.z());
    full_blocks.sort_by_key(key);
    other_blocks.sort_by_key(key);
    block_entities.sort_by_key(key);

    let mut blocks = full_blocks;
    blocks.extend(other_blocks);
    blocks.extend(block_entities);
    blocks
}

pub fn process_block_infos(
    region: &WorldGenRegion<'_>,
    position: BlockPos,
    reference_pos: BlockPos,
    settings: &StructurePlaceSettings,
    blocks: &[StructureBlockInfo],
    random: &mut RandomSource,
) -> Vec<StructureBlockInfo> {
    let mut original_infos = Vec::new();
    let mut processed_infos = Vec::new();
    let process_only_in_current_chunk = !settings
        .processors()
        .iter()
        .any(super::structure_processor::StructureProcessor::evaluates_entire_piece_state);
    let chunk_bounds = settings.bounding_box();

    for info in blocks {
        let pos = offset(calculate_relative_position(settings, info.pos), position);
        if process_only_in_current_chunk
            && !chunk_bounds.is_none_or(|bounds| bounds.is_inside(pos))
        {
            continue;
        }

        let mut processed = Some(StructureBlockInfo {
            pos,
            state: info.state,
            nbt: info.nbt.clone(),
        });
        for processor in settings.processors() {
            let Some(current) = processed else {
                break;
            };
            processed = processor.process_block(
                region,
                position,
                reference_pos,
                info.pos,
                current,
                settings,
                random,
            );
        }

        if let Some(processed) = processed {
            processed_infos.push(processed);
            original_infos.push(info.clone());
        }
    }

    for processor in settings.processors() {
        processor.finalize_processing(
            region,
            position,
            reference_pos,
            &original_infos,
            &mut processed_infos,
            settings,
            random,
        );
    }

    processed_infos
}

pub const fn calculate_relative_position(
    settings: &StructurePlaceSettings,
    pos: BlockPos,
) -> BlockPos {
    transform_pos(
        pos,
        settings.mirror(),
        settings.rotation(),
        settings.rotation_pivot(),
    )
}

pub const fn transform_pos(
    pos: BlockPos,
    mirror: Mirror,
    rotation: Rotation,
    pivot: BlockPos,
) -> BlockPos {
    let (mut x, y, mut z) = (pos.x(), pos.y(), pos.z());
    let mut was_mirrored = true;
    match mirror {
        Mirror::LeftRight => z = -z,
        Mirror::FrontBack => x = -x,
        Mirror::None => was_mirrored = false,
    }

    let pivot_x = pivot.x();
    let pivot_z = pivot.z();

    match rotation {
        Rotation::CounterClockwise90 => {
            BlockPos::new(pivot_x - pivot_z + z, y, pivot_x + pivot_z - x)
        }
        Rotation::Clockwise90 => BlockPos::new(pivot_x + pivot_z - z, y, pivot_z - pivot_x + x),
        Rotation::Clockwise180 => BlockPos::new(pivot_x + pivot_x - x, y, pivot_z + pivot_z - z),
        Rotation::None => {
            if was_mirrored {
                BlockPos::new(x, y, z)
            } else {
                pos
            }
        }
    }
}

pub const fn zero_position_with_transform(
    zero_pos: BlockPos,
    mirror: Mirror,
    rotation: Rotation,
    size_x: i32,
    size_z: i32,
) -> BlockPos {
    let size_x = size_x - 1;
    let size_z = size_z - 1;
    let mirror_delta_x = if matches!(mirror, Mirror::FrontBack) {
        size_x
    } else {
        0
    };
    let mirror_delta_z = if matches!(mirror, Mirror::LeftRight) {
        size_z
    } else {
        0
    };

    match rotation {
        Rotation::CounterClockwise90 => {
            zero_pos.offset(mirror_delta_z, 0, size_x - mirror_delta_x)
        }
        Rotation::Clockwise90 => zero_pos.offset(size_z - mirror_delta_z, 0, mirror_delta_x),
        Rotation::Clockwise180 => {
            zero_pos.offset(size_x - mirror_delta_x, 0, size_z - mirror_delta_z)
        }
        Rotation::None => zero_pos.offset(mirror_delta_x, 0, mirror_delta_z),
    }
}

pub const fn bounding_box_of(
    position: BlockPos,
    rotation: Rotation,
    pivot: BlockPos,
    mirror: Mirror,
    size: IVec3,
) -> BoundingBox {
    let corner1 = transform_pos(BlockPos::new(0, 0, 0), mirror, rotation, pivot);
    let corner2 = transform_pos(
        BlockPos::new(size.x - 1, size.y - 1, size.z - 1),
        mirror,
        rotation,
        pivot,
    );
    BoundingBox::new(corner1.0, corner2.0).moved(position.x(), position.y(), position.z())
}

const fn offset(pos: BlockPos, by: BlockPos) -> BlockPos {
    pos.offset(by.x(), by.y(), by.z())
}

fn can_place_liquid(state: BlockState) -> bool {
    REGISTRY
        .blocks
        .get_properties(state)
        .iter()
        .any(|(property, value)| *property == "waterlogged" && *value == "false")
}

fn place_liquid(region: &WorldGenRegion<'_>, pos: BlockPos, state: BlockState) {
    let Some(block) = REGISTRY.blocks.by_state_id(state) else {
        return;
    };
    let mut properties = REGISTRY.blocks.get_properties(state);
    for entry in &mut properties {
        if entry.0 == "waterlogged" {
            entry.1 = "true";
        }
    }
    if let Some(waterlogged) = REGISTRY
        .blocks
        .state_id_from_block_properties(block, &properties)
    {
        region.set_block(pos, waterlogged);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Once;

    use super::*;
    use crate::registry::Registries;
    use crate::util::ident::Identifier;
    use crate::worldgen::structure::templatesystem::structure_template_manager::{
        processor_list, templates,
    };

    fn init_global_registry() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let mut registries = Registries::new_vanilla().expect("vanilla datapack should load");
            registries.freeze();
            let _ = REGISTRY.init(registries);
        });
    }

    #[test]
    fn fossil_templates_load_with_real_block_states() {
        init_global_registry();
        let template = templates().get_or_create(&Identifier::const_vanilla("fossil/spine_1"));
        assert_eq!(template.size(), IVec3::new(3, 3, 13));

        let blocks = template.palettes[0].blocks();
        assert!(!blocks.is_empty());
        let bone_block = REGISTRY
            .blocks
            .by_key(&Identifier::const_vanilla("bone_block"))
            .expect("bone_block should exist");
        assert!(blocks.iter().all(|info| REGISTRY
            .blocks
            .by_state_id(info.state)
            .is_some_and(|block| block.key == bone_block.key)));
    }

    #[test]
    fn missing_templates_resolve_to_an_empty_template() {
        init_global_registry();
        let template = templates().get_or_create(&Identifier::const_vanilla("fossil/nope"));
        assert_eq!(template.size(), IVec3::ZERO);
    }

    #[test]
    fn fossil_processor_lists_parse() {
        init_global_registry();
        for key in ["fossil_rot", "fossil_coal", "fossil_diamonds"] {
            let list = processor_list(&Identifier::vanilla(key.to_owned()))
                .unwrap_or_else(|| panic!("{key} should load"));
            assert!(!list.processors.is_empty());
        }
    }
}
