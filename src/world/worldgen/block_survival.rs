use shulkr::util as shapes;
use shulkr::util::Key;
use shulkr::util::SupportType;
use shulkr::world::block::{Block, BlockState, BoolProperty, DoubleBlockHalf, Properties};

use crate::registry::fluid::FluidState;
use crate::world::chunk::heightmap::HeightmapType;
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util::BlockPosition;
use shulkr::util::Direction;

const SUPPORTS_VEGETATION: Key = Key::const_vanilla("supports_vegetation");
const SUPPORTS_DRY_VEGETATION: Key = Key::const_vanilla("supports_dry_vegetation");
const SUPPORTS_WITHER_ROSE: Key = Key::const_vanilla("supports_wither_rose");
const SUPPORTS_CRIMSON_ROOTS: Key = Key::const_vanilla("supports_crimson_roots");
const SUPPORTS_WARPED_ROOTS: Key = Key::const_vanilla("supports_warped_roots");
const SUPPORTS_CRIMSON_FUNGUS: Key = Key::const_vanilla("supports_crimson_fungus");
const SUPPORTS_WARPED_FUNGUS: Key = Key::const_vanilla("supports_warped_fungus");
const SUPPORTS_NETHER_SPROUTS: Key = Key::const_vanilla("supports_nether_sprouts");
const SUPPORTS_AZALEA: Key = Key::const_vanilla("supports_azalea");
const SUPPORTS_LILY_PAD: Key = Key::const_vanilla("supports_lily_pad");
const SUPPORTS_BIG_DRIPLEAF: Key = Key::const_vanilla("supports_big_dripleaf");
const SUPPORTS_SMALL_DRIPLEAF: Key = Key::const_vanilla("supports_small_dripleaf");
const SUPPORT_OVERRIDE_CACTUS_FLOWER: Key = Key::const_vanilla("support_override_cactus_flower");
const OVERRIDES_MUSHROOM_LIGHT_REQUIREMENT: Key =
    Key::const_vanilla("overrides_mushroom_light_requirement");
const WATER: Key = Key::const_vanilla("water");
const SUPPORTS_SUGAR_CANE: Key = Key::const_vanilla("supports_sugar_cane");
const SUPPORTS_SUGAR_CANE_ADJACENTLY: Key = Key::const_vanilla("supports_sugar_cane_adjacently");
const SUPPORTS_CACTUS: Key = Key::const_vanilla("supports_cactus");
const SUPPORTS_MANGROVE_PROPAGULE: Key = Key::const_vanilla("supports_mangrove_propagule");
const SUPPORTS_HANGING_MANGROVE_PROPAGULE: Key =
    Key::const_vanilla("supports_hanging_mangrove_propagule");
const LAVA: Key = Key::const_vanilla("lava");
const CANNOT_SUPPORT_SNOW_LAYER: Key = Key::const_vanilla("cannot_support_snow_layer");
const SUPPORT_OVERRIDE_SNOW_LAYER: Key = Key::const_vanilla("support_override_snow_layer");
const CANNOT_SUPPORT_SEAGRASS: Key = Key::const_vanilla("cannot_support_seagrass");
const SOUL_FIRE_BASE_BLOCKS: Key = Key::const_vanilla("soul_fire_base_blocks");

const HORIZONTAL_OFFSETS: [(i32, i32); 4] = [(0, -1), (0, 1), (-1, 0), (1, 0)];

