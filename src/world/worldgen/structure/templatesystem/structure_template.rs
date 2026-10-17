use glam::IVec3;
use shulkr::util::Key;
use shulkr::world::block::Block;
use simdnbt::owned::{NbtCompound, NbtList, NbtTag};

use shulkr::util::is_shape_full_block;
use shulkr::world::block::Properties;
use shulkr::world::fluid::Fluid;

use crate::registry::fluid::FluidState;
use crate::util::rng::RandomSource;
use crate::world::worldgen::region::WorldGenRegion;
use crate::world::worldgen::structure::templatesystem::structure_place_settings::StructurePlaceSettings;
use shulkr::util::BlockPosition;
use shulkr::util::BoundingBox;
use shulkr::util::Direction;
use shulkr::world::block::BlockState;
use shulkr::world::block::Mirror;
use shulkr::world::block::Rotation;
use std::sync;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub struct StructureBlockInfo {
    pub pos: BlockPosition,
    pub state: BlockState,
    pub nbt: Option<Arc<NbtCompound>>,
}

#[derive(Debug, Clone)]
pub struct StructureEntityInfo {
    pub pos: (f64, f64, f64),
    pub block_pos: BlockPosition,
    pub nbt: NbtCompound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JointType {
    Rollable,
    Aligned,
}

#[derive(Debug, Clone)]
pub struct JigsawBlockInfo {
    pub info: StructureBlockInfo,
    pub joint_type: JointType,
    pub name: Option<Key>,
    pub pool: Key,
    pub target: Key,
    pub placement_priority: i32,
    pub selection_priority: i32,
}

impl JigsawBlockInfo {
    pub const fn new(
        info: StructureBlockInfo,
        joint_type: JointType,
        name: Option<Key>,
        pool: Key,
        target: Key,
        placement_priority: i32,
        selection_priority: i32,
    ) -> Self {
        Self {
            info,
            joint_type,
            name,
            pool,
            target,
            placement_priority,
            selection_priority,
        }
    }

    pub fn of(info: StructureBlockInfo) -> Self {
        let nbt = info.nbt.clone().unwrap_or_default();
        let joint_type = joint_type(&nbt, info.state);
        Self {
            joint_type,
            name: Some(nbt_identifier(&nbt, "name").unwrap_or_else(empty_jigsaw_id)),
            pool: nbt_identifier(&nbt, "pool").unwrap_or_else(empty_pool_id),
            target: nbt_identifier(&nbt, "target").unwrap_or_else(empty_jigsaw_id),
            placement_priority: nbt.int("placement_priority").unwrap_or_default(),
            selection_priority: nbt.int("selection_priority").unwrap_or_default(),
            info,
        }
    }

    pub fn with_info(&self, info: StructureBlockInfo) -> Self {
        Self {
            info,
            joint_type: self.joint_type,
            name: self.name.clone(),
            pool: self.pool.clone(),
            target: self.target.clone(),
            placement_priority: self.placement_priority,
            selection_priority: self.selection_priority,
        }
    }
}

pub const fn empty_jigsaw_id() -> Key {
    Key::const_vanilla("empty")
}

pub const fn empty_pool_id() -> Key {
    Key::const_vanilla("empty")
}

fn nbt_identifier(nbt: &NbtCompound, key: &str) -> Option<Key> {
    nbt.string(key)
        .map(|value| Key::of(value.to_str().into_owned()))
}

fn joint_type(nbt: &NbtCompound, state: BlockState) -> JointType {
    match nbt
        .string("joint")
        .map(|value| value.to_string())
        .as_deref()
    {
        Some("rollable") => JointType::Rollable,
        Some("aligned") => JointType::Aligned,
        _ => {
            if front_facing(state).axis().is_horizontal() {
                JointType::Aligned
            } else {
                JointType::Rollable
            }
        }
    }
}

pub fn front_facing(state: BlockState) -> Direction {
    state.try_get(Properties::ORIENTATION).front()
}

pub fn top_facing(state: BlockState) -> Direction {
    state.try_get(Properties::ORIENTATION).top()
}

pub fn can_attach(source: &JigsawBlockInfo, target: &JigsawBlockInfo) -> bool {
    let source_front = front_facing(source.info.state);
    let target_front = front_facing(target.info.state);
    let source_top = top_facing(source.info.state);
    let target_top = top_facing(target.info.state);
    let rollable = source.joint_type == JointType::Rollable;
    source_front == target_front.opposite()
        && (rollable || source_top == target_top)
        && target
            .name
            .as_ref()
            .is_none_or(|name| source.target == *name)
}

#[derive(Debug, Default)]
pub struct Palette {
    blocks: Vec<StructureBlockInfo>,
    jigsaws: Vec<JigsawBlockInfo>,
}

impl Palette {
    pub fn blocks(&self) -> &[StructureBlockInfo] {
        &self.blocks
    }

