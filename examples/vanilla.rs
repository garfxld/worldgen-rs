use worldgen_rs::WorldgenBuilder;

#[path = "common/example.rs"]
mod example;

fn main() {
    example::init_tracing();

    let worldgen = WorldgenBuilder::new()
        .with_seed(42)
        .build()
        .expect("failed to build worldgen");

    example::run(worldgen, [0.5, 150.0, 0.5]);
}
