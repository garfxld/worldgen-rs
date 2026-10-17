use std::sync::OnceLock;

use serde::Deserialize;
use shulkr::util::Key;

use crate::world::chunk::heightmap::HeightmapType;
use shulkr::util::BlockPosition;

use crate::util::rng::RandomSource;
use crate::util::shuffle;
use crate::world::worldgen::structure::pools::structure_pool_element::StructurePoolElement;
use crate::world::worldgen::structure::templatesystem::structure_processor::StructureProcessor;
use shulkr::world::block::Rotation;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Projection {
    #[serde(rename = "terrain_matching")]
    TerrainMatching,
    #[serde(rename = "rigid")]
    Rigid,
}

impl Projection {
    pub fn processors(self) -> Vec<StructureProcessor> {
        match self {
            Self::TerrainMatching => vec![StructureProcessor::Gravity {
                heightmap: HeightmapType::WorldSurfaceWg,
                offset: -1,
            }],
            Self::Rigid => Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct StructureTemplatePool {
    templates: Vec<&'static StructurePoolElement>,
    fallback: Key,
    max_size: OnceLock<i32>,
}

impl StructureTemplatePool {
    pub fn new(fallback: Key, templates: Vec<(&'static StructurePoolElement, i32)>) -> Self {
        let mut expanded = Vec::new();
        for (element, weight) in templates {
            for _ in 0..weight {
                expanded.push(element);
            }
        }
        Self {
            templates: expanded,
            fallback,
            max_size: OnceLock::new(),
        }
    }

    pub fn get_max_size(&self) -> i32 {
        *self.max_size.get_or_init(|| {
            self.templates
                .iter()
                .filter(|element| !matches!(element, StructurePoolElement::Empty))
                .map(|element| {
                    element
                        .get_bounding_box(BlockPosition::new(0, 0, 0), Rotation::None)
                        .y_span()
                })
                .max()
                .unwrap_or(0)
        })
    }

    pub const fn fallback(&self) -> &Key {
        &self.fallback
    }

    pub fn get_random_template(&self, random: &mut RandomSource) -> &'static StructurePoolElement {
        if self.templates.is_empty() {
            return StructurePoolElement::empty();
        }
        self.templates[random.next_i32_bounded(self.templates.len() as i32) as usize]
    }

    pub fn get_shuffled_templates(
        &self,
        random: &mut RandomSource,
    ) -> Vec<&'static StructurePoolElement> {
        let mut templates = self.templates.clone();
        shuffle(&mut templates, random);
        templates
    }

    pub const fn size(&self) -> usize {
        self.templates.len()
    }
}
