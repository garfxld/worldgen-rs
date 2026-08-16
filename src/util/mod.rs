pub mod axis;
pub mod datapack;
pub mod direction;
pub mod geometry;
pub mod hash_set;
pub mod ident;
pub mod inclusive_range;
pub mod math;
pub mod mirror;
pub mod noise;
pub mod pos;
pub mod rng;
pub mod rotation;
pub mod shapes;
pub mod value_providers;

use crate::util::rng::RandomSource;

pub fn shuffle<T>(list: &mut [T], random: &mut RandomSource) {
    let mut i = list.len() as i32;
    while i > 1 {
        let swap_to = random.next_i32_bounded(i) as usize;
        list.swap((i - 1) as usize, swap_to);
        i -= 1;
    }
}
