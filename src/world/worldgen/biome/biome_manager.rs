use shulkr::util::BlockPosition;

use crate::registry::biome::BiomeRef;

use super::obfuscate_biome_seed;

const fn lcg_next(rval: i64, c: i64) -> i64 {
    rval.wrapping_mul(
        rval.wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407),
    )
    .wrapping_add(c)
}

fn get_fiddle(rval: i64) -> f64 {
    let uniform = (rval >> 24).rem_euclid(1024) as f64 / 1024.0;
    (uniform - 0.5) * 0.9
}

fn get_fiddled_distance(
    seed: i64,
    x_random: i32,
    y_random: i32,
    z_random: i32,
    distance_x: f64,
    distance_y: f64,
    distance_z: f64,
) -> f64 {
    let mut rval = seed;
    rval = lcg_next(rval, i64::from(x_random));
    rval = lcg_next(rval, i64::from(y_random));
    rval = lcg_next(rval, i64::from(z_random));
    rval = lcg_next(rval, i64::from(x_random));
    rval = lcg_next(rval, i64::from(y_random));
    rval = lcg_next(rval, i64::from(z_random));
    let fiddle_x = get_fiddle(rval);
    rval = lcg_next(rval, seed);
    let fiddle_y = get_fiddle(rval);
    rval = lcg_next(rval, seed);
    let fiddle_z = get_fiddle(rval);
    let square = |v: f64| v * v;
    square(distance_z + fiddle_z) + square(distance_y + fiddle_y) + square(distance_x + fiddle_x)
}

pub struct BiomeManager<'a, T = BiomeRef> {
    noise_biome_source: &'a dyn Fn(i32, i32, i32) -> T,
    biome_zoom_seed: i64,
}

impl<'a, T> BiomeManager<'a, T> {
    pub const fn new(
        noise_biome_source: &'a dyn Fn(i32, i32, i32) -> T,
        biome_zoom_seed: i64,
    ) -> Self {
        Self {
            noise_biome_source,
            biome_zoom_seed,
        }
    }

    pub fn obfuscate_seed(seed: i64) -> i64 {
        obfuscate_biome_seed(seed)
    }

    pub fn get_biome(&self, pos: BlockPosition) -> T {
        let abs_x = pos.x() - 2;
        let abs_y = pos.y() - 2;
        let abs_z = pos.z() - 2;
        let parent_x = abs_x >> 2;
        let parent_y = abs_y >> 2;
        let parent_z = abs_z >> 2;
        let fract_x = f64::from(abs_x & 3) / 4.0;
        let fract_y = f64::from(abs_y & 3) / 4.0;
        let fract_z = f64::from(abs_z & 3) / 4.0;
        let mut min_i = 0;
        let mut min_fiddled_distance = f64::INFINITY;

        for i in 0..8 {
            let x_even = (i & 4) == 0;
            let y_even = (i & 2) == 0;
            let z_even = (i & 1) == 0;
            let corner_x = if x_even { parent_x } else { parent_x + 1 };
            let corner_y = if y_even { parent_y } else { parent_y + 1 };
            let corner_z = if z_even { parent_z } else { parent_z + 1 };
            let distance_x = if x_even { fract_x } else { fract_x - 1.0 };
            let distance_y = if y_even { fract_y } else { fract_y - 1.0 };
            let distance_z = if z_even { fract_z } else { fract_z - 1.0 };
            let next = get_fiddled_distance(
                self.biome_zoom_seed,
                corner_x,
                corner_y,
                corner_z,
                distance_x,
                distance_y,
                distance_z,
            );
            if min_fiddled_distance > next {
                min_i = i;
                min_fiddled_distance = next;
            }
        }

        let biome_x = if (min_i & 4) == 0 {
            parent_x
        } else {
            parent_x + 1
        };
        let biome_y = if (min_i & 2) == 0 {
            parent_y
        } else {
            parent_y + 1
        };
        let biome_z = if (min_i & 1) == 0 {
            parent_z
        } else {
            parent_z + 1
        };
        (self.noise_biome_source)(biome_x, biome_y, biome_z)
    }
}
