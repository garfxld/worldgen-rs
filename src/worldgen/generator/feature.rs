use rustc_hash::FxHashSet;

use crate::registry::REGISTRY;
use crate::util::pos::{BlockPos, ChunkPos};
use crate::util::rng::RandomSource;
use crate::util::rng::worldgen_random::WorldgenRandom;
use crate::worldgen::feature::features;
use crate::worldgen::generator::{GENERATION_STEPS, Generator};
use crate::worldgen::region::WorldGenRegion;



impl Generator {
    pub fn apply_biome_decoration(&self, region: &WorldGenRegion<'_>) {
        let center = region.center();
        let origin = BlockPos::new(center.0.x * 16, region.min_y(), center.0.y * 16);

        let mut random = RandomSource::Worldgen(WorldgenRandom::from_seed(0));
        let decoration_seed = random.set_decoration_seed(self.seed, origin.0.x, origin.0.z);

        let mut possible_biomes: FxHashSet<u16> = FxHashSet::default();
        for dz in -1..=1 {
            for dx in -1..=1 {
                let Some(chunk) = region.chunk(ChunkPos::new(center.0.x + dx, center.0.y + dz))
                else {
                    continue;
                };
                chunk.sections().for_each_biome_palette_entry(|biome_id| {
                    possible_biomes.insert(biome_id);
                });
            }
        }
        possible_biomes.retain(|biome_id| self.possible_biome_ids.contains(biome_id));

        let feature_step_count = self.features_per_step.len();
        let generation_steps = GENERATION_STEPS.max(feature_step_count);

        let mut features_this_step: Vec<usize> = Vec::new();
        for step in 0..generation_steps {
            self.place_structures_for_step(region, &mut random, decoration_seed, step);

            if step >= feature_step_count {
                continue;
            }
            let step_data = &self.features_per_step[step];

            features_this_step.clear();
            for &biome_id in &possible_biomes {
                let Some(biome) = REGISTRY.biomes.by_id(biome_id as usize) else {
                    continue;
                };
                let features_in_biome = features().features_for_biome(biome);
                let Some(features_in_step) = features_in_biome.get(step) else {
                    continue;
                };
                for &feature in features_in_step {
                    if let Some(index) = step_data.index_of(feature) {
                        features_this_step.push(index);
                    }
                }
            }
            features_this_step.sort_unstable();
            features_this_step.dedup();

            for &global_index in &features_this_step {
                let feature = step_data.features[global_index];
                random.set_feature_seed(decoration_seed, global_index as i32, step as i32);
                feature.place_with_biome_check(region, &mut random, origin);
            }
        }
    }
}
