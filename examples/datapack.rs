use std::env;
use std::path::{Path, PathBuf};

use shulkr::util::Key;
use worldgen_rs::WorldgenBuilder;
use worldgen_rs::datapack::Datapack;

#[path = "common/example.rs"]
mod example;

const USAGE: &str =
    "usage: cargo run --release --example datapack <datapack.zip|directory> [dimension]";

fn load_datapack(path: &Path) -> Datapack {
    let pack = if path.is_dir() {
        Datapack::directory(path)
    } else {
        Datapack::zip(path)
    };

    pack.unwrap_or_else(|error| panic!("failed to load {}: {error}", path.display()))
        .layered_over(Datapack::builtin())
}

fn main() {
    example::init_tracing();

    let mut args = env::args().skip(1);
    let path = PathBuf::from(args.next().unwrap_or_else(|| panic!("{USAGE}")));
    let dimension = Key::try_from(
        args.next()
            .unwrap_or_else(|| "minecraft:overworld".to_owned()),
    )
    .expect("dimension should be a valid identifier");

    let worldgen = WorldgenBuilder::new()
        .with_seed(42)
        .with_dimension(dimension)
        .with_datapack(load_datapack(&path))
        .build()
        .expect("failed to build worldgen");

    example::run(worldgen, [0.5, 150.0, 0.5]);
}