    pub fn jigsaws(&self) -> &[JigsawBlockInfo] {
        &self.jigsaws
    }

    pub fn blocks_of(&self, block: &Key) -> impl Iterator<Item = &StructureBlockInfo> {
        self.blocks
            .iter()
            .filter(move |info| info.state.as_block().key() == block)
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
        let legacy_field_names = nbt.int("DataVersion").unwrap_or(500) < 5006;

        let size = nbt.list("size").and_then(NbtList::ints).unwrap_or_default();
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
                    .push(load_palette(entries, &block_list, legacy_field_names));
            }
        } else {
            let entries = nbt
                .list("palette")
                .and_then(NbtList::compounds)
                .unwrap_or(&[]);
            template
                .palettes
                .push(load_palette(entries, &block_list, legacy_field_names));
        }

        if let Some(entities) = nbt.list("entities").and_then(NbtList::compounds) {
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
                    block_pos: BlockPosition::new(
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

    pub fn get_jigsaws(
        &self,
        position: BlockPosition,
        rotation: Rotation,
        shared: &mut RandomSource,
    ) -> Vec<JigsawBlockInfo> {
        if self.palettes.is_empty() {
            return Vec::new();
        }

        let settings = StructurePlaceSettings::new().set_rotation(rotation);
        let palette_index = settings.random_palette(self.palettes.len(), position, shared);
        self.palettes[palette_index]
            .jigsaws()
            .iter()
            .map(|jigsaw| {
                let info = &jigsaw.info;
                jigsaw.with_info(StructureBlockInfo {
                    pos: offset(calculate_relative_position(&settings, info.pos), position),
                    state: info.state.rotate(settings.rotation()),
                    nbt: info.nbt.clone(),
                })
            })
            .collect()
    }

    pub const fn get_bounding_box(
        &self,
        settings: &StructurePlaceSettings,
        position: BlockPosition,
    ) -> BoundingBox {
        bounding_box_of(
            position,
            settings.rotation(),
            settings.rotation_pivot(),
            settings.mirror(),
            self.size,
        )
    }

    pub fn filter_blocks(
        &self,
        position: BlockPosition,
        settings: &StructurePlaceSettings,
        block: &Key,
        absolute: bool,
        random: &mut RandomSource,
    ) -> Vec<StructureBlockInfo> {
        let mut result = Vec::new();
        if self.palettes.is_empty() {
            return result;
        }
        let bounding_box = settings.bounding_box();
        let palette =
            &self.palettes[settings.random_palette(self.palettes.len(), position, random)];

        for info in palette.blocks_of(block) {
            let pos = if absolute {
                offset(calculate_relative_position(settings, info.pos), position)
            } else {
                info.pos
            };
            if bounding_box.is_none_or(|bounds| bounds.is_inside(pos)) {
                result.push(StructureBlockInfo {
                    pos,
                    state: info.state.rotate(settings.rotation()),
                    nbt: info.nbt.clone(),
                });
            }
        }

        result
    }

    pub fn place_in_world(
        &self,
        region: &WorldGenRegion<'_>,
        position: BlockPosition,
        reference_pos: BlockPosition,
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
        let mut to_fill: Vec<BlockPosition> = Vec::new();
        let mut locked_fluids: Vec<BlockPosition> = Vec::new();
        let processed =
            process_block_infos(region, position, reference_pos, settings, blocks, random);

        for info in &processed {
            if !bounding_box.is_none_or(|bounds| bounds.is_inside(info.pos)) {
                continue;
            }
            let previous_fluid_state = settings
                .should_apply_waterlogging()
                .then(|| region.get_block(info.pos).get_fluid_state());
            let state = info
                .state
                .mirror(settings.mirror())
                .rotate(settings.rotation());

            if region.set_block(info.pos, state)
                && let Some(previous_fluid_state) = previous_fluid_state
            {
                if state.get_fluid_state().is_source() {
                    locked_fluids.push(info.pos);
                } else if is_liquid_block_container(state) {
                    place_liquid(region, info.pos, state, previous_fluid_state);
                    if !previous_fluid_state.is_source() {
                        to_fill.push(info.pos);
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
                let mut to_place = region.get_block(pos).get_fluid_state();
                for direction in neighbours {
                    if to_place.is_source() {
                        break;
                    }
                    let neighbour_pos = pos.relative(direction);
                    let neighbour = region.get_block(neighbour_pos).get_fluid_state();
                    if neighbour.is_source() && !locked_fluids.contains(&neighbour_pos) {
                        to_place = neighbour;
                    }
                }
                if to_place.is_source() {
                    let state = region.get_block(pos);
                    if is_liquid_block_container(state) {
                        place_liquid(region, pos, state, to_place);
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
        zero_pos: BlockPosition,
        mirror: Mirror,
        rotation: Rotation,
    ) -> BlockPosition {
        zero_position_with_transform(zero_pos, mirror, rotation, self.size.x, self.size.z)
    }

    pub const fn bounding_box(
        &self,
        settings: &StructurePlaceSettings,
        position: BlockPosition,
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

fn load_palette(
    entries: &[NbtCompound],
    block_list: &[&NbtCompound],
    legacy_field_names: bool,
) -> Palette {
    let states: Vec<BlockState> = entries
        .iter()
        .map(|entry| parse_block_state(entry, legacy_field_names))
        .collect();

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
            .unwrap_or_else(|| Block::AIR.default_state());
        let info = StructureBlockInfo {
            pos: BlockPosition::new(
                pos.first().copied().unwrap_or_default(),
                pos.get(1).copied().unwrap_or_default(),
                pos.get(2).copied().unwrap_or_default(),
            ),
            state,
            nbt: block.compound("nbt").cloned().map(sync::Arc::new),
        };
        add_to_lists(
            info,
            &mut full_blocks,
            &mut block_entities,
            &mut other_blocks,
        );
    }

    let blocks = build_info_list(full_blocks, block_entities, other_blocks);
    let jigsaws = blocks
        .iter()
        .filter(|info| Some(info.state.as_block()).is_some_and(|block| block == Block::JIGSAW))
        .map(|info| JigsawBlockInfo::of(info.clone()))
        .collect();

    Palette { blocks, jigsaws }
}

fn parse_block_state(entry: &NbtCompound, legacy_field_names: bool) -> BlockState {
    let (id_field, properties_field) = if legacy_field_names {
        ("Name", "Properties")
    } else {
        ("id", "properties")
    };
    let air = Block::AIR.default_state();
    let Some(name) = entry.string(id_field) else {
        return air;
    };
    let key = Key::of(name.to_string());
    let Some(block) = Block::from_key(key.clone()) else {
        return air;
    };

    let mut properties: Vec<(String, String)> = Vec::new();
    if let Some(property_tags) = entry.compound(properties_field) {
        for (property, value) in property_tags.iter() {
            if let NbtTag::String(value) = value {
                properties.push((property.to_string(), value.to_string()));
            }
        }
    }

    block.default_state().with_properties(
        properties
            .iter()
            .map(|(property, value)| (property.as_str(), value.as_str())),
    )
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
    Some(state.as_block()).is_some_and(|_block| !state.has_dynamic_shape())
        && is_shape_full_block(*state.get_static_collision_shape())
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
    position: BlockPosition,
    reference_pos: BlockPosition,
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
        if process_only_in_current_chunk && !chunk_bounds.is_none_or(|bounds| bounds.is_inside(pos))
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
    pos: BlockPosition,
) -> BlockPosition {
    transform_pos(
        pos,
        settings.mirror(),
        settings.rotation(),
        settings.rotation_pivot(),
    )
}

pub const fn transform_pos(
    pos: BlockPosition,
    mirror: Mirror,
    rotation: Rotation,
    pivot: BlockPosition,
) -> BlockPosition {
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
            BlockPosition::new(pivot_x - pivot_z + z, y, pivot_x + pivot_z - x)
        }
        Rotation::Clockwise90 => {
            BlockPosition::new(pivot_x + pivot_z - z, y, pivot_z - pivot_x + x)
        }
        Rotation::Clockwise180 => {
            BlockPosition::new(pivot_x + pivot_x - x, y, pivot_z + pivot_z - z)
        }
        Rotation::None => {
            if was_mirrored {
                BlockPosition::new(x, y, z)
            } else {
                pos
            }
        }
    }
}

pub const fn zero_position_with_transform(
    zero_pos: BlockPosition,
    mirror: Mirror,
    rotation: Rotation,
    size_x: i32,
    size_z: i32,
) -> BlockPosition {
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
        Rotation::CounterClockwise90 => zero_pos.offset(mirror_delta_z, 0, size_x - mirror_delta_x),
        Rotation::Clockwise90 => zero_pos.offset(size_z - mirror_delta_z, 0, mirror_delta_x),
        Rotation::Clockwise180 => {
            zero_pos.offset(size_x - mirror_delta_x, 0, size_z - mirror_delta_z)
        }
        Rotation::None => zero_pos.offset(mirror_delta_x, 0, mirror_delta_z),
    }
}

pub const fn bounding_box_of(
    position: BlockPosition,
    rotation: Rotation,
    pivot: BlockPosition,
    mirror: Mirror,
    size: IVec3,
) -> BoundingBox {
    let corner1 = transform_pos(BlockPosition::new(0, 0, 0), mirror, rotation, pivot);
    let corner2 = transform_pos(
        BlockPosition::new(size.x - 1, size.y - 1, size.z - 1),
        mirror,
        rotation,
        pivot,
    );
    BoundingBox::new(
        corner1.x(),
        corner1.y(),
        corner1.z(),
        corner2.x(),
        corner2.y(),
        corner2.z(),
    )
    .moved(position.x(), position.y(), position.z())
}

const fn offset(pos: BlockPosition, by: BlockPosition) -> BlockPosition {
    pos.offset(by.x(), by.y(), by.z())
}

fn is_liquid_block_container(state: BlockState) -> bool {
    state
        .properties()
        .iter()
        .any(|(property, _)| *property == "waterlogged")
}

fn place_liquid(
    region: &WorldGenRegion<'_>,
    pos: BlockPosition,
    state: BlockState,
    fluid: FluidState,
) {
    if fluid.fluid() != Fluid::WATER {
        return;
    }
    let Some(block) = Some(state.as_block()) else {
        return;
    };
    let mut properties = state.properties();
    if properties
        .iter()
        .any(|(property, value)| *property == "waterlogged" && *value == "true")
    {
        return;
    }
    for entry in &mut properties {
        if entry.0 == "waterlogged" {
            entry.1 = "true";
        }
    }
    region.set_block(
        pos,
        block
            .default_state()
            .with_properties(properties.iter().copied()),
    );
}

#[cfg(test)]
mod tests {
    use crate::datapack;
    use crate::world::worldgen;
    use std::sync::Once;

    use super::*;
    use crate::world::worldgen::structure::templatesystem::structure_template_manager::{
        processor_list, templates,
    };
    use shulkr::util::Key;

    fn init_global_registry() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            worldgen::ensure_registry_initialized(&datapack::Datapack::builtin())
                .expect("vanilla datapack should load");
        });
    }

    #[test]
    fn fossil_templates_load_with_real_block_states() {
        init_global_registry();
        let template = templates().get_or_create(&Key::const_vanilla("fossil/spine_1"));
        assert_eq!(template.size(), IVec3::new(3, 3, 13));

        let blocks = template.palettes[0].blocks();
        assert!(!blocks.is_empty());
        let bone_block =
            *Block::from_key(Key::const_vanilla("bone_block")).expect("bone_block should exist");
        assert!(blocks.iter().all(|info| {
            Some(info.state.as_block()).is_some_and(|block| block.key() == bone_block.key())
        }));
    }

    #[test]
    fn missing_templates_resolve_to_an_empty_template() {
        init_global_registry();
        let template = templates().get_or_create(&Key::const_vanilla("fossil/nope"));
        assert_eq!(template.size(), IVec3::ZERO);
    }

    #[test]
    fn fossil_processor_lists_parse() {
        init_global_registry();
        for key in ["fossil_rot", "fossil_coal", "fossil_diamonds"] {
            let list = processor_list(&Key::vanilla(key.to_owned()))
                .unwrap_or_else(|| panic!("{key} should load"));
            assert!(!list.processors.is_empty());
        }
    }
}
