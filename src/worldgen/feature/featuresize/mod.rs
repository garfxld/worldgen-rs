pub mod three_layers;
pub mod two_layers;

use serde::Deserialize;

pub use three_layers::ThreeLayersFeatureSize;
pub use two_layers::TwoLayersFeatureSize;

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum FeatureSize {
    #[serde(rename = "minecraft:two_layers_feature_size")]
    TwoLayers(TwoLayersFeatureSize),
    #[serde(rename = "minecraft:three_layers_feature_size")]
    ThreeLayers(ThreeLayersFeatureSize),
}

impl FeatureSize {
    pub const fn get_size_at_height(&self, tree_height: i32, yo: i32) -> i32 {
        match self {
            Self::TwoLayers(p) => p.get_size_at_height(yo),
            Self::ThreeLayers(p) => p.get_size_at_height(tree_height, yo),
        }
    }

    pub const fn min_clipped_height(&self) -> Option<i32> {
        match self {
            Self::TwoLayers(TwoLayersFeatureSize {
                min_clipped_height, ..
            })
            | Self::ThreeLayers(ThreeLayersFeatureSize {
                min_clipped_height, ..
            }) => *min_clipped_height,
        }
    }
}
