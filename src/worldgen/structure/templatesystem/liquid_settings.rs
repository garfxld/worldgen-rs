use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
pub enum LiquidSettings {
    #[serde(rename = "ignore_waterlogging")]
    IgnoreWaterlogging,
    #[default]
    #[serde(rename = "apply_waterlogging")]
    ApplyWaterlogging,
}
