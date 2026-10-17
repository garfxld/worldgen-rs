use std::collections::{BTreeMap, BTreeSet};

use rustc_hash::FxHashMap;

use crate::world::worldgen::feature::PlacedFeature;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct FeatureData {
    step: usize,
    feature_index: usize,
}

pub struct StepFeatureData {
    pub features: Vec<&'static PlacedFeature>,
    index_mapping: FxHashMap<usize, usize>,
}

impl StepFeatureData {
    fn new(features: Vec<&'static PlacedFeature>) -> Self {
        let index_mapping = features
            .iter()
            .enumerate()
            .map(|(index, feature)| (feature.id, index))
            .collect();
        Self {
            features,
            index_mapping,
        }
    }

    pub fn index_of(&self, feature: &'static PlacedFeature) -> Option<usize> {
        self.index_mapping.get(&feature.id).copied()
    }
}

pub fn build_features_per_step<'f, T>(
    feature_sources: &[T],
    feature_getter: impl Fn(&T) -> &'f [Vec<&'static PlacedFeature>],
) -> Vec<StepFeatureData> {
    let mut feature_index: FxHashMap<usize, usize> = FxHashMap::default();
    let mut features_by_data: FxHashMap<FeatureData, &'static PlacedFeature> = FxHashMap::default();
    let mut next_feature_index = 0;
    let mut edges: BTreeMap<FeatureData, BTreeSet<FeatureData>> = BTreeMap::new();
    let mut max_step = 0;

    for source in feature_sources {
        let mut feature_list = Vec::new();
        let features_for_step = feature_getter(source);
        max_step = max_step.max(features_for_step.len());

        for (step, features) in features_for_step.iter().enumerate() {
            for &feature in features {
                let index = *feature_index.entry(feature.id).or_insert_with(|| {
                    let index = next_feature_index;
                    next_feature_index += 1;
                    index
                });
                let data = FeatureData {
                    step,
                    feature_index: index,
                };
                features_by_data.insert(data, feature);
                feature_list.push(data);
            }
        }

        for index in 0..feature_list.len() {
            let entry = edges.entry(feature_list[index]).or_default();
            if index + 1 < feature_list.len() {
                entry.insert(feature_list[index + 1]);
            }
        }
    }

    let mut discovered: BTreeSet<FeatureData> = BTreeSet::new();
    let mut currently_visiting: BTreeSet<FeatureData> = BTreeSet::new();
    let mut sorted_features: Vec<FeatureData> = Vec::new();

    let keys: Vec<FeatureData> = edges.keys().copied().collect();
    for data in keys {
        if !discovered.contains(&data)
            && depth_first_search(
                &edges,
                &mut discovered,
                &mut currently_visiting,
                &mut sorted_features,
                data,
            )
        {
            panic!("feature order cycle found");
        }
    }

    sorted_features.reverse();

    (0..max_step)
        .map(|step| {
            StepFeatureData::new(
                sorted_features
                    .iter()
                    .filter(|data| data.step == step)
                    .map(|data| features_by_data[data])
                    .collect(),
            )
        })
        .collect()
}

fn depth_first_search(
    edges: &BTreeMap<FeatureData, BTreeSet<FeatureData>>,
    discovered: &mut BTreeSet<FeatureData>,
    currently_visiting: &mut BTreeSet<FeatureData>,
    output: &mut Vec<FeatureData>,
    current: FeatureData,
) -> bool {
    if discovered.contains(&current) {
        return false;
    }
    if !currently_visiting.insert(current) {
        return true;
    }

    if let Some(neighbors) = edges.get(&current) {
        for &neighbor in neighbors {
            if depth_first_search(edges, discovered, currently_visiting, output, neighbor) {
                return true;
            }
        }
    }

    currently_visiting.remove(&current);
    discovered.insert(current);
    output.push(current);
    false
}
