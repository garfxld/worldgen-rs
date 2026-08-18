use crate::chunk::heightmap::HeightmapType;
use crate::registry::REGISTRY;
use crate::registry::blocks::BlockRef;
use crate::registry::blocks::properties::{BoolProperty, DoubleBlockHalf, Properties};
use crate::registry::blocks::shapes::{self, SupportType};
use crate::registry::blocks::vanilla_blocks;
use crate::registry::fluid::FluidState;
use crate::util::direction::Direction;
use crate::util::ident::{BlockState, Identifier};
use crate::util::pos::BlockPos;
use crate::worldgen::region::WorldGenRegion;

const SUPPORTS_VEGETATION: Identifier = Identifier::const_vanilla("supports_vegetation");
const SUPPORTS_DRY_VEGETATION: Identifier = Identifier::const_vanilla("supports_dry_vegetation");
const SUPPORTS_WITHER_ROSE: Identifier = Identifier::const_vanilla("supports_wither_rose");
const SUPPORTS_CRIMSON_ROOTS: Identifier = Identifier::const_vanilla("supports_crimson_roots");
const SUPPORTS_WARPED_ROOTS: Identifier = Identifier::const_vanilla("supports_warped_roots");
const SUPPORTS_CRIMSON_FUNGUS: Identifier = Identifier::const_vanilla("supports_crimson_fungus");
const SUPPORTS_WARPED_FUNGUS: Identifier = Identifier::const_vanilla("supports_warped_fungus");
const SUPPORTS_NETHER_SPROUTS: Identifier = Identifier::const_vanilla("supports_nether_sprouts");
const SUPPORTS_AZALEA: Identifier = Identifier::const_vanilla("supports_azalea");
const SUPPORTS_LILY_PAD: Identifier = Identifier::const_vanilla("supports_lily_pad");
const SUPPORTS_BIG_DRIPLEAF: Identifier = Identifier::const_vanilla("supports_big_dripleaf");
const SUPPORTS_SMALL_DRIPLEAF: Identifier = Identifier::const_vanilla("supports_small_dripleaf");
const SUPPORT_OVERRIDE_CACTUS_FLOWER: Identifier =
    Identifier::const_vanilla("support_override_cactus_flower");
const OVERRIDES_MUSHROOM_LIGHT_REQUIREMENT: Identifier =
    Identifier::const_vanilla("overrides_mushroom_light_requirement");
const WATER: Identifier = Identifier::const_vanilla("water");
const SUPPORTS_SUGAR_CANE: Identifier = Identifier::const_vanilla("supports_sugar_cane");
const SUPPORTS_SUGAR_CANE_ADJACENTLY: Identifier =
    Identifier::const_vanilla("supports_sugar_cane_adjacently");
const SUPPORTS_CACTUS: Identifier = Identifier::const_vanilla("supports_cactus");
const SUPPORTS_MANGROVE_PROPAGULE: Identifier =
    Identifier::const_vanilla("supports_mangrove_propagule");
const SUPPORTS_HANGING_MANGROVE_PROPAGULE: Identifier =
    Identifier::const_vanilla("supports_hanging_mangrove_propagule");
const LAVA: Identifier = Identifier::const_vanilla("lava");
const CANNOT_SUPPORT_SNOW_LAYER: Identifier =
    Identifier::const_vanilla("cannot_support_snow_layer");
const SUPPORT_OVERRIDE_SNOW_LAYER: Identifier =
    Identifier::const_vanilla("support_override_snow_layer");

const HORIZONTAL_OFFSETS: [(i32, i32); 4] = [(0, -1), (0, 1), (-1, 0), (1, 0)];

