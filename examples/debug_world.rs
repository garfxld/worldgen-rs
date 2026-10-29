use worldgen_rs::{DebugPreset, WorldgenBuilder};

#[path = "common/example.rs"]
mod example;

fn main() {
    example::init_tracing();

    let worldgen = WorldgenBuilder::new()
        .with_preset(DebugPreset::new())
        .build()
        .expect("failed to build worldgen");

    example::run(worldgen, [0.5, 80.0, 0.5]);
}
