use serde::Deserialize;

use crate::util::rng::RandomSource;
use crate::util::value_providers::int::IntProvider;

#[derive(Debug, Deserialize)]
pub struct BlobFoliagePlacer {
    pub radius: IntProvider,
    pub offset: IntProvider,
    pub height: i32,
}

impl BlobFoliagePlacer {
    pub fn should_skip_location(
        &self,
        random: &mut RandomSource,
        dx: i32,
        y: i32,
        dz: i32,
        current_radius: i32,
        _double_trunk: bool,
    ) -> bool {
        dx == current_radius && dz == current_radius && (random.next_i32_bounded(2) == 0 || y == 0)
    }
}
