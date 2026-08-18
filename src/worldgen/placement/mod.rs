use std::sync::LazyLock;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer};

use smallvec::SmallVec;

use crate::chunk::heightmap::HeightmapType;
use crate::registry::REGISTRY;
use crate::registry::blocks::vanilla_blocks;
use crate::util::axis::Axis;
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
        self.region.get_block(pos)
    }

    pub fn get_biome(&self, pos: BlockPos) -> u16 {
        fuzzed_biome_at_block(self.biome_zoom_seed, pos, |quart| {
            self.region.get_noise_biome(quart.x, quart.y, quart.z)
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaveSurface {
    Ceiling,
    Floor,
}

impl CaveSurface {
    pub const fn direction(self) -> Direction {
        match self {
            Self::Ceiling => Direction::Up,
            Self::Floor => Direction::Down,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum PlacementModifier {
    #[serde(rename = "minecraft:count")]
    Count { count: IntProvider },
    #[serde(rename = "minecraft:noise_threshold_count")]
    NoiseThresholdCount {
        noise_level: f64,
        below_noise: i32,
        above_noise: i32,
    },
    #[serde(rename = "minecraft:noise_based_count")]
    NoiseBasedCount {
        noise_to_count_ratio: i32,
        noise_factor: f64,
        #[serde(default)]
        noise_offset: f64,
    },
    #[serde(rename = "minecraft:in_square")]
    InSquare,
    #[serde(rename = "minecraft:height_range")]
    HeightRange { height: HeightProvider },
    #[serde(rename = "minecraft:heightmap")]
    Heightmap { heightmap: HeightmapType },
    #[serde(rename = "minecraft:random_offset")]
    RandomOffset {
        xz_spread: IntProvider,
        y_spread: IntProvider,
    },
    #[serde(rename = "minecraft:count_on_every_layer")]
    CountOnEveryLayer { count: IntProvider },
    #[serde(rename = "minecraft:environment_scan")]
    EnvironmentScan {
        #[serde(deserialize_with = "vertical_direction")]
        direction_of_search: Direction,
        target_condition: BlockPredicate,
        #[serde(default = "always_true")]
        allowed_search_condition: BlockPredicate,
        max_steps: i32,
    },
    #[serde(rename = "minecraft:fixed_placement")]
    FixedPlacement { positions: Vec<BlockPos> },
    #[serde(rename = "minecraft:biome")]
    BiomeFilter,
    #[serde(rename = "minecraft:rarity_filter")]
    RarityFilter { chance: i32 },
    #[serde(rename = "minecraft:block_predicate_filter")]
    BlockPredicateFilter { predicate: BlockPredicate },
    #[serde(rename = "minecraft:surface_water_depth_filter")]
    SurfaceWaterDepthFilter { max_water_depth: i32 },
    #[serde(rename = "minecraft:surface_relative_threshold_filter")]
    SurfaceRelativeThresholdFilter {
        heightmap: HeightmapType,
        #[serde(default = "i32_min")]
        min_inclusive: i32,
        #[serde(default = "i32_max")]
        max_inclusive: i32,
    },
}

const fn always_true() -> BlockPredicate {
    BlockPredicate::AlwaysTrue
}

const fn i32_min() -> i32 {
    i32::MIN
}

const fn i32_max() -> i32 {
    i32::MAX
}

fn vertical_direction<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Direction, D::Error> {
    let direction = Direction::deserialize(deserializer)?;
    if direction.get_axis() == Axis::Y {
        return Ok(direction);
    }
    Err(D::Error::custom(format!(
        "{direction:?} is not a vertical direction"
    )))
}

impl PlacementModifier {
    pub fn positions(
        &self,
        context: &PlacementContext<'_, '_>,
        random: &mut RandomSource,
        origin: BlockPos,
        out: &mut Positions,
    ) {
        match self {
            Self::Count { count } => repeat(count.sample(random), origin, out),
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
            Self::HeightRange { height } => {
                let y = height.sample(random, context.min_y(), context.gen_depth());
                out.push(origin.at_y(y));
            }
            Self::Heightmap { heightmap } => {
                let height = context.get_height(*heightmap, origin.0.x, origin.0.z);
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
            Self::CountOnEveryLayer { count } => {
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
            Self::FixedPlacement { positions } => {
                let chunk_x = origin.0.x >> 4;
                let chunk_z = origin.0.z >> 4;
                let in_chunk = |pos: &&BlockPos| pos.0.x >> 4 == chunk_x && pos.0.z >> 4 == chunk_z;
                if positions.iter().any(|pos| in_chunk(&pos)) {
                    out.extend(positions.iter().filter(in_chunk).copied());
                }
            }
            Self::BiomeFilter => {
                let place = match context.top_feature {
                    Some(top_feature) => biome_has_feature(context.get_biome(origin), top_feature),
                    None => true,
                };
                if place {
                    out.push(origin);
                }
            }
            Self::RarityFilter { chance } => {
                if random.next_f32() < 1.0 / *chance as f32 {
                    out.push(origin);
                }
            }
            Self::BlockPredicateFilter { predicate } => {
                if predicate.test(context.region, origin) {
                    out.push(origin);
                }
            }
            Self::SurfaceWaterDepthFilter { max_water_depth } => {
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

#[cfg(test)]
mod tests {
    use crate::util::ident::Identifier;
    use crate::worldgen::dimension::DimensionSpec;

    fn spec(source: &DatapackSource, dimension: &str) -> DimensionSpec {
        let id = Identifier::new(Identifier::VANILLA_NAMESPACE, dimension.to_owned());
        crate::worldgen::dimension::resolve(source, &id).expect("dimension should resolve")
    }

    use std::sync::Once;

    use super::*;
    use crate::chunk::proto::Chunk;
    use crate::chunk::status::ChunkStatus;
    use crate::registry::Registries;
    use crate::util::pos::ChunkPos;
    use crate::util::rng::worldgen_random::WorldgenRandom;
    use crate::worldgen::backend::ir::loader::DatapackSource;
    use crate::worldgen::feature::features;
    use crate::worldgen::generator::Generator;
    use crate::worldgen::scheduler::ChunkStore;

    fn init_global_registry() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let mut registries = Registries::new_vanilla().expect("vanilla datapack should load");
            registries.freeze();
            let _ = REGISTRY.init(registries);
        });
    }

    #[test]
    fn ore_placement_chain_yields_in_chunk_positions() {
        init_global_registry();

        let source = DatapackSource::builtin();
        let generator = Generator::new(&source, &spec(&source, "overworld"), 42)
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
        let min_y = chunks[4].min_y();
        let height = chunks[4].height();

        let generator = store.generator();
        let region = WorldGenRegion::new(
            center,
            1,
            chunks,
            min_y,
            height,
            generator.sea_level(),
            generator.seed(),
            generator.biome_zoom_seed(),
            generator.region_random(center),
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
