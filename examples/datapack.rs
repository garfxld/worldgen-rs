use std::env;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use shulkr::Server;
use shulkr::auth::AuthMode;
use shulkr::command::arg::Arg;
use shulkr::command::arg::kind::ArgKind;
use shulkr::command::{Command, dispatcher::CommandError};
use shulkr::entity::GameMode;
use shulkr::event::player::{
    CommandResultEvent, PlayerConfigEvent, PlayerEvent, PlayerRequestGameModeEvent,
    PlayerSpawnEvent,
};
use shulkr::util::Key;
use shulkr::world::chunk::Chunk;
use shulkr::world::{DimensionType, World};
use worldgen_rs::datapack::Datapack;
use worldgen_rs::{ChunkPos, ChunkStatus, Worldgen, WorldgenSettings};

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

const SPAWN: [f64; 3] = [0.5, 150.0, 0.5];

const DEFAULT_FLYING_SPEED: f32 = 0.05;
const MIN_FLY_MULTIPLIER: f32 = 0.1;
const MAX_FLY_MULTIPLIER: f32 = 20.0;

fn fill_chunk(worldgen: &Worldgen, chunk: &Chunk) {
    let pos = ChunkPos::new(chunk.x(), chunk.z());
    worldgen
        .generate(pos, ChunkStatus::Features, |view| {
            chunk.fill_biomes(
                view.biomes()
                    .map(|(pos, biome)| (pos.x(), pos.y(), pos.z(), i32::from(biome))),
            );
            chunk.fill_blocks(
                view.blocks()
                    .map(|(pos, state)| (pos.x(), pos.y(), pos.z(), state)),
            );
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

    let mut args = env::args().skip(1);
    let path = PathBuf::from(args.next().unwrap_or_else(|| panic!("{USAGE}")));
    let dimension = Key::try_from(
        args.next()
            .unwrap_or_else(|| "minecraft:overworld".to_owned()),
    )
    .expect("dimension should be a valid identifier");

    let datapack = load_datapack(&path);

    let worldgen = Arc::new(
        Worldgen::new(WorldgenSettings::new(42, dimension.clone()).with_datapack(datapack))
            .unwrap_or_else(|error| panic!("worldgen should build for {dimension}: {error}")),
    );

    let server = Server::new(AuthMode::Offline);

    let world = World::with_generator(DimensionType::OVERWORLD, move |chunk: &Chunk| {
        fill_chunk(&worldgen, chunk);
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

    server
        .bind("127.0.0.1:25565")
        .expect("failed binding to 127.0.0.1:25565")
}
