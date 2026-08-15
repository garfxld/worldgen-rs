use std::sync::LazyLock;

use serde_json::Value;

use smallvec::SmallVec;

use crate::chunk::heightmap::HeightmapType;
use crate::registry::REGISTRY;
use crate::registry::blocks::vanilla_blocks;
use crate::util::direction::Direction;
use crate::util::ident::BlockState;
use crate::util::noise::perlin_simplex_noise::PerlinSimplexNoise;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::util::rng::legacy_random::LegacyRandomSource;
use crate::util::value_providers::height::HeightProvider;
use crate::util::value_providers::int::IntProvider;
use crate::worldgen::block_predicate::BlockPredicate;
use crate::worldgen::feature::{PlacedFeature, features};
use crate::worldgen::generator::fuzzed_biome::fuzzed_biome_at_block;
use crate::worldgen::json::{
    parse_direction, parse_heightmap_type, parse_json, type_of, value_list,
};
use crate::worldgen::region::WorldGenRegion;

static BIOME_INFO_NOISE: LazyLock<PerlinSimplexNoise> = LazyLock::new(|| {
    let mut random = RandomSource::Legacy(LegacyRandomSource::from_seed(2345));
    PerlinSimplexNoise::new(&mut random, &[0])
});

pub type Positions = SmallVec<[BlockPos; 8]>;

pub struct PlacementContext<'a, 'b> {
    pub region: &'a WorldGenRegion<'b>,
    pub top_feature: Option<&'static PlacedFeature>,
    biome_zoom_seed: i64,
}

impl<'a, 'b> PlacementContext<'a, 'b> {
    pub const fn new(region: &'a WorldGenRegion<'b>, biome_zoom_seed: i64) -> Self {
        Self {
            region,
            top_feature: None,
            biome_zoom_seed,
        }
    }

    pub const fn with_top_feature(mut self, top_feature: &'static PlacedFeature) -> Self {
        self.top_feature = Some(top_feature);
        self
    }

    pub fn get_height(&self, heightmap_type: HeightmapType, x: i32, z: i32) -> i32 {
        self.region.get_height(heightmap_type, x, z)
    }

    pub const fn min_y(&self) -> i32 {
        self.region.min_y()
    }

    pub const fn gen_depth(&self) -> i32 {
        self.region.max_y() - self.region.min_y()
    }

    pub fn get_block_state(&self, pos: BlockPos) -> BlockState {
        self.region.get_block_state(pos)
    }

    pub fn get_biome(&self, pos: BlockPos) -> u16 {
        fuzzed_biome_at_block(self.biome_zoom_seed, pos, |quart| {
            self.region.get_noise_biome(quart.x, quart.y, quart.z)
        })
    }
}

#[derive(Debug, Clone)]
pub enum PlacementModifier {
    Count(IntProvider),
    NoiseThresholdCount {
        noise_level: f64,
        below_noise: i32,
        above_noise: i32,
    },
    NoiseBasedCount {
        noise_to_count_ratio: i32,
        noise_factor: f64,
        noise_offset: f64,
    },
    InSquare,
    HeightRange(HeightProvider),
    Heightmap(HeightmapType),
    RandomOffset {
        xz_spread: IntProvider,
        y_spread: IntProvider,
    },
    CountOnEveryLayer(IntProvider),
    EnvironmentScan {
        direction_of_search: Direction,
        target_condition: BlockPredicate,
        allowed_search_condition: BlockPredicate,
        max_steps: i32,
    },
    FixedPlacement(Vec<BlockPos>),
    BiomeFilter,
    RarityFilter(i32),
    BlockPredicateFilter(BlockPredicate),
    SurfaceWaterDepthFilter(i32),
    SurfaceRelativeThresholdFilter {
        heightmap: HeightmapType,
        min_inclusive: i32,
        max_inclusive: i32,
    },
}

