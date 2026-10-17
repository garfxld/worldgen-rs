use serde::Deserialize;

use crate::util::rng::RandomSource;
use crate::util::value_providers::int::IntProvider;

#[derive(Debug, Deserialize)]
pub struct RandomSpreadFoliagePlacer {
    pub radius: IntProvider,
    pub offset: IntProvider,
    pub foliage_height: IntProvider,
    pub leaf_placement_attempts: i32,
}

impl RandomSpreadFoliagePlacer {
    pub const fn should_skip_location(
        &self,
        _random: &mut RandomSource,
        _dx: i32,
        _y: i32,
        _dz: i32,
        _current_radius: i32,
        _double_trunk: bool,
    ) -> bool {
        false
    }
}
