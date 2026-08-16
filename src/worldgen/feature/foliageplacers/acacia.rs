use serde::Deserialize;

use crate::util::rng::RandomSource;
use crate::util::value_providers::int::IntProvider;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcaciaFoliagePlacer {
    pub radius: IntProvider,
    pub offset: IntProvider,
}

impl AcaciaFoliagePlacer {
    pub const fn should_skip_location(
        &self,
        _random: &mut RandomSource,
        dx: i32,
        y: i32,
        dz: i32,
        current_radius: i32,
        _double_trunk: bool,
    ) -> bool {
        if y == 0 {
            (dx > 1 || dz > 1) && dx != 0 && dz != 0
        } else {
            dx == current_radius && dz == current_radius && current_radius > 0
        }
    }
}