pub fn can_survive(region: &WorldGenRegion, pos: BlockPosition, state: BlockState) -> bool {
    let block = state.as_block();
    match block.key().path() {
        "cactus" => cactus_can_survive(region, pos, block),
        "sugar_cane" => sugar_cane_can_survive(region, pos, block),
        "mangrove_propagule" => mangrove_propagule_can_survive(region, pos, state),
        "wither_rose" => supported_by_tag(region, pos, &SUPPORTS_WITHER_ROSE),
        "dead_bush" | "short_dry_grass" | "tall_dry_grass" => {
            supported_by_tag(region, pos, &SUPPORTS_DRY_VEGETATION)
        }
        "crimson_roots" => supported_by_tag(region, pos, &SUPPORTS_CRIMSON_ROOTS),
        "warped_roots" => supported_by_tag(region, pos, &SUPPORTS_WARPED_ROOTS),
        "crimson_fungus" => supported_by_tag(region, pos, &SUPPORTS_CRIMSON_FUNGUS),
        "warped_fungus" => supported_by_tag(region, pos, &SUPPORTS_WARPED_FUNGUS),
        "nether_sprouts" => supported_by_tag(region, pos, &SUPPORTS_NETHER_SPROUTS),
        "azalea" | "flowering_azalea" => supported_by_tag(region, pos, &SUPPORTS_AZALEA),
        "brown_mushroom" | "red_mushroom" => mushroom_can_survive(region, pos),
        "lily_pad" => lily_pad_can_survive(region, pos),
        "moss_carpet" => !region.get_block(pos.below()).is_air(),
        "pale_moss_carpet" => mossy_carpet_can_survive(region, pos, state, block),
        "leaf_litter" => sturdy_below(region, pos),
        "cactus_flower" => cactus_flower_can_survive(region, pos),
        "spore_blossom" => spore_blossom_can_survive(region, pos),
        "hanging_roots" => {
            let above = pos.above();
            region.get_block(above).is_side_solid(Direction::Down)
        }
        "big_dripleaf" => big_dripleaf_can_survive(region, pos, block),
        "small_dripleaf" => small_dripleaf_can_survive(region, pos, state, block),
        "snow" => snow_can_survive(region, pos),
        "seagrass" => seagrass_may_place_on(region, pos.below()),
        "tall_seagrass" => tall_seagrass_can_survive(region, pos, state, block),
        "sea_pickle" => sea_pickle_can_survive(region, pos),
        "fire" => sturdy_below(region, pos) || is_valid_fire_location(region, pos),
        "soul_fire" => supported_by_tag(region, pos, &SOUL_FIRE_BASE_BLOCKS),
        path if path.ends_with("_coral_wall_fan") => coral_wall_fan_can_survive(region, pos, state),
        path if path.ends_with("_coral") || path.ends_with("_coral_fan") => {
            sturdy_below(region, pos)
        }
        path if is_double_plant(path) => double_plant_can_survive(region, pos, state, block),
        path if is_vegetation(path) => vegetation_can_survive(region, pos),
        _ => true,
    }
}

pub const fn is_double_plant(path: &str) -> bool {
    matches!(
        path.as_bytes(),
        b"tall_grass"
            | b"large_fern"
            | b"sunflower"
            | b"lilac"
            | b"rose_bush"
            | b"peony"
            | b"pitcher_plant"
            | b"tall_seagrass"
            | b"small_dripleaf"
    )
}

fn is_vegetation(path: &str) -> bool {
    if path.ends_with("_sapling") {
        return true;
    }
    matches!(
        path,
        "short_grass"
            | "fern"
            | "bush"
            | "red_shrub"
            | "firefly_bush"
            | "sweet_berry_bush"
            | "pink_petals"
            | "wildflowers"
            | "dandelion"
            | "golden_dandelion"
            | "torchflower"
            | "poppy"
            | "blue_orchid"
            | "allium"
            | "azure_bluet"
            | "red_tulip"
            | "orange_tulip"
            | "white_tulip"
            | "pink_tulip"
            | "oxeye_daisy"
            | "cornflower"
            | "lily_of_the_valley"
            | "open_eyeblossom"
            | "closed_eyeblossom"
    )
}

fn supported_by_tag(region: &WorldGenRegion, pos: BlockPosition, tag: &Key) -> bool {
    block_at(region, pos.below()).has_tag(tag)
}

fn snow_can_survive(region: &WorldGenRegion, pos: BlockPosition) -> bool {
    let below_pos = pos.below();
    let below = region.get_block(below_pos);
    let below_block = below.as_block();
    if below_block.has_tag(&CANNOT_SUPPORT_SNOW_LAYER) {
        return false;
    }
    if below_block.has_tag(&SUPPORT_OVERRIDE_SNOW_LAYER) {
        return true;
    }

    shapes::is_offset_face_full(below.get_collision_shape_at(below_pos), Direction::Up)
        || (below_block == Block::SNOW && below.try_get(Properties::LAYERS) == 8)
}

