use serde::Deserialize;

use crate::util::rng::RandomSource;
use crate::util::value_providers::int::IntProvider;

#[derive(Debug, Deserialize)]
pub struct MegaPineFoliagePlacer {
    pub radius: IntProvider,
    pub offset: IntProvider,
    pub crown_height: IntProvider,
}

impl MegaPineFoliagePlacer {
    pub const fn should_skip_location(
        &self,
        _random: &mut RandomSource,
        dx: i32,
        _y: i32,
        dz: i32,
        current_radius: i32,
        _double_trunk: bool,
    ) -> bool {
        dx + dz >= 7 || dx * dx + dz * dz > current_radius * current_radius
    }
}