pub fn can_survive(region: &WorldGenRegion, pos: BlockPos, state: BlockState) -> bool {
    let block = state.get_block();
    match block.key.path.as_ref() {
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
            region
                .get_block(above)
                .is_face_sturdy_at(above, Direction::Down)
        }
        "big_dripleaf" => big_dripleaf_can_survive(region, pos, block),
        "small_dripleaf" => small_dripleaf_can_survive(region, pos, state, block),
        "snow" => snow_can_survive(region, pos),
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

fn supported_by_tag(region: &WorldGenRegion, pos: BlockPos, tag: &Identifier) -> bool {
    block_at(region, pos.below()).has_tag(tag)
}

fn snow_can_survive(region: &WorldGenRegion, pos: BlockPos) -> bool {
    let below_pos = pos.below();
    let below = region.get_block(below_pos);
    let below_block = below.get_block();
    if below_block.has_tag(&CANNOT_SUPPORT_SNOW_LAYER) {
        return false;
    }
    if below_block.has_tag(&SUPPORT_OVERRIDE_SNOW_LAYER) {
        return true;
    }

    shapes::is_offset_face_full(below.get_collision_shape_at(below_pos), Direction::Up)
        || (below_block == &vanilla_blocks::SNOW && below.get_value(&Properties::LAYERS) == 8)
}

fn sturdy_below(region: &WorldGenRegion, pos: BlockPos) -> bool {
    let below = pos.below();
    region
        .get_block(below)
        .is_face_sturdy_at(below, Direction::Up)
}

fn double_plant_can_survive(
    region: &WorldGenRegion,
    pos: BlockPos,
    state: BlockState,
    block: BlockRef,
) -> bool {
    if !is_upper_half(state) {
        return vegetation_can_survive(region, pos);
    }
    let below = region.get_block(pos.below());
    below.get_block() == block && !is_upper_half(below)
}

fn is_upper_half(state: BlockState) -> bool {
    state
        .try_get_value(&Properties::DOUBLE_BLOCK_HALF)
        .is_some_and(|half| half == DoubleBlockHalf::Upper)
}

fn mushroom_can_survive(region: &WorldGenRegion, pos: BlockPos) -> bool {
    let below_pos = pos.below();
    let below = region.get_block(below_pos);
    if below
        .get_block()
        .has_tag(&OVERRIDES_MUSHROOM_LIGHT_REQUIREMENT)
    {
        return true;
    }
    if pos.y() >= region.get_height(HeightmapType::WorldSurfaceWg, pos.x(), pos.z()) {
        return false;
    }
    below.is_solid_render()
}

fn lily_pad_can_survive(region: &WorldGenRegion, pos: BlockPos) -> bool {
    if !region.get_block(pos.above()).get_fluid_state().is_empty() {
        return false;
    }
    let below_pos = pos.below();
    let below = region.get_block(below_pos);
    REGISTRY
        .fluids
        .is_in_tag(&below.get_fluid_state().fluid_id.key, &SUPPORTS_LILY_PAD)
        || below.get_block().has_tag(&SUPPORTS_LILY_PAD)
}

fn mossy_carpet_can_survive(
    region: &WorldGenRegion,
    pos: BlockPos,
    state: BlockState,
    block: BlockRef,
) -> bool {
    let below = region.get_block(pos.below());
    if is_carpet_base(state) {
        return !below.is_air();
    }
    below.get_block() == block && is_carpet_base(below)
}

pub fn is_carpet_base(state: BlockState) -> bool {
    state.try_get_value(&Properties::BOTTOM).unwrap_or(false)
}

fn cactus_flower_can_survive(region: &WorldGenRegion, pos: BlockPos) -> bool {
    let below_pos = pos.below();
    let below = region.get_block(below_pos);
    below.get_block().has_tag(&SUPPORT_OVERRIDE_CACTUS_FLOWER)
        || below.is_face_sturdy_for_at(below_pos, Direction::Up, SupportType::Center)
}

fn spore_blossom_can_survive(region: &WorldGenRegion, pos: BlockPos) -> bool {
    let above_pos = pos.above();
    let above = region.get_block(above_pos);
    above.is_face_sturdy_for_at(above_pos, Direction::Down, SupportType::Center)
        && !is_water(region.get_block(pos))
}

fn big_dripleaf_can_survive(region: &WorldGenRegion, pos: BlockPos, block: BlockRef) -> bool {
    let below = region.get_block(pos.below());
    let below_block = below.get_block();
    below_block == block
        || below_block.key.path.as_ref() == "big_dripleaf_stem"
        || below_block.has_tag(&SUPPORTS_BIG_DRIPLEAF)
}

fn small_dripleaf_can_survive(
    region: &WorldGenRegion,
    pos: BlockPos,
    state: BlockState,
    block: BlockRef,
) -> bool {
    if is_upper_half(state) {
        let below = region.get_block(pos.below());
        return below.get_block() == block && !is_upper_half(below);
    }
    let below_pos = pos.below();
    let below = region.get_block(below_pos);
    if below.get_block().has_tag(&SUPPORTS_SMALL_DRIPLEAF) {
        return true;
    }
    let above_fluid = region.get_block(pos).get_fluid_state();
    is_water_source(above_fluid) && below.get_block().has_tag(&SUPPORTS_VEGETATION)
}

fn is_water(state: BlockState) -> bool {
    REGISTRY
        .fluids
        .is_in_tag(&state.get_fluid_state().fluid_id.key, &WATER)
}

fn is_water_source(fluid: FluidState) -> bool {
    fluid.fluid_id.is_source && REGISTRY.fluids.is_in_tag(&fluid.fluid_id.key, &WATER)
}

fn vegetation_can_survive(region: &WorldGenRegion, pos: BlockPos) -> bool {
    block_at(region, pos.below()).has_tag(&SUPPORTS_VEGETATION)
}

fn mangrove_propagule_can_survive(
    region: &WorldGenRegion,
    pos: BlockPos,
    state: BlockState,
) -> bool {
    if is_hanging(state) {
        return block_at(region, pos.above()).has_tag(&SUPPORTS_HANGING_MANGROVE_PROPAGULE);
    }
    block_at(region, pos.below()).has_tag(&SUPPORTS_MANGROVE_PROPAGULE)
}

fn is_hanging(state: BlockState) -> bool {
    static HANGING: BoolProperty = BoolProperty::new("hanging");
    state.try_get_value(&HANGING).unwrap_or(false)
}

fn sugar_cane_can_survive(region: &WorldGenRegion, pos: BlockPos, block: BlockRef) -> bool {
    let below = pos.below();
    let below_block = block_at(region, below);
    if below_block == block {
        return true;
    }
    if !below_block.has_tag(&SUPPORTS_SUGAR_CANE) {
        return false;
    }
    HORIZONTAL_OFFSETS.iter().any(|&(dx, dz)| {
        let neighbor = BlockPos::new(below.0.x + dx, below.0.y, below.0.z + dz);
        let state = region.get_block(neighbor);
        let fluid = state.get_fluid_state();
        REGISTRY
            .fluids
            .is_in_tag(&fluid.fluid_id.key, &SUPPORTS_SUGAR_CANE_ADJACENTLY)
            || state.get_block().has_tag(&SUPPORTS_SUGAR_CANE_ADJACENTLY)
    })
}

fn cactus_can_survive(region: &WorldGenRegion, pos: BlockPos, block: BlockRef) -> bool {
    for &(dx, dz) in &HORIZONTAL_OFFSETS {
        let neighbor = BlockPos::new(pos.0.x + dx, pos.0.y, pos.0.z + dz);
        let state = region.get_block(neighbor);
        if state.is_solid()
            || REGISTRY
                .fluids
                .is_in_tag(&state.get_fluid_state().fluid_id.key, &LAVA)
        {
            return false;
        }
    }

    let below_block = block_at(region, pos.below());
    (below_block == block || below_block.has_tag(&SUPPORTS_CACTUS))
        && !region.get_block(pos.above()).get_block().config.liquid
}

fn block_at(region: &WorldGenRegion, pos: BlockPos) -> BlockRef {
    region.get_block(pos).get_block()
}