fn sturdy_below(region: &WorldGenRegion, pos: BlockPosition) -> bool {
    let below = pos.below();
    region
        .get_block(below)
        .is_side_solid_at(below, Direction::Up)
}

fn double_plant_can_survive(
    region: &WorldGenRegion,
    pos: BlockPosition,
    state: BlockState,
    block: Block,
) -> bool {
    if !is_upper_half(state) {
        return vegetation_can_survive(region, pos);
    }
    let below = region.get_block(pos.below());
    below.is(block) && !is_upper_half(below)
}

fn is_upper_half(state: BlockState) -> bool {
    state
        .get(Properties::DOUBLE_BLOCK_HALF)
        .is_some_and(|half| half == DoubleBlockHalf::Upper)
}

fn mushroom_can_survive(region: &WorldGenRegion, pos: BlockPosition) -> bool {
    let below_pos = pos.below();
    let below = region.get_block(below_pos);
    if below
        .as_block()
        .has_tag(&OVERRIDES_MUSHROOM_LIGHT_REQUIREMENT)
    {
        return true;
    }
    if pos.y() >= region.get_height(HeightmapType::WorldSurfaceWg, pos.x(), pos.z()) {
        return false;
    }
    below.is_solid_render()
}

fn lily_pad_can_survive(region: &WorldGenRegion, pos: BlockPosition) -> bool {
    if !region.get_block(pos.above()).get_fluid_state().is_empty() {
        return false;
    }
    let below_pos = pos.below();
    let below = region.get_block(below_pos);
    below.get_fluid_state().fluid().has_tag(&SUPPORTS_LILY_PAD)
        || below.as_block().has_tag(&SUPPORTS_LILY_PAD)
}

fn mossy_carpet_can_survive(
    region: &WorldGenRegion,
    pos: BlockPosition,
    state: BlockState,
    block: Block,
) -> bool {
    let below = region.get_block(pos.below());
    if is_carpet_base(state) {
        return !below.is_air();
    }
    below.as_block() == block && is_carpet_base(below)
}

pub fn is_carpet_base(state: BlockState) -> bool {
    state.get(Properties::BOTTOM).unwrap_or(false)
}

fn cactus_flower_can_survive(region: &WorldGenRegion, pos: BlockPosition) -> bool {
    let below_pos = pos.below();
    let below = region.get_block(below_pos);
    below.as_block().has_tag(&SUPPORT_OVERRIDE_CACTUS_FLOWER)
        || below.is_side_solid_for_at(below_pos, Direction::Up, SupportType::Center)
}

fn spore_blossom_can_survive(region: &WorldGenRegion, pos: BlockPosition) -> bool {
    let above_pos = pos.above();
    let above = region.get_block(above_pos);
    above.is_side_solid_for_at(above_pos, Direction::Down, SupportType::Center)
        && !is_water(region.get_block(pos))
}

fn big_dripleaf_can_survive(region: &WorldGenRegion, pos: BlockPosition, block: Block) -> bool {
    let below = region.get_block(pos.below());
    let below_block = below.as_block();
    below_block == block
        || below_block.key().path() == "big_dripleaf_stem"
        || below_block.has_tag(&SUPPORTS_BIG_DRIPLEAF)
}

fn small_dripleaf_can_survive(
    region: &WorldGenRegion,
    pos: BlockPosition,
    state: BlockState,
    block: Block,
) -> bool {
    if is_upper_half(state) {
        let below = region.get_block(pos.below());
        return below.as_block() == block && !is_upper_half(below);
    }
    let below_pos = pos.below();
    let below = region.get_block(below_pos);
    if below.as_block().has_tag(&SUPPORTS_SMALL_DRIPLEAF) {
        return true;
    }
    let above_fluid = region.get_block(pos).get_fluid_state();
    is_water_source(above_fluid) && below.as_block().has_tag(&SUPPORTS_VEGETATION)
}

fn is_water(state: BlockState) -> bool {
    state.get_fluid_state().fluid().has_tag(&WATER)
}

fn is_water_source(fluid: FluidState) -> bool {
    fluid.is_source() && fluid.fluid().has_tag(&WATER)
}

fn vegetation_can_survive(region: &WorldGenRegion, pos: BlockPosition) -> bool {
    block_at(region, pos.below()).has_tag(&SUPPORTS_VEGETATION)
}

