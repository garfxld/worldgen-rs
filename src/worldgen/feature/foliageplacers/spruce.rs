use serde::Deserialize;

use crate::util::rng::RandomSource;
use crate::util::value_providers::int::IntProvider;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpruceFoliagePlacer {
    pub radius: IntProvider,
    pub offset: IntProvider,
    pub trunk_height: IntProvider,
}

impl SpruceFoliagePlacer {
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
