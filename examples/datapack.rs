use std::collections::HashMap;
use std::path::{Path, PathBuf};
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
use worldgen_rs::datapack::Datapack;
use worldgen_rs::registry::REGISTRY;
use worldgen_rs::{ChunkPos, ChunkStatus, Identifier, Worldgen, WorldgenSettings};

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

fn build_biome_id_table(server: &cerium::Server) -> std::collections::HashMap<u16, i32> {
    let registries = server.registries();
    REGISTRY
        .biomes
        .iter()
        .filter_map(|(id, biome)| {
            let key = format!("{}:{}", biome.key.namespace, biome.key.path);
            let cerium_id = registries
                .biome
                .get_id(RegistryKey::<CeriumBiome>::of(key))?;
            Some((id as u16, i32::from(cerium_id)))
        })
        .collect()
}

const SPAWN: (f64, f64, f64) = (0.5, 150.0, 0.5);

const DEFAULT_FLYING_SPEED: f32 = 0.05;
const MIN_FLY_MULTIPLIER: f32 = 0.1;
const MAX_FLY_MULTIPLIER: f32 = 20.0;

fn fill_chunk(worldgen: &Worldgen, chunk: &Chunk, biome_ids: &HashMap<u16, i32>) {
    let pos = ChunkPos::new(chunk.x(), chunk.z());
    worldgen
        .generate(pos, ChunkStatus::Features, |view| {
            chunk.fill_biomes(view.biomes().filter_map(|(pos, biome)| {
                biome_ids
                    .get(&biome)
                    .map(|&id| (pos.x(), pos.y(), pos.z(), id))
            }));
            chunk.fill_blocks(view.blocks().filter_map(|(pos, state)| {
                BlockState::from_id(state.0).map(|s| (pos.x(), pos.y(), pos.z(), s))
            }));
        })
        .expect("chunk should generate to Features");
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "worldgen_rs=debug,cranelift_codegen=warn,cranelift_jit=warn,cranelift_frontend=warn,cranelift_module=warn,info"
                    .into()
            }),
        )
        .init();

    let mut args = std::env::args().skip(1);
    let path = PathBuf::from(args.next().unwrap_or_else(|| panic!("{USAGE}")));
    let dimension: Identifier = args
        .next()
        .unwrap_or_else(|| "minecraft:overworld".to_owned())
        .parse()
        .expect("dimension should be a valid identifier");

    let datapack = load_datapack(&path);
    tracing::info!(
        "loaded {} (namespaces: {:?})",
        path.display(),
        datapack.namespaces()
    );

    let worldgen = Arc::new(
        Worldgen::new(WorldgenSettings::new(42, dimension.clone()).with_datapack(datapack))
            .unwrap_or_else(|error| panic!("worldgen should build for {dimension}: {error}")),
    );

    let server = Server::new(AuthMode::Offline);
    let biome_ids = Arc::new(build_biome_id_table(&server));
    let total_biomes = REGISTRY.biomes.iter().count();
    tracing::info!(
        "{} of {total_biomes} worldgen biomes mapped into cerium's registry",
        biome_ids.len()
    );
    if biome_ids.len() < total_biomes {
        tracing::warn!(
            "{} datapack biomes are missing from cerium's registry",
            total_biomes - biome_ids.len()
        );
    }

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