fn mangrove_propagule_can_survive(
    region: &WorldGenRegion,
    pos: BlockPosition,
    state: BlockState,
) -> bool {
    if is_hanging(state) {
        return block_at(region, pos.above()).has_tag(&SUPPORTS_HANGING_MANGROVE_PROPAGULE);
    }
    block_at(region, pos.below()).has_tag(&SUPPORTS_MANGROVE_PROPAGULE)
}

fn is_hanging(state: BlockState) -> bool {
    static HANGING: BoolProperty = BoolProperty::new("hanging");
    state.get(HANGING).unwrap_or(false)
}

fn sugar_cane_can_survive(region: &WorldGenRegion, pos: BlockPosition, block: Block) -> bool {
    let below = pos.below();
    let below_block = block_at(region, below);
    if below_block == block {
        return true;
    }
    if !below_block.has_tag(&SUPPORTS_SUGAR_CANE) {
        return false;
    }
    HORIZONTAL_OFFSETS.iter().any(|&(dx, dz)| {
        let neighbor = BlockPosition::new(below.x() + dx, below.y(), below.z() + dz);
        let state = region.get_block(neighbor);
        let fluid = state.get_fluid_state();
        fluid.fluid().has_tag(&SUPPORTS_SUGAR_CANE_ADJACENTLY)
            || state.as_block().has_tag(&SUPPORTS_SUGAR_CANE_ADJACENTLY)
    })
}

fn cactus_can_survive(region: &WorldGenRegion, pos: BlockPosition, block: Block) -> bool {
    for &(dx, dz) in &HORIZONTAL_OFFSETS {
        let neighbor = BlockPosition::new(pos.x() + dx, pos.y(), pos.z() + dz);
        let state = region.get_block(neighbor);
        if state.is_solid() || state.get_fluid_state().fluid().has_tag(&LAVA) {
            return false;
        }
    }

    let below_block = block_at(region, pos.below());
    (below_block == block || below_block.has_tag(&SUPPORTS_CACTUS))
        && !region.get_block(pos.above()).liquid()
}

fn seagrass_may_place_on(region: &WorldGenRegion, below: BlockPosition) -> bool {
    let state = region.get_block(below);
    state.is_side_solid_at(below, Direction::Up)
        && !state.as_block().has_tag(&CANNOT_SUPPORT_SEAGRASS)
}

fn tall_seagrass_can_survive(
    region: &WorldGenRegion,
    pos: BlockPosition,
    state: BlockState,
    block: Block,
) -> bool {
    if is_upper_half(state) {
        let below = region.get_block(pos.below());
        return below.is(block) && !is_upper_half(below);
    }
    let fluid = region.get_block(pos).get_fluid_state();
    seagrass_may_place_on(region, pos.below()) && is_water_source(fluid)
}

fn sea_pickle_can_survive(region: &WorldGenRegion, pos: BlockPosition) -> bool {
    let below_pos = pos.below();
    let below = region.get_block(below_pos);
    below
        .get_collision_shape_at(below_pos)
        .iter()
        .any(|aabb| !aabb.is_empty() && aabb.min_y() <= TOP_FACE && aabb.max_y() >= TOP_FACE)
        || below.is_side_solid_at(below_pos, Direction::Up)
}

const TOP_FACE: f64 = 0.999_999_9;

fn coral_wall_fan_can_survive(
    region: &WorldGenRegion,
    pos: BlockPosition,
    state: BlockState,
) -> bool {
    let facing = state.try_get(Properties::HORIZONTAL_FACING);
    let relative_pos = pos.relative(facing.opposite());
    region
        .get_block(relative_pos)
        .is_side_solid_at(relative_pos, facing)
}

fn is_valid_fire_location(region: &WorldGenRegion, pos: BlockPosition) -> bool {
    Direction::ALL
        .into_iter()
        .any(|direction| can_burn(region.get_block(pos.relative(direction))))
}

fn can_burn(state: BlockState) -> bool {
    if state.get(Properties::WATERLOGGED) == Some(true) {
        return false;
    }
    FLAMMABLE.contains(&state.as_block())
}

