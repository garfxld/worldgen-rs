use serde::Deserialize;

use crate::util::axis::Axis;
use crate::util::direction::Direction;
use crate::util::ident::Identifier;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::util::rotation::Rotation;
use crate::worldgen::region::WorldGenRegion;
use crate::worldgen::structure::templatesystem::structure_place_settings::StructurePlaceSettings;
use crate::worldgen::structure::templatesystem::structure_template::StructureTemplate;
use crate::worldgen::structure::templatesystem::structure_template_manager::templates;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateConfig {
    pub templates: Vec<WeightedTemplateEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeightedTemplateEntry {
    pub data: TemplateEntry,
    pub weight: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateEntry {
    pub id: Identifier,
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
    origin: BlockPos,
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

    let settings = StructurePlaceSettings::new()
        .set_rotation(rotation)
        .use_shared_random(true);
    template.place_in_world(region, pos, pos, &settings, random)
}

fn rotated_offset(rotation: Rotation, axis: Axis, template: &StructureTemplate) -> BlockPos {
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
    BlockPos(rotation.rotate(negative).offset_vec() * length)
}
