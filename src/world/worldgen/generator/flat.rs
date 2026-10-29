use std::sync::atomic::{AtomicUsize, Ordering};

use shulkr::util::Key;
use shulkr::world::block::{Block, BlockState};

use crate::preset::FlatSettings;
use crate::registry::biome::BiomeRef;
use crate::world::chunk::heightmap::{Heightmap, HeightmapType};
use crate::world::chunk::proto::Chunk;
use crate::world::worldgen::feature::features::Feature;
use crate::world::worldgen::feature::features::fill_layer::FillLayerFeature;
use crate::world::worldgen::feature::{PlacedFeature, features};
use crate::world::worldgen::generator::GENERATION_STEPS;

const LAKES: usize = 1;
const UNDERGROUND_STRUCTURES: usize = 3;
const SURFACE_STRUCTURES: usize = 4;
const TOP_LAYER_MODIFICATION: usize = 10;

static NEXT_INLINE_FEATURE_ID: AtomicUsize = AtomicUsize::new(usize::MAX / 2);

fn inline_fill_layer(height: usize, state: BlockState) -> &'static PlacedFeature {
    let feature: &'static Feature = Box::leak(Box::new(Feature::FillLayer(FillLayerFeature {
        height: height as i32,
        state,
    })));
    Box::leak(Box::new(PlacedFeature {
        id: NEXT_INLINE_FEATURE_ID.fetch_add(1, Ordering::Relaxed),
        feature,
        placement: Vec::new(),
    }))
}

pub struct FlatGenerator {
    layers: Vec<Option<BlockState>>,
    structure_overrides: Option<Vec<Key>>,
    biome: BiomeRef,
    features: Vec<Vec<&'static PlacedFeature>>,
}

impl FlatGenerator {
    pub fn new(settings: &FlatSettings, biome: BiomeRef) -> Self {
        let mut layers: Vec<Option<BlockState>> = settings
            .layers
            .iter()
            .flat_map(|&(state, height)| (0..height).map(move |_| Some(state)))
            .collect();
        let void = layers.iter().flatten().all(|state| state.is(Block::AIR));

        let mut steps: Vec<Vec<&'static PlacedFeature>> = vec![Vec::new(); GENERATION_STEPS];
        if settings.lakes {
            for key in ["lake_lava_underground", "lake_lava_surface"] {
                if let Some(lake) = features().placed(&Key::const_vanilla(key)) {
                    steps[LAKES].push(lake);
                }
            }
        }
        let decorate =
            (!void || biome.key() == &Key::const_vanilla("the_void")) && settings.features;
        if decorate {
            for (step, placed) in features().features_for_biome(biome).iter().enumerate() {
                if step == UNDERGROUND_STRUCTURES
                    || step == SURFACE_STRUCTURES
                    || (settings.lakes && step == LAKES)
                {
                    continue;
                }
                if steps.len() <= step {
                    steps.resize(step + 1, Vec::new());
                }
                steps[step].extend(placed.iter().copied());
            }
        }
        for (index, layer) in layers.iter_mut().enumerate() {
            if let Some(state) = *layer
                && !HeightmapType::MotionBlocking.is_opaque(state)
            {
                *layer = None;
                steps[TOP_LAYER_MODIFICATION].push(inline_fill_layer(index, state));
            }
        }

        Self {
            layers,
            structure_overrides: settings.structure_overrides.clone(),
            biome,
            features: steps,
        }
    }

    pub fn features_for_biome(&self, biome: BiomeRef) -> &[Vec<&'static PlacedFeature>] {
        if biome.id() == self.biome.id() {
            &self.features
        } else {
            features().features_for_biome(biome)
        }
    }

    pub fn structure_overrides(&self) -> Option<&[Key]> {
        self.structure_overrides.as_deref()
    }

    // FlatLevelSource.buildTerrain
    pub fn build_terrain(&self, chunk: &Chunk) {
        let count = self.layers.len().min(chunk.height() as usize);
        let mut world_surface = Heightmap::new(HeightmapType::WorldSurfaceWg, chunk.min_y());
        let mut ocean_floor = Heightmap::new(HeightmapType::OceanFloorWg, chunk.min_y());
        let mut blocks = Vec::with_capacity(256);
        for (index, layer) in self.layers[..count].iter().enumerate() {
            let Some(state) = *layer else {
                continue;
            };
            let y = chunk.min_y() + index as i32;
            blocks.clear();
            for x in 0..16 {
                for z in 0..16 {
                    blocks.push((x, index, z, state));
                    world_surface.update_for_initial_fill(x, y, z, state);
                    ocean_floor.update_for_initial_fill(x, y, z, state);
                }
            }
            chunk.write_block_batch_for_generation(&blocks);
        }
        chunk.replace_noise_heightmaps(ocean_floor, world_surface);
    }

    // FlatLevelSource.getBaseColumn
    pub fn base_column(&self, min_y: i32) -> (i32, Vec<BlockState>) {
        let air = Block::AIR.default_state();
        (
            min_y,
            self.layers
                .iter()
                .map(|layer| layer.unwrap_or(air))
                .collect(),
        )
    }

    // ChunkGenerator.getFirstFreeHeight
    pub fn first_free_height(&self, min_y: i32, heightmap: HeightmapType) -> i32 {
        let surface = matches!(
            heightmap,
            HeightmapType::WorldSurface | HeightmapType::WorldSurfaceWg
        );
        let top = self.layers.iter().rposition(|layer| {
            layer.is_some_and(|state| {
                !state.is_air() && (surface || !(state.is(Block::WATER) || state.is(Block::LAVA)))
            })
        });
        top.map_or(min_y, |index| min_y + index as i32 + 1)
    }
}
