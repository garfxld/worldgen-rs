use serde::Deserialize;
use shulkr::util::Key;

use shulkr::util::Axis;
use shulkr::util::Direction;

use crate::util::rng::RandomSource;
use crate::util::rotation::RotationExt;
use crate::world::worldgen::region::WorldGenRegion;
use crate::world::worldgen::structure::templatesystem::structure_place_settings::StructurePlaceSettings;
use crate::world::worldgen::structure::templatesystem::structure_processor::{
    StructureProcessorList, deserialize_optional_processor_list_holder,
};
use crate::world::worldgen::structure::templatesystem::structure_template::StructureTemplate;
use crate::world::worldgen::structure::templatesystem::structure_template_manager::templates;
use shulkr::util::BlockPosition;
use shulkr::world::block::Rotation;

#[derive(Debug, Clone, Deserialize)]
pub struct TemplateConfig {
    pub templates: Vec<WeightedTemplateEntry>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_processor_list_holder"
    )]
    pub processors: Option<StructureProcessorList>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WeightedTemplateEntry {
    pub data: TemplateEntry,
    pub weight: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TemplateEntry {
    pub id: Key,
    #[serde(default = "all_rotations")]
    pub rotations: Vec<Rotation>,
}

fn all_rotations() -> Vec<Rotation> {
    vec![
        Rotation::None,
        Rotation::Clockwise90,
        Rotation::Clockwise180,
        Rotation::CounterClockwise90,
    ]
}

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPosition,
    config: &TemplateConfig,
) -> bool {
    let total_weight: i32 = config.templates.iter().map(|entry| entry.weight).sum();
    if total_weight <= 0 {
        return false;
    }
    let mut roll = random.next_i32_bounded(total_weight);
    let mut chosen = None;
    for entry in &config.templates {
        roll -= entry.weight;
        if roll < 0 {
            chosen = Some(&entry.data);
            break;
        }
    }
    let Some(entry) = chosen else {
        return false;
    };

    if entry.rotations.is_empty() {
        return false;
    }
    let rotation = entry.rotations[random.next_i32_bounded(entry.rotations.len() as i32) as usize];

    let template = templates().get_or_create(&entry.id);
    let offset_x = rotated_offset(rotation, Axis::X, template);
    let offset_z = rotated_offset(rotation, Axis::Z, template);
    let pos = origin
        .offset(offset_x.x(), offset_x.y(), offset_x.z())
        .offset(offset_z.x(), offset_z.y(), offset_z.z());

    let mut settings = StructurePlaceSettings::new()
        .set_rotation(rotation)
        .use_shared_random(true);
    if let Some(list) = &config.processors {
        for processor in &list.processors {
            settings.add_processor(processor.clone());
        }
    }
    template.place_in_world(region, pos, pos, &settings, random)
}

fn rotated_offset(rotation: Rotation, axis: Axis, template: &StructureTemplate) -> BlockPosition {
    let negative = match axis {
        Axis::X => Direction::West,
        Axis::Y => Direction::Down,
        Axis::Z => Direction::North,
    };
    let size = template.size();
    let length = match axis {
        Axis::X => size.x,
        Axis::Y => size.y,
        Axis::Z => size.z,
    } / 2;
    BlockPosition::ZERO.relative_n(rotation.rotate(negative), length)
}