impl PlacementModifier {
    pub fn parse(value: &Value) -> Self {
        match type_of(value) {
            "minecraft:count" => Self::Count(parse_json(&value["count"])),
            "minecraft:noise_threshold_count" => Self::NoiseThresholdCount {
                noise_level: as_f64(value, "noise_level"),
                below_noise: as_i32(value, "below_noise"),
                above_noise: as_i32(value, "above_noise"),
            },
            "minecraft:noise_based_count" => Self::NoiseBasedCount {
                noise_to_count_ratio: as_i32(value, "noise_to_count_ratio"),
                noise_factor: as_f64(value, "noise_factor"),
                noise_offset: value
                    .get("noise_offset")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0),
            },
            "minecraft:in_square" => Self::InSquare,
            "minecraft:height_range" => Self::HeightRange(parse_json(&value["height"])),
            "minecraft:heightmap" => Self::Heightmap(parse_heightmap_type(&value["heightmap"])),
            "minecraft:random_offset" => Self::RandomOffset {
                xz_spread: parse_json(&value["xz_spread"]),
                y_spread: parse_json(&value["y_spread"]),
            },
            "minecraft:count_on_every_layer" => {
                Self::CountOnEveryLayer(parse_json(&value["count"]))
            }
            "minecraft:environment_scan" => Self::EnvironmentScan {
                direction_of_search: parse_direction(value.get("direction_of_search")),
                target_condition: BlockPredicate::parse(&value["target_condition"]),
                allowed_search_condition: value
                    .get("allowed_search_condition")
                    .map_or(BlockPredicate::AlwaysTrue, BlockPredicate::parse),
                max_steps: as_i32(value, "max_steps"),
            },
            "minecraft:fixed_placement" => Self::FixedPlacement(
                value_list(&value["positions"])
                    .iter()
                    .map(parse_block_pos)
                    .collect(),
            ),
            "minecraft:biome" => Self::BiomeFilter,
            "minecraft:rarity_filter" => Self::RarityFilter(as_i32(value, "chance")),
            "minecraft:block_predicate_filter" => {
                Self::BlockPredicateFilter(BlockPredicate::parse(&value["predicate"]))
            }
            "minecraft:surface_water_depth_filter" => {
                Self::SurfaceWaterDepthFilter(as_i32(value, "max_water_depth"))
            }
            "minecraft:surface_relative_threshold_filter" => Self::SurfaceRelativeThresholdFilter {
                heightmap: parse_heightmap_type(&value["heightmap"]),
                min_inclusive: value
                    .get("min_inclusive")
                    .and_then(Value::as_i64)
                    .unwrap_or(i64::from(i32::MIN)) as i32,
                max_inclusive: value
                    .get("max_inclusive")
                    .and_then(Value::as_i64)
                    .unwrap_or(i64::from(i32::MAX)) as i32,
            },
            other => panic!("unknown placement modifier type {other}"),
        }
    }

    pub fn positions(
        &self,
        context: &PlacementContext<'_, '_>,
        random: &mut RandomSource,
        origin: BlockPos,
        out: &mut Positions,
    ) {
        match self {
            Self::Count(count) => repeat(count.sample(random), origin, out),
            Self::NoiseThresholdCount {
                noise_level,
                below_noise,
                above_noise,
            } => {
                let noise = BIOME_INFO_NOISE
                    .get_value(f64::from(origin.0.x) / 200.0, f64::from(origin.0.z) / 200.0);
                let count = if noise < *noise_level {
                    *below_noise
                } else {
                    *above_noise
                };
                repeat(count, origin, out);
            }
            Self::NoiseBasedCount {
                noise_to_count_ratio,
                noise_factor,
                noise_offset,
            } => {
                let noise = BIOME_INFO_NOISE.get_value(
                    f64::from(origin.0.x) / noise_factor,
                    f64::from(origin.0.z) / noise_factor,
                );
                let count =
                    ((noise + noise_offset) * f64::from(*noise_to_count_ratio)).ceil() as i32;
                repeat(count, origin, out);
            }
            Self::InSquare => {
                let x = random.next_i32_bounded(16) + origin.0.x;
                let z = random.next_i32_bounded(16) + origin.0.z;
                out.push(BlockPos::new(x, origin.0.y, z));
            }
            Self::HeightRange(height) => {
                let y = height.sample(random, context.min_y(), context.gen_depth());
                out.push(origin.at_y(y));
            }
            Self::Heightmap(heightmap_type) => {
                let height = context.get_height(*heightmap_type, origin.0.x, origin.0.z);
                if height > context.min_y() {
                    out.push(BlockPos::new(origin.0.x, height, origin.0.z));
                }
            }
            Self::RandomOffset {
                xz_spread,
                y_spread,
            } => {
                let x = origin.0.x + xz_spread.sample(random);
                let y = origin.0.y + y_spread.sample(random);
                let z = origin.0.z + xz_spread.sample(random);
                out.push(BlockPos::new(x, y, z));
            }
            Self::CountOnEveryLayer(count) => {
                count_on_every_layer(context, random, origin, count, out);
            }
            Self::EnvironmentScan {
                direction_of_search,
                target_condition,
                allowed_search_condition,
                max_steps,
            } => {
                environment_scan(
                    context,
                    origin,
                    *direction_of_search,
                    target_condition,
                    allowed_search_condition,
                    *max_steps,
                    out,
                );
            }
            Self::FixedPlacement(positions) => {
                let chunk_x = origin.0.x >> 4;
                let chunk_z = origin.0.z >> 4;
                let in_chunk = |pos: &&BlockPos| pos.0.x >> 4 == chunk_x && pos.0.z >> 4 == chunk_z;
                if positions.iter().any(|pos| in_chunk(&pos)) {
                    out.extend(positions.iter().filter(in_chunk).copied());
                }
            }
            Self::BiomeFilter => {
                let top_feature = context
                    .top_feature
                    .expect("biome filter on a feature without a top-level placed feature");
                let biome = context.get_biome(origin);
                if biome_has_feature(biome, top_feature) {
                    out.push(origin);
                }
            }
            Self::RarityFilter(chance) => {
                if random.next_f32() < 1.0 / *chance as f32 {
                    out.push(origin);
                }
            }
            Self::BlockPredicateFilter(predicate) => {
                if predicate.test(context.region, origin) {
                    out.push(origin);
                }
            }
            Self::SurfaceWaterDepthFilter(max_water_depth) => {
                let ocean_floor =
                    context.get_height(HeightmapType::OceanFloor, origin.0.x, origin.0.z);
                let world_surface =
                    context.get_height(HeightmapType::WorldSurface, origin.0.x, origin.0.z);
                if world_surface - ocean_floor <= *max_water_depth {
                    out.push(origin);
                }
            }
            Self::SurfaceRelativeThresholdFilter {
                heightmap,
                min_inclusive,
                max_inclusive,
            } => {
                let surface_y = i64::from(context.get_height(*heightmap, origin.0.x, origin.0.z));
                let y = i64::from(origin.0.y);
                if surface_y + i64::from(*min_inclusive) <= y
                    && y <= surface_y + i64::from(*max_inclusive)
                {
                    out.push(origin);
                }
            }
        }
    }
}

