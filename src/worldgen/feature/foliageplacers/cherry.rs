use serde::Deserialize;

use crate::util::rng::RandomSource;
use crate::util::value_providers::int::IntProvider;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CherryFoliagePlacer {
    pub radius: IntProvider,
    pub offset: IntProvider,
    pub height: IntProvider,
    pub wide_bottom_layer_hole_chance: f32,
    pub corner_hole_chance: f32,
    pub hanging_leaves_chance: f32,
    pub hanging_leaves_extension_chance: f32,
}

impl CherryFoliagePlacer {
    pub fn should_skip_location(
        &self,
        random: &mut RandomSource,
        dx: i32,
        y: i32,
        dz: i32,
        current_radius: i32,
        _double_trunk: bool,
    ) -> bool {
        if y == -1
            && (dx == current_radius || dz == current_radius)
            && random.next_f32() < self.wide_bottom_layer_hole_chance
        {
            return true;
        }

        let corner = dx == current_radius && dz == current_radius;
        let wide_layer = current_radius > 2;
        if wide_layer {
            corner || (dx + dz > current_radius * 2 - 2 && random.next_f32() < self.corner_hole_chance)
        } else {
            corner && random.next_f32() < self.corner_hole_chance
        }
    }
}
