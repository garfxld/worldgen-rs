use std::env;

use shulkr::util::Key;
use worldgen_rs::{SuperflatPreset, WorldgenBuilder};

#[path = "common/example.rs"]
mod example;

fn main() {
    example::init_tracing();

    let flat = match env::args().nth(1) {
        Some(preset) => match preset.as_str() {
            "classic_flat" => SuperflatPreset::classic_flat(),
            "tunnelers_dream" => SuperflatPreset::tunnelers_dream(),
            "water_world" => SuperflatPreset::water_world(),
            "overworld" => SuperflatPreset::overworld(),
            "snowy_kingdom" => SuperflatPreset::snowy_kingdom(),
            "bottomless_pit" => SuperflatPreset::bottomless_pit(),
            "desert" => SuperflatPreset::desert(),
            "redstone_ready" => SuperflatPreset::redstone_ready(),
            "the_void" => SuperflatPreset::the_void(),
            _ => SuperflatPreset::from_preset(Key::of(preset)),
        },
        None => SuperflatPreset::new(),
    };

    let worldgen = WorldgenBuilder::new()
        .with_seed(42)
        .with_preset(flat)
        .build()
        .expect("failed to build worldgen");

    example::run(worldgen, [0.5, 0.0, 0.5]);
}