fn repeat(count: i32, origin: BlockPos, out: &mut Positions) {
    for _ in 0..count {
        out.push(origin);
    }
}

fn biome_has_feature(biome_id: u16, feature: &'static PlacedFeature) -> bool {
    let Some(biome) = REGISTRY.biomes.by_id(biome_id as usize) else {
        return false;
    };
    features().biome_contains(biome, feature)
}

fn environment_scan(
    context: &PlacementContext<'_, '_>,
    origin: BlockPos,
    direction_of_search: Direction,
    target_condition: &BlockPredicate,
    allowed_search_condition: &BlockPredicate,
    max_steps: i32,
    out: &mut Positions,
) {
    let mut pos = origin;
    if !allowed_search_condition.test(context.region, pos) {
        return;
    }

    for _ in 0..max_steps {
        if target_condition.test(context.region, pos) {
            out.push(pos);
            return;
        }

        pos = pos.relative(direction_of_search);
        if !context.region.is_inside_build_height(pos.0.y) {
            return;
        }

        if !allowed_search_condition.test(context.region, pos) {
            break;
        }
    }

    if target_condition.test(context.region, pos) {
        out.push(pos);
    }
}

fn count_on_every_layer(
    context: &PlacementContext<'_, '_>,
    random: &mut RandomSource,
    origin: BlockPos,
    count: &IntProvider,
    out: &mut Positions,
) {
    let mut layer = 0;
    loop {
        let mut found_any = false;
        let mut index = 0;
        while index < count.sample(random) {
            index += 1;
            let x = random.next_i32_bounded(16) + origin.0.x;
            let z = random.next_i32_bounded(16) + origin.0.z;
            let start_y = context.get_height(HeightmapType::MotionBlocking, x, z);
            if let Some(y) = find_on_ground_y(context, x, start_y, z, layer) {
                out.push(BlockPos::new(x, y, z));
                found_any = true;
            }
        }
        layer += 1;
        if !found_any {
            break;
        }
    }
}

fn find_on_ground_y(
    context: &PlacementContext<'_, '_>,
    x: i32,
    start_y: i32,
    z: i32,
    layer_to_place_on: i32,
) -> Option<i32> {
    let mut current_layer = 0;
    let mut current_block = context.get_block_state(BlockPos::new(x, start_y, z));

    let mut y = start_y;
    while y > context.min_y() {
        let below_block = context.get_block_state(BlockPos::new(x, y - 1, z));
        if !is_empty(below_block) && is_empty(current_block) && !is_bedrock(below_block) {
            if current_layer == layer_to_place_on {
                return Some(y);
            }
            current_layer += 1;
        }
        current_block = below_block;
        y -= 1;
    }
    None
}

