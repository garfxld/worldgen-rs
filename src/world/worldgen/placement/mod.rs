use crate::world::worldgen::worldgen_context::WorldGenContext;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer};

use shulkr::world::block::{Block, BlockState};
use smallvec::SmallVec;

use crate::registry::biome::biome_info_noise;
use crate::util::rng::RandomSource;
use crate::util::value_providers::height::HeightProvider;
use crate::util::value_providers::int::IntProvider;
use crate::world::chunk::heightmap::HeightmapType;
use crate::world::worldgen::block_predicate::BlockPredicate;
use crate::world::worldgen::feature::{PlacedFeature, features};
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::registry::registries;
use shulkr::util::Axis;
use shulkr::util::BlockPosition;
use shulkr::util::Direction;

pub type Positions = SmallVec<[BlockPosition; 8]>;

pub struct PlacementContext<'a, 'b> {
    pub region: &'a WorldGenRegion<'b>,
    pub top_feature: Option<&'static PlacedFeature>,
}

impl<'a, 'b> PlacementContext<'a, 'b> {
    pub const fn new(region: &'a WorldGenRegion<'b>) -> Self {
        Self {
            region,
            top_feature: None,
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

    pub const fn worldgen_context(&self) -> WorldGenContext {
        self.region.worldgen_context()
    }

    pub fn get_block_state(&self, pos: BlockPosition) -> BlockState {
        self.region.get_block(pos)
    }

    pub fn get_biome(&self, pos: BlockPosition) -> u16 {
        self.region.get_biome(pos)
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
    #[serde(rename = "minecraft:offset")]
    Offset {
        x: IntProvider,
        y: IntProvider,
        z: IntProvider,
    },
    #[serde(rename = "minecraft:random_chance")]
    RandomChance { chance: f32 },
    #[serde(rename = "minecraft:randomly_selected")]
    RandomlySelected { placements: Vec<PlacementModifier> },
    #[serde(rename = "minecraft:cuboid")]
    Cuboid {
        xz_size: IntProvider,
        y_size: IntProvider,
        #[serde(default = "always")]
        include_edges: bool,
        #[serde(default = "always")]
        include_interior: bool,
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
    FixedPlacement { positions: Vec<BlockPosition> },
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

const fn always() -> bool {
    true
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
        origin: BlockPosition,
        out: &mut Positions,
    ) {
        match self {
            Self::Count { count } => repeat(count.sample(random), origin, out),
            Self::NoiseThresholdCount {
                noise_level,
                below_noise,
                above_noise,
            } => {
                let noise = f64::from(
                    biome_info_noise()
                        .get2(f64::from(origin.x()) / 200.0, f64::from(origin.z()) / 200.0),
                );
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
                let noise = f64::from(biome_info_noise().get2(
                    f64::from(origin.x()) / noise_factor,
                    f64::from(origin.z()) / noise_factor,
                ));
                let count =
                    ((noise + noise_offset) * f64::from(*noise_to_count_ratio)).ceil() as i32;
                repeat(count, origin, out);
            }
            Self::InSquare => {
                let x = random.next_i32_bounded(16) + origin.x();
                let z = random.next_i32_bounded(16) + origin.z();
                out.push(BlockPosition::new(x, origin.y(), z));
            }
            Self::HeightRange { height } => {
                let y = height.sample(random, &context.worldgen_context());
                out.push(origin.at_y(y));
            }
            Self::Heightmap { heightmap } => {
                let height = context.get_height(*heightmap, origin.x(), origin.z());
                if height > context.min_y() {
                    out.push(BlockPosition::new(origin.x(), height, origin.z()));
                }
            }
            Self::Offset { x, y, z } => {
                let dx = x.sample(random);
                let dy = y.sample(random);
                let dz = z.sample(random);
                out.push(origin.offset(dx, dy, dz));
            }
            Self::RandomChance { chance } => {
                if random.next_f32() < *chance {
                    out.push(origin);
                }
            }
            Self::RandomlySelected { placements } => {
                let index = random.next_i32_bounded(placements.len() as i32) as usize;
                placements[index].positions(context, random, origin, out);
            }
            Self::Cuboid {
                xz_size,
                y_size,
                include_edges,
                include_interior,
            } => cuboid(
                random,
                origin,
                xz_size,
                y_size,
                *include_edges,
                *include_interior,
                out,
            ),
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
                let chunk_x = origin.x() >> 4;
                let chunk_z = origin.z() >> 4;
                let in_chunk =
                    |pos: &&BlockPosition| pos.x() >> 4 == chunk_x && pos.z() >> 4 == chunk_z;
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
                    context.get_height(HeightmapType::OceanFloor, origin.x(), origin.z());
                let world_surface =
                    context.get_height(HeightmapType::WorldSurface, origin.x(), origin.z());
                if world_surface - ocean_floor <= *max_water_depth {
                    out.push(origin);
                }
            }
            Self::SurfaceRelativeThresholdFilter {
                heightmap,
                min_inclusive,
                max_inclusive,
            } => {
                let surface_y = i64::from(context.get_height(*heightmap, origin.x(), origin.z()));
                let y = i64::from(origin.y());
                if surface_y + i64::from(*min_inclusive) <= y
                    && y <= surface_y + i64::from(*max_inclusive)
                {
                    out.push(origin);
                }
            }
        }
    }
}

fn cuboid(
    random: &mut RandomSource,
    origin: BlockPosition,
    xz_size: &IntProvider,
    y_size: &IntProvider,
    include_edges: bool,
    include_interior: bool,
    out: &mut Positions,
) {
    let height = y_size.sample(random);
    let width = xz_size.sample(random);
    let length = xz_size.sample(random);
    for x in 0..=width {
        for y in 0..=height {
            for z in 0..=length {
                let x_edge = x == 0 || x == width;
                let y_edge = y == 0 || y == height;
                let z_edge = z == 0 || z == length;
                if (include_edges || !x_edge || !y_edge)
                    && (include_edges || !z_edge || !y_edge)
                    && (include_edges || !x_edge || !z_edge)
                    && (include_interior || x_edge || y_edge || z_edge)
                {
                    out.push(origin.offset(x, y, z));
                }
            }
        }
    }
}

fn repeat(count: i32, origin: BlockPosition, out: &mut Positions) {
    for _ in 0..count {
        out.push(origin);
    }
}

fn biome_has_feature(biome_id: u16, feature: &'static PlacedFeature) -> bool {
    let Some(biome) = registries::BIOME.by_id(biome_id) else {
        return false;
    };
    features().biome_contains(biome, feature)
}

fn environment_scan(
    context: &PlacementContext<'_, '_>,
    origin: BlockPosition,
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
        if !context.region.is_inside_build_height(pos.y()) {
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
    origin: BlockPosition,
    count: &IntProvider,
    out: &mut Positions,
) {
    let mut layer = 0;
    loop {
        let mut found_any = false;
        let mut index = 0;
        while index < count.sample(random) {
            index += 1;
            let x = random.next_i32_bounded(16) + origin.x();
            let z = random.next_i32_bounded(16) + origin.z();
            let start_y = context.get_height(HeightmapType::MotionBlocking, x, z);
            if let Some(y) = find_on_ground_y(context, x, start_y, z, layer) {
                out.push(BlockPosition::new(x, y, z));
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
    let mut current_block = context.get_block_state(BlockPosition::new(x, start_y, z));

    let mut y = start_y;
    while y > context.min_y() {
        let below_block = context.get_block_state(BlockPosition::new(x, y - 1, z));
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
    state.is_air() || state.is(Block::WATER) || state.is(Block::LAVA)
}

fn is_bedrock(state: BlockState) -> bool {
    state.is(Block::BEDROCK)
}

#[cfg(test)]
mod tests {
    use crate::datapack;
    use crate::world::worldgen;
    use crate::world::worldgen::dimension;
    use crate::world::worldgen::dimension::DimensionSpec;
    use shulkr::util::Key;

    fn spec(source: &Datapack, dimension: &str) -> DimensionSpec {
        let id = Key::new(Key::VANILLA_NAMESPACE, dimension.to_owned());
        dimension::resolve(source, &id).expect("dimension should resolve")
    }

    use std::sync::Once;

    use super::*;
    use crate::datapack::Datapack;
    use crate::util::pos::ChunkPos;
    use crate::util::rng::worldgen_random::WorldgenRandom;
    use crate::world::chunk::proto::Chunk;
    use crate::world::chunk::status::ChunkStatus;
    use crate::world::worldgen::feature::features;
    use crate::world::worldgen::generator::Generator;
    use crate::world::worldgen::scheduler::ChunkStore;

    fn init_global_registry() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            worldgen::ensure_registry_initialized(&datapack::Datapack::builtin())
                .expect("vanilla datapack should load");
        });
    }

    #[test]
    fn ore_placement_chain_yields_in_chunk_positions() {
        init_global_registry();

        let source = Datapack::builtin();
        let generator = Generator::new(&source, spec(&source, "overworld"), 42)
            .expect("generator should build");
        let store = ChunkStore::new(generator);

        let center = ChunkPos::new(0, 0);
        store.generate_to(&[center], ChunkStatus::Terrain);

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
        let generation_context = store.generator().generation_context(min_y, height);

        let generator = store.generator();
        let region = WorldGenRegion::new(
            center,
            1,
            chunks,
            min_y,
            height,
            generation_context,
            generator.seed(),
            generator.biome_zoom_seed(),
            generator.region_random(center),
        );

        let feature = features()
            .placed(&Key::of("minecraft:ore_dirt".to_owned()))
            .expect("ore_dirt should load");

        let context = PlacementContext::new(&region).with_top_feature(feature);

        let mut random = RandomSource::Worldgen(WorldgenRandom::from_seed(0));
        let decoration_seed = random.set_decoration_seed(generator.seed(), 0, 0);
        random.set_feature_seed(decoration_seed, 0, 6);

        let origin = BlockPosition::new(0, region.min_y(), 0);
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
            assert!((0..16).contains(&pos.x()), "x out of chunk: {pos:?}");
            assert!((0..16).contains(&pos.z()), "z out of chunk: {pos:?}");
            assert!(
                region.is_inside_build_height(pos.y()),
                "y out of world: {pos:?}"
            );
        }
    }
}
