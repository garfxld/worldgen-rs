use worldgen_rs::{LargeBiomesPreset, WorldgenBuilder};

#[path = "common/example.rs"]
mod example;

fn main() {
    example::init_tracing();

    let worldgen = WorldgenBuilder::new()
        .with_seed(42)
        .with_preset(LargeBiomesPreset::new())
        .build()
        .expect("failed to build worldgen");

    example::run(worldgen, [0.5, 150.0, 0.5]);
}
