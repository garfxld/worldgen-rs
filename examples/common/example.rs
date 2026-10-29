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
use shulkr::world::chunk::Chunk;
use shulkr::world::{DimensionType, World};
use worldgen_rs::{ChunkPos, ChunkStatus, Worldgen};

const DEFAULT_FLYING_SPEED: f32 = 0.05;
const MIN_FLY_MULTIPLIER: f32 = 0.1;
const MAX_FLY_MULTIPLIER: f32 = 20.0;

pub fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "worldgen_rs=debug,cranelift_codegen=warn,cranelift_jit=warn,cranelift_frontend=warn,cranelift_module=warn,info"
                    .into()
            }),
        )
        .init();
}

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

pub fn run(worldgen: Worldgen, spawn: [f64; 3]) {
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
            event.set_position(spawn);
        })
        .subscribe(|event: &mut PlayerSpawnEvent| {
            let player = event.get_player();
            player.set_game_mode(GameMode::Spectator);
            player.set_permission_level(4);
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
