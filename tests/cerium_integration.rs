//! Exercises the worldgen-rs -> cerium generator hook end to end, without a
//! real network client: `World::load_chunk` calls the hook synchronously.

use cerium::Server;
use cerium::auth::AuthMode;
use cerium::world::block::{Block, BlockState};
use cerium::world::chunk::Chunk;
use cerium::world::{DimensionType, World};
use worldgen_rs::registry::REGISTRY;
use worldgen_rs::{ChunkPos, ChunkStatus, Identifier, Worldgen, WorldgenSettings};

#[expect(
    clippy::similar_names,
    reason = "world_x0/world_z0 are the chunk's block-space origin, x vs z"
)]
fn fill_chunk(worldgen: &Worldgen, chunk: &Chunk) {
    let pos = ChunkPos::new(chunk.x(), chunk.z());
    worldgen
        .generate(pos, ChunkStatus::Surface, |view| {
            let min_y = view.min_y();
            let height = view.height();
            let world_x0 = chunk.x() * 16;
            let world_z0 = chunk.z() * 16;

            for relative_y in 0..height as usize {
                let world_y = min_y + relative_y as i32;
                for local_x in 0..16usize {
                    for local_z in 0..16usize {
                        let Some(state) = view.get_relative_block(local_x, relative_y, local_z)
                        else {
                            continue;
                        };
                        if state.is_air() {
                            continue;
                        }
                        let Some(block) = REGISTRY.blocks.by_state_id(state) else {
                            continue;
                        };
                        let key = format!("{}:{}", block.key.namespace, block.key.path);
                        let Some(cerium_state) = BlockState::from_key(key) else {
                            continue;
                        };
                        chunk.set_block(
                            world_x0 + local_x as i32,
                            world_y,
                            world_z0 + local_z as i32,
                            &cerium_state,
                        );
                    }
                }
            }
        })
        .expect("chunk should generate to Surface");
}

#[test]
fn cerium_world_generator_hook_fills_terrain() {
    // cerium's Server holds process-global state (CURRENT: OnceLock), so at
    // most one test in this binary may construct it.
    let _server = Server::new(AuthMode::Offline);

    let worldgen = Worldgen::new(WorldgenSettings {
        seed: 42,
        dimension: Identifier::const_vanilla("overworld"),
    })
    .expect("worldgen should build");

    let world = World::with_generator(DimensionType::OVERWORLD, move |chunk: &Chunk| {
        fill_chunk(&worldgen, chunk);
    });

    let chunk = world.load_chunk(0, 0);
    assert_eq!(chunk.x(), 0);
    assert_eq!(chunk.z(), 0);

    // seed 42, chunk (0,0), column (0,0): water down to y=55, stone from y=42
    // (cross-checked against the old steel-worldgen tree).
    let water = world.get_block(0, 60, 0);
    assert_eq!(water.block(), Block::Water);

    let stone = world.get_block(0, 30, 0);
    assert_eq!(stone.block(), Block::Stone);

    // Near the top of the world, nothing was generated -> still air.
    let sky = world.get_block(0, 300, 0);
    assert_eq!(sky.block(), Block::Air);
}
