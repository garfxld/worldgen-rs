use serde::Deserialize;

use crate::util::rng::RandomSource;
use crate::util::value_providers::int::IntProvider;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FancyFoliagePlacer {
    pub radius: IntProvider,
    pub offset: IntProvider,
    pub height: i32,
}

impl FancyFoliagePlacer {
    pub fn should_skip_location(
        &self,
        _random: &mut RandomSource,
        dx: i32,
        _y: i32,
        dz: i32,
        current_radius: i32,
        _double_trunk: bool,
    ) -> bool {
        let dx = dx as f32 + 0.5;
        let dz = dz as f32 + 0.5;
        dx * dx + dz * dz > (current_radius * current_radius) as f32
    }
}
