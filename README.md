# worldgen-rs

A Rust library for generating Minecraft worlds. It reads vanilla and custom datapacks, JIT-compiles their density functions, and generates chunks that match vanilla's output.

![vanilla](assets/vanilla.jpg)


## Status

This project is in a very experimental state. The API is not stable and is subject to change.

Generation runs through `ChunkStatus::Features`. `InitializeLight`, `Light`, `Spawn`, and `Full` are not implemented and return `Error::Unsupported`.



## Features

- Loads worldgen from a datapack directory or `.zip`, layered over vanilla data
- Density functions JIT-compiled to native code with [Cranelift](https://cranelift.dev/)
- Biomes, noise, surface rules, carvers, features, and structures
- Overworld, nether, and end, plus datapack-defined dimensions
- Parallel chunk generation on a Rayon pool
- World presets (e.g. amplified, large biomes)


## Installation

This crate requires a nightly toolchain, since it uses `portable_simd` and several `const` trait features. The pinned version is in `rust-toolchain.toml`.

```toml
[dependencies]
worldgen-rs = "0.1.3"
```



## Usage

```rust
use worldgen_rs::{ChunkPos, ChunkStatus, WorldgenBuilder};

let worldgen = WorldgenBuilder::new()
    .with_seed(42)
    .build()
    .expect("failed to build worldgen");

let top = worldgen
    .generate(ChunkPos::new(0, 0), ChunkStatus::Features, |view| {
        view.get_relative_block(0, (view.height() - 1) as usize, 0)
    })
    .expect("failed to generate chunk");
```

`generate` takes a closure rather than returning the chunk, so the chunk data is read under the store's lock and never copied out wholesale. `ChunkView` exposes block and biome lookups, the dimension's `min_y` and `height`, and iterators over both.

To use a custom datapack, layer it over the built-in one:

```rust
use worldgen_rs::{WorldgenBuilder, datapack::Datapack};

let datapack = Datapack::zip("Terralith.zip")
    .expect("failed to load Terralith datapack")
    .layered_over(Datapack::builtin());

let worldgen = WorldgenBuilder::new()
    .with_seed(42)
    .with_datapack(datapack)
    .build()
    .expect("failed to build worldgen");
```



## Examples

The examples use [shulkr](https://github.com/shulkr-rs/shulkr) to start a real server you can join.

```bash
cargo r -r --example vanilla
cargo r -r --example datapack <datapack.zip|directory> [dimension]
```

There are also examples for every world preset:

```bash
cargo r -r --example flat_world [water_world|tunnelers_dream|the_void|...]
cargo r -r --example debug_world
cargo r -r --example amplified
cargo r -r --example large_biomes
cargo r -r --example single_biome [biome]
```



## Roadmap

- [x] Structure starts and references
- [x] Biomes
- [x] Noise
- [x] Surface rules
- [x] Carvers
- [x] Features
- [x] Datapack loading (directory and `.zip`, layered)
- [x] JIT-compiled density functions
- [x] Structures, including jigsaw assembly
- [x] Overworld, nether, and end, plus datapack-defined dimensions
- [x] Parallel chunk generation
- [ ] Lighting
- [ ] Loot tables
- [ ] Entity spawning
- [ ] Performance improvements



## License

This project is licensed under [Apache-2.0](LICENSE).
