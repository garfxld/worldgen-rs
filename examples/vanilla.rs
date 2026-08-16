use std::sync::Arc;

use cerium::Server;
use cerium::auth::AuthMode;
use cerium::command::arg::Arg;
use cerium::command::arg::kind::ArgKind;
use cerium::command::{Command, dispatcher::CommandError};
use cerium::entity::GameMode;
use cerium::event::player::{
    CommandResultEvent, PlayerConfigEvent, PlayerEvent, PlayerRequestGameModeEvent,
    PlayerSpawnEvent,
};
use cerium::registry::RegistryKey;
use cerium::world::biome::Biome as CeriumBiome;
use cerium::world::block::BlockState;
use cerium::world::chunk::Chunk;
use cerium::world::{DimensionType, World};
use worldgen_rs::registry::REGISTRY;
use worldgen_rs::{ChunkPos, ChunkStatus, Identifier, Worldgen, WorldgenSettings};

fn build_biome_id_table(server: &cerium::Server) -> std::collections::HashMap<u16, i32> {
    let registries = server.registries();
    REGISTRY
        .biomes
        .iter()
        .filter_map(|(id, biome)| {
            let key = format!("{}:{}", biome.key.namespace, biome.key.path);
            let cerium_id = registries.biome.get_id(RegistryKey::<CeriumBiome>::of(key))?;
            Some((id as u16, i32::from(cerium_id)))
        })
        .collect()
}

const SPAWN: (f64, f64, f64) = (0.5, 150.0, 0.5);

const DEFAULT_FLYING_SPEED: f32 = 0.05;
const MIN_FLY_MULTIPLIER: f32 = 0.1;
const MAX_FLY_MULTIPLIER: f32 = 20.0;

#[expect(
    clippy::similar_names,
    reason = "world_x0/world_z0 are the chunk's block-space origin, x vs z"
)]
fn fill_chunk(worldgen: &Worldgen, chunk: &Chunk, biome_ids: &std::collections::HashMap<u16, i32>) {
    let pos = ChunkPos::new(chunk.x(), chunk.z());
    worldgen
        .generate(pos, ChunkStatus::Features, |view| {
            let min_y = view.min_y();
            let height = view.height();
            let world_x0 = chunk.x() * 16;
            let world_z0 = chunk.z() * 16;

            for relative_y in (0..height as usize).step_by(4) {
                let world_y = min_y + relative_y as i32;
                for local_x in (0..16usize).step_by(4) {
                    for local_z in (0..16usize).step_by(4) {
                        let Some(worldgen_biome) =
                            view.get_relative_biome(local_x, relative_y, local_z)
                        else {
                            continue;
                        };
                        let Some(&cerium_biome) = biome_ids.get(&worldgen_biome) else {
                            continue;
                        };
                        chunk.set_biome(
                            world_x0 + local_x as i32,
                            world_y,
                            world_z0 + local_z as i32,
                            cerium_biome,
                        );
                    }
                }
            }

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
                        let Some(cerium_state) = BlockState::from_id(state.0) else {
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

fn main() {
    let worldgen = Arc::new(
        Worldgen::new(WorldgenSettings {
            seed: 42,
            dimension: Identifier::const_vanilla("overworld"),
        })
        .expect("worldgen should build"),
    );

    let server = Server::new(AuthMode::Offline);
    let biome_ids = Arc::new(build_biome_id_table(&server));
    let world = World::with_generator(DimensionType::OVERWORLD, move |chunk: &Chunk| {
        fill_chunk(&worldgen, chunk, &biome_ids);
    });

    let dispatcher = server.command_dispatcher();
    dispatcher.register(Command::new("flyspeed").arg(Arg::new(
        "multiplier",
        ArgKind::Float {
            min: Some(MIN_FLY_MULTIPLIER),
            max: Some(MAX_FLY_MULTIPLIER),
        },
    )));

    server
        .events()
        .subscribe(move |event: &mut PlayerConfigEvent| {
            event.set_world(world.clone());
            event.set_position(SPAWN);
        })
        .subscribe(|event: &mut PlayerSpawnEvent| {
            event.get_player().set_game_mode(GameMode::Spectator);
            event.get_player().set_permission_level(4);
        })
        .subscribe(|event: &mut PlayerRequestGameModeEvent| {
            event.get_player().set_game_mode(event.requested_game_mode());
        })
        .subscribe(|event: &mut CommandResultEvent| {
            if let Some(matches) = event.matches() {
                if matches.command_name() != "flyspeed" {
                    return;
                }
                let Some(multiplier) = matches.get::<f32>("multiplier") else {
                    return;
                };

                let player = event.get_player();
                player.set_flying_speed(DEFAULT_FLYING_SPEED * multiplier);
                player.send_message(format!("fly speed set to {multiplier}x"));
                return;
            }

            let Some(error) = event.error() else {
                return;
            };
            let message = match error {
                CommandError::InvalidArgument { command, value, .. } if command == "flyspeed" => {
                    format!("`{value}` is not a fly speed between {MIN_FLY_MULTIPLIER} and {MAX_FLY_MULTIPLIER}")
                }
                CommandError::MissingArgument { command, .. } if command == "flyspeed" => {
                    format!("usage: /flyspeed <{MIN_FLY_MULTIPLIER}-{MAX_FLY_MULTIPLIER}>")
                }
                _ => return,
            };
            event.get_player().send_message(message);
        });

    server.bind("127.0.0.1:25565").unwrap()
}