fn block_at(region: &WorldGenRegion, pos: BlockPosition) -> Block {
    region.get_block(pos).as_block()
}

const FLAMMABLE: &[Block] = &[
    Block::OAK_PLANKS,
    Block::SPRUCE_PLANKS,
    Block::BIRCH_PLANKS,
    Block::JUNGLE_PLANKS,
    Block::ACACIA_PLANKS,
    Block::CHERRY_PLANKS,
    Block::DARK_OAK_PLANKS,
    Block::PALE_OAK_PLANKS,
    Block::POPLAR_PLANKS,
    Block::MANGROVE_PLANKS,
    Block::BAMBOO_PLANKS,
    Block::BAMBOO_MOSAIC,
    Block::OAK_SLAB,
    Block::SPRUCE_SLAB,
    Block::BIRCH_SLAB,
    Block::JUNGLE_SLAB,
    Block::ACACIA_SLAB,
    Block::CHERRY_SLAB,
    Block::DARK_OAK_SLAB,
    Block::PALE_OAK_SLAB,
    Block::POPLAR_SLAB,
    Block::MANGROVE_SLAB,
    Block::BAMBOO_SLAB,
    Block::BAMBOO_MOSAIC_SLAB,
    Block::OAK_FENCE_GATE,
    Block::SPRUCE_FENCE_GATE,
    Block::BIRCH_FENCE_GATE,
    Block::JUNGLE_FENCE_GATE,
    Block::ACACIA_FENCE_GATE,
    Block::CHERRY_FENCE_GATE,
    Block::DARK_OAK_FENCE_GATE,
    Block::PALE_OAK_FENCE_GATE,
    Block::POPLAR_FENCE_GATE,
    Block::MANGROVE_FENCE_GATE,
    Block::BAMBOO_FENCE_GATE,
    Block::OAK_FENCE,
    Block::SPRUCE_FENCE,
    Block::BIRCH_FENCE,
    Block::JUNGLE_FENCE,
    Block::ACACIA_FENCE,
    Block::CHERRY_FENCE,
    Block::DARK_OAK_FENCE,
    Block::PALE_OAK_FENCE,
    Block::POPLAR_FENCE,
    Block::MANGROVE_FENCE,
    Block::BAMBOO_FENCE,
    Block::OAK_STAIRS,
    Block::BIRCH_STAIRS,
    Block::SPRUCE_STAIRS,
    Block::JUNGLE_STAIRS,
    Block::ACACIA_STAIRS,
    Block::CHERRY_STAIRS,
    Block::DARK_OAK_STAIRS,
    Block::PALE_OAK_STAIRS,
    Block::POPLAR_STAIRS,
    Block::MANGROVE_STAIRS,
    Block::BAMBOO_STAIRS,
    Block::BAMBOO_MOSAIC_STAIRS,
    Block::OAK_LOG,
    Block::SPRUCE_LOG,
    Block::BIRCH_LOG,
    Block::JUNGLE_LOG,
    Block::ACACIA_LOG,
    Block::CHERRY_LOG,
    Block::PALE_OAK_LOG,
    Block::POPLAR_LOG,
    Block::DARK_OAK_LOG,
    Block::MANGROVE_LOG,
    Block::BAMBOO_BLOCK,
    Block::STRIPPED_OAK_LOG,
    Block::STRIPPED_SPRUCE_LOG,
    Block::STRIPPED_BIRCH_LOG,
    Block::STRIPPED_JUNGLE_LOG,
    Block::STRIPPED_ACACIA_LOG,
    Block::STRIPPED_CHERRY_LOG,
    Block::STRIPPED_DARK_OAK_LOG,
    Block::STRIPPED_PALE_OAK_LOG,
    Block::STRIPPED_POPLAR_LOG,
    Block::STRIPPED_MANGROVE_LOG,
    Block::STRIPPED_BAMBOO_BLOCK,
    Block::STRIPPED_OAK_WOOD,
    Block::STRIPPED_SPRUCE_WOOD,
    Block::STRIPPED_BIRCH_WOOD,
    Block::STRIPPED_JUNGLE_WOOD,
    Block::STRIPPED_ACACIA_WOOD,
    Block::STRIPPED_CHERRY_WOOD,
    Block::STRIPPED_DARK_OAK_WOOD,
    Block::STRIPPED_PALE_OAK_WOOD,
    Block::STRIPPED_POPLAR_WOOD,
    Block::STRIPPED_MANGROVE_WOOD,
    Block::OAK_WOOD,
    Block::SPRUCE_WOOD,
    Block::BIRCH_WOOD,
    Block::JUNGLE_WOOD,
    Block::ACACIA_WOOD,
    Block::CHERRY_WOOD,
    Block::PALE_OAK_WOOD,
    Block::POPLAR_WOOD,
    Block::DARK_OAK_WOOD,
    Block::MANGROVE_WOOD,
    Block::MANGROVE_ROOTS,
    Block::OAK_LEAVES,
    Block::SPRUCE_LEAVES,
    Block::BIRCH_LEAVES,
    Block::JUNGLE_LEAVES,
    Block::ACACIA_LEAVES,
    Block::CHERRY_LEAVES,
    Block::DARK_OAK_LEAVES,
    Block::PALE_OAK_LEAVES,
    Block::MANGROVE_LEAVES,
    Block::RED_POPLAR_LEAVES,
    Block::ORANGE_POPLAR_LEAVES,
    Block::YELLOW_POPLAR_LEAVES,
    Block::BOOKSHELF,
    Block::TNT,
    Block::SHORT_GRASS,
    Block::FERN,
    Block::DEAD_BUSH,
    Block::SHORT_DRY_GRASS,
    Block::TALL_DRY_GRASS,
    Block::SUNFLOWER,
    Block::LILAC,
    Block::ROSE_BUSH,
    Block::PEONY,
    Block::RED_SHRUB,
    Block::TALL_GRASS,
    Block::LARGE_FERN,
    Block::DANDELION,
    Block::GOLDEN_DANDELION,
    Block::POPPY,
    Block::OPEN_EYEBLOSSOM,
    Block::CLOSED_EYEBLOSSOM,
    Block::BLUE_ORCHID,
    Block::ALLIUM,
    Block::AZURE_BLUET,
    Block::RED_TULIP,
    Block::ORANGE_TULIP,
    Block::WHITE_TULIP,
    Block::PINK_TULIP,
    Block::OXEYE_DAISY,
    Block::CORNFLOWER,
    Block::LILY_OF_THE_VALLEY,
    Block::TORCHFLOWER,
    Block::PITCHER_PLANT,
    Block::WITHER_ROSE,
    Block::PINK_PETALS,
    Block::WILDFLOWERS,
    Block::LEAF_LITTER,
    Block::CACTUS_FLOWER,
    Block::VINE,
    Block::COAL_BLOCK,
    Block::HAY_BLOCK,
    Block::TARGET,
    Block::PALE_MOSS_BLOCK,
    Block::PALE_MOSS_CARPET,
    Block::PALE_HANGING_MOSS,
    Block::DRIED_KELP_BLOCK,
    Block::BAMBOO,
    Block::SCAFFOLDING,
    Block::LECTERN,
    Block::COMPOSTER,
    Block::SWEET_BERRY_BUSH,
    Block::BEEHIVE,
    Block::BEE_NEST,
    Block::AZALEA_LEAVES,
    Block::FLOWERING_AZALEA_LEAVES,
    Block::CAVE_VINES,
    Block::CAVE_VINES_PLANT,
    Block::SPORE_BLOSSOM,
    Block::AZALEA,
    Block::FLOWERING_AZALEA,
    Block::BIG_DRIPLEAF,
    Block::BIG_DRIPLEAF_STEM,
    Block::SMALL_DRIPLEAF,
    Block::HANGING_ROOTS,
    Block::GLOW_LICHEN,
    Block::FIREFLY_BUSH,
    Block::BUSH,
    Block::ACACIA_SHELF,
    Block::BAMBOO_SHELF,
    Block::BIRCH_SHELF,
    Block::CHERRY_SHELF,
    Block::DARK_OAK_SHELF,
    Block::JUNGLE_SHELF,
    Block::MANGROVE_SHELF,
    Block::POPLAR_SHELF,
    Block::OAK_SHELF,
    Block::PALE_OAK_SHELF,
    Block::SPRUCE_SHELF,
    Block::STRAW_BED,
];