fn is_empty(state: BlockState) -> bool {
    let block = state.get_block();
    state.is_air() || block == &vanilla_blocks::WATER || block == &vanilla_blocks::LAVA
}

fn is_bedrock(state: BlockState) -> bool {
    state.get_block() == &vanilla_blocks::BEDROCK
}

fn parse_block_pos(value: &Value) -> BlockPos {
    let components = value_list(value);
    let component = |index: usize| {
        components
            .get(index)
            .and_then(Value::as_i64)
            .unwrap_or_default() as i32
    };
    BlockPos::new(component(0), component(1), component(2))
}

fn as_i32(value: &Value, key: &str) -> i32 {
    value[key]
        .as_i64()
        .unwrap_or_else(|| panic!("missing integer field {key} in {value}")) as i32
}

fn as_f64(value: &Value, key: &str) -> f64 {
    value[key]
        .as_f64()
        .unwrap_or_else(|| panic!("missing float field {key} in {value}"))
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Once;

    use super::*;
    use crate::chunk::proto::Chunk;
    use crate::chunk::status::ChunkStatus;
    use crate::registry::Registries;
    use crate::util::pos::ChunkPos;
    use crate::util::rng::worldgen_random::WorldgenRandom;
    use crate::worldgen::backend::ir::loader::DatapackSource;
    use crate::worldgen::feature::features;
    use crate::worldgen::generator::{BiomeSourceDimension, Generator};
    use crate::worldgen::scheduler::ChunkStore;

    fn init_global_registry() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let mut registries = Registries::new_vanilla().expect("vanilla datapack should load");
            registries.freeze();
            let _ = REGISTRY.init(registries);
        });
    }

    fn datapack_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/datapack/worldgen")
    }

    #[test]
    fn ore_placement_chain_yields_in_chunk_positions() {
        init_global_registry();

        let source = DatapackSource::new(datapack_root());
        let generator = Generator::new(
            &source,
            "overworld",
            "Overworld",
            BiomeSourceDimension::Overworld,
            42,
        )
        .expect("generator should build");
        let store = ChunkStore::new(generator);

        let center = ChunkPos::new(0, 0);
        store.generate_to(&[center], ChunkStatus::Carvers);

        let slots: Vec<_> = (-1..=1)
            .flat_map(|dz| (-1..=1).map(move |dx| (dx, dz)))
            .map(|(dx, dz)| {
                store
                    .get(ChunkPos::new(center.0.x + dx, center.0.y + dz))
                    .expect("neighbor chunk should exist")
            })
            .collect();
        let chunks: Vec<&Chunk> = slots
            .iter()
            .map(|slot| slot.chunk().expect("chunk allocated"))
            .collect();
        let chunks: [&Chunk; 9] = chunks.try_into().unwrap_or_else(|_| panic!("nine chunks"));

        let generator = store.generator();
        let region = WorldGenRegion::new(
            center,
            chunks,
            chunks[4].min_y(),
            chunks[4].height(),
            generator.sea_level(),
            generator.seed(),
            generator.biome_zoom_seed(),
        );

        let feature = features()
            .placed(&"minecraft:ore_dirt".parse().expect("valid id"))
            .expect("ore_dirt should load");

        let context =
            PlacementContext::new(&region, generator.biome_zoom_seed()).with_top_feature(feature);

        let mut random = RandomSource::Worldgen(WorldgenRandom::from_seed(0));
        let decoration_seed = random.set_decoration_seed(generator.seed(), 0, 0);
        random.set_feature_seed(decoration_seed, 0, 6);

        let origin = BlockPos::new(0, region.min_y(), 0);
        let mut current = Positions::new();
        current.push(origin);
        for modifier in &feature.placement {
            let mut next = Positions::new();
            for pos in current.drain(..) {
                modifier.positions(&context, &mut random, pos, &mut next);
            }
            current = next;
        }

        assert!(!current.is_empty());
        for pos in &current {
            assert!((0..16).contains(&pos.0.x), "x out of chunk: {pos:?}");
            assert!((0..16).contains(&pos.0.z), "z out of chunk: {pos:?}");
            assert!(
                region.is_inside_build_height(pos.0.y),
                "y out of world: {pos:?}"
            );
        }
    }
}
