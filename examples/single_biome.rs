use std::env;

use shulkr::util::Key;
use worldgen_rs::{SingleBiomePreset, WorldgenBuilder};

#[path = "common/example.rs"]
mod example;

fn main() {
    example::init_tracing();

    let biome = env::args()
        .nth(1)
        .map_or_else(|| Key::const_vanilla("cherry_grove"), Key::of);

    let worldgen = WorldgenBuilder::new()
        .with_seed(42)
        .with_preset(SingleBiomePreset::new(biome))
        .build()
        .expect("failed to build worldgen");

    example::run(worldgen, [0.5, 150.0, 0.5]);
}
