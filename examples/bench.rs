use std::sync::Arc;
use std::time::Instant;

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use worldgen_rs::datapack::Datapack;
use worldgen_rs::{ChunkPos, ChunkStatus, Identifier, Worldgen, WorldgenSettings};

const USAGE: &str = "usage: bench [side] [threads] [status]\n  \
    side:    grid width in chunks (default 33)\n  \
    threads: rayon threads, 0 = default (default 0)\n  \
    status:  structstarts|structrefs|biomes|noise|surface|carvers|features (default features)\n\
    env BENCH_SERIAL=1  drive chunks from one thread instead of a rayon pool\n\
    env BENCH_HASH=1    print a parity hash of all blocks and biomes instead of timings";

fn parse<T: std::str::FromStr>(value: Option<String>, default: T) -> T {
    value.and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn status_of(name: &str) -> ChunkStatus {
    match name {
        "structstarts" => ChunkStatus::StructureStarts,
        "structrefs" => ChunkStatus::StructureReferences,
        "biomes" => ChunkStatus::Biomes,
        "noise" => ChunkStatus::Noise,
        "surface" => ChunkStatus::Surface,
        "carvers" => ChunkStatus::Carvers,
        "features" => ChunkStatus::Features,
        other => panic!("unknown status `{other}`\n{USAGE}"),
    }
}

fn chunk_hash(worldgen: &Worldgen, pos: ChunkPos, status: ChunkStatus) -> u64 {
    worldgen
        .generate(pos, status, |view| {
            let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
            let mut mix = |word: u64| {
                hash ^= word;
                hash = hash.wrapping_mul(0x100_0000_01b3);
            };
            for (pos, state) in view.blocks() {
                mix(pos.x() as u64);
                mix(pos.y() as u64);
                mix(pos.z() as u64);
                mix(u64::from(state.0));
            }
            for (pos, biome) in view.biomes() {
                mix(pos.x() as u64);
                mix(pos.y() as u64);
                mix(pos.z() as u64);
                mix(u64::from(biome));
            }
            hash
        })
        .expect("chunk should generate")
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn".into()),
        )
        .init();

    let mut args = std::env::args().skip(1);
    let side: i32 = parse(args.next(), 33);
    let threads: usize = parse(args.next(), 0);
    let status_name = args.next().unwrap_or_else(|| "features".to_owned());
    let status = status_of(&status_name);

    if threads > 0 {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build_global()
            .expect("rayon pool should build");
    }

    let build = Instant::now();
    let worldgen = Arc::new(
        Worldgen::new(WorldgenSettings {
            seed: 42,
            dimension: Identifier::const_vanilla("overworld"),
            datapack: Datapack::builtin(),
        })
        .expect("worldgen should build"),
    );
    let build = build.elapsed();

    let half = side / 2;
    let positions: Vec<ChunkPos> = (-half..=half)
        .flat_map(|z| (-half..=half).map(move |x| ChunkPos::new(x, z)))
        .collect();
    let total = positions.len();

    if std::env::var("BENCH_HASH").is_ok() {
        let hash = positions
            .iter()
            .fold(0u64, |acc, &pos| acc ^ chunk_hash(&worldgen, pos, status));
        println!("hash={hash:016x} chunks={total} status={status_name}");
        return;
    }

    // Warm the JIT and every lazily initialized table well outside the measured grid.
    worldgen
        .generate(ChunkPos::new(9000, 9000), status, |_| ())
        .expect("warmup chunk should generate");

    let serial = std::env::var("BENCH_SERIAL").is_ok();
    let start = Instant::now();
    if serial {
        for &pos in &positions {
            worldgen.generate(pos, status, |_| ()).expect("chunk");
        }
    } else {
        positions.par_iter().for_each(|&pos| {
            worldgen.generate(pos, status, |_| ()).expect("chunk");
        });
    }
    let elapsed = start.elapsed();

    println!(
        "side={side} chunks={total} status={status_name} threads={} serial={serial} build={build:?}",
        rayon::current_num_threads()
    );
    println!(
        "total={:?}  per_chunk={:.3}ms  chunks/s={:.1}",
        elapsed,
        elapsed.as_secs_f64() * 1000.0 / total as f64,
        total as f64 / elapsed.as_secs_f64()
    );
}
