use crate::registry::REGISTRY;
use crate::registry::blocks::BlockRef;
use crate::registry::blocks::properties::BoolProperty;
use crate::util::ident::{BlockState, Identifier};
use crate::util::pos::BlockPos;
use crate::worldgen::region::WorldGenRegion;

const SUPPORTS_VEGETATION: Identifier = Identifier::const_vanilla("supports_vegetation");
const SUPPORTS_SUGAR_CANE: Identifier = Identifier::const_vanilla("supports_sugar_cane");
const SUPPORTS_SUGAR_CANE_ADJACENTLY: Identifier =
    Identifier::const_vanilla("supports_sugar_cane_adjacently");
const SUPPORTS_CACTUS: Identifier = Identifier::const_vanilla("supports_cactus");
const SUPPORTS_MANGROVE_PROPAGULE: Identifier =
    Identifier::const_vanilla("supports_mangrove_propagule");
const SUPPORTS_HANGING_MANGROVE_PROPAGULE: Identifier =
    Identifier::const_vanilla("supports_hanging_mangrove_propagule");
const LAVA: Identifier = Identifier::const_vanilla("lava");

const HORIZONTAL_OFFSETS: [(i32, i32); 4] = [(0, -1), (0, 1), (-1, 0), (1, 0)];

pub fn can_survive(region: &WorldGenRegion, pos: BlockPos, state: BlockState) -> bool {
    let block = state.get_block();
    match block.key.path.as_ref() {
        "cactus" => cactus_can_survive(region, pos, block),
        "sugar_cane" => sugar_cane_can_survive(region, pos, block),
        "mangrove_propagule" => mangrove_propagule_can_survive(region, pos, state),
        path if path.ends_with("_sapling") || path == "firefly_bush" || path == "bush" => {
            vegetation_can_survive(region, pos)
        }
        _ => true,
    }
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
        let state = region.get_block_state(neighbor);
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
        let state = region.get_block_state(neighbor);
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
        && !region
            .get_block_state(pos.above())
            .get_block()
            .config
            .liquid
}

fn block_at(region: &WorldGenRegion, pos: BlockPos) -> BlockRef {
    region.get_block_state(pos).get_block()
}
