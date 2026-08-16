use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TwoLayersFeatureSize {
    #[serde(default = "default_limit")]
    pub limit: i32,
    #[serde(default)]
    pub lower_size: i32,
    #[serde(default = "default_upper_size")]
    pub upper_size: i32,
    #[serde(default)]
    pub min_clipped_height: Option<i32>,
}

const fn default_limit() -> i32 {
    1
}

const fn default_upper_size() -> i32 {
    1
}

impl TwoLayersFeatureSize {
    pub const fn get_size_at_height(&self, yo: i32) -> i32 {
        if yo < self.limit {
            self.lower_size
        } else {
            self.upper_size
        }
    }
}
