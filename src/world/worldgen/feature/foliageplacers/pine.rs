use serde::Deserialize;

use crate::util::rng::RandomSource;
use crate::util::value_providers::int::IntProvider;

#[derive(Debug, Deserialize)]
pub struct PineFoliagePlacer {
    pub radius: IntProvider,
    pub offset: IntProvider,
    pub height: IntProvider,
}

impl PineFoliagePlacer {
    pub const fn should_skip_location(
        &self,
        _random: &mut RandomSource,
        dx: i32,
        _y: i32,
        dz: i32,
        current_radius: i32,
        _double_trunk: bool,
    ) -> bool {
        dx == current_radius && dz == current_radius && current_radius > 0
    }
}
