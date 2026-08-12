use std::env;

use worldgen_rs::registry::REGISTRY;
use worldgen_rs::{ChunkPos, ChunkStatus, Identifier, Worldgen, WorldgenSettings};

fn parse_args() -> (i64, i32, i32) {
    let mut seed = 42_i64;
    let mut chunk = (0_i32, 0_i32);

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--seed" => {
                seed = args
                    .next()
                    .expect("--seed needs a value")
                    .parse()
                    .expect("--seed must be an integer");
            }
            "--chunk" => {
                let value = args.next().expect("--chunk needs a value");
                let (x, z) = value.split_once(',').expect("--chunk must be x,z");
                chunk = (
                    x.parse().expect("chunk x must be an integer"),
                    z.parse().expect("chunk z must be an integer"),
                );
            }
            other => panic!("unknown argument {other}"),
        }
    }

    (seed, chunk.0, chunk.1)
}

fn main() {
    let (seed, chunk_x, chunk_z) = parse_args();

    let worldgen = Worldgen::new(WorldgenSettings {
        seed,
        dimension: Identifier::const_vanilla("overworld"),
    })
    .expect("worldgen should build");

    worldgen
        .generate(
            ChunkPos::new(chunk_x, chunk_z),
            ChunkStatus::Surface,
            |view| {
                for local_x in [0usize, 8, 15] {
                    for local_z in [0usize, 8, 15] {
                        print!("({local_x},{local_z}):");
                        let mut printed = 0;
                        for relative_y in (0..view.height() as usize).rev() {
                            let Some(state) = view.get_relative_block(local_x, relative_y, local_z)
                            else {
                                continue;
                            };
                            if state.is_air() {
                                continue;
                            }
                            let name = REGISTRY
                                .blocks
                                .by_state_id(state)
                                .map_or_else(|| "?".to_owned(), |block| block.key.path.to_string());
                            let world_y = view.min_y() + relative_y as i32;
                            print!(" {world_y}:{name}");
                            printed += 1;
                            if printed >= 8 {
                                break;
                            }
                        }
                        println!();
                    }
                }
            },
        )
        .expect("chunk should generate to Surface");
}
