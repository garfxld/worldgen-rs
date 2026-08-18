use serde::Deserialize;

use crate::util::rng::RandomSource;
use crate::util::value_providers::int::IntProvider;

#[derive(Debug, Deserialize)]
pub struct DarkOakFoliagePlacer {
    pub radius: IntProvider,
    pub offset: IntProvider,
}

impl DarkOakFoliagePlacer {
    pub const fn should_skip_location(
        &self,
        _random: &mut RandomSource,
        dx: i32,
        y: i32,
        dz: i32,
        current_radius: i32,
        double_trunk: bool,
    ) -> bool {
        if y == -1 && !double_trunk {
            dx == current_radius && dz == current_radius
        } else if y == 1 {
            dx + dz > current_radius * 2 - 2
        } else {
            false
        }
    }
}
