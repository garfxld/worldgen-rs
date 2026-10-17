use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ThreeLayersFeatureSize {
    #[serde(default = "default_limit")]
    pub limit: i32,
    #[serde(default = "default_upper_limit")]
    pub upper_limit: i32,
    #[serde(default)]
    pub lower_size: i32,
    #[serde(default = "default_middle_size")]
    pub middle_size: i32,
    #[serde(default = "default_upper_size")]
    pub upper_size: i32,
    #[serde(default)]
    pub min_clipped_height: Option<i32>,
}

const fn default_limit() -> i32 {
    1
}

const fn default_upper_limit() -> i32 {
    1
}

const fn default_middle_size() -> i32 {
    1
}

const fn default_upper_size() -> i32 {
    1
}

impl ThreeLayersFeatureSize {
    pub const fn get_size_at_height(&self, tree_height: i32, yo: i32) -> i32 {
        if yo < self.limit {
            self.lower_size
        } else if yo >= tree_height - self.upper_limit {
            self.upper_size
        } else {
            self.middle_size
        }
    }
}
