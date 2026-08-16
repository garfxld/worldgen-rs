use crate::registry::REGISTRY;
use crate::registry::blocks::properties::Properties;
use crate::registry::blocks::vanilla_blocks;
use crate::registry::fluid::is_water_fluid;
use crate::util::direction::Direction;
use crate::util::ident::{BlockState, Identifier};
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::util::shuffle;
use crate::worldgen::feature::multiface::{self, Spreader};
use crate::worldgen::region::WorldGenRegion;

const SCULK_REPLACEABLE: Identifier = Identifier::const_vanilla("sculk_replaceable");
const SCULK_REPLACEABLE_WORLD_GEN: Identifier =
    Identifier::const_vanilla("sculk_replaceable_world_gen");

const NON_CORNER_NEIGHBOURS: [(i32, i32, i32); 18] = [
    (0, -1, -1),
    (-1, 0, -1),
    (0, 0, -1),
    (1, 0, -1),
    (0, 1, -1),
    (-1, -1, 0),
    (0, -1, 0),
    (1, -1, 0),
    (-1, 0, 0),
    (1, 0, 0),
    (-1, 1, 0),
    (0, 1, 0),
    (1, 1, 0),
    (0, -1, 1),
    (-1, 0, 1),
    (0, 0, 1),
    (1, 0, 1),
    (0, 1, 1),
];

struct SpreaderParams {
    growth_spawn_cost: i32,
    no_growth_radius: i32,
    charge_decay_rate: i32,
    additional_decay_rate: i32,
}

const WORLD_GEN_PARAMS: SpreaderParams = SpreaderParams {
    growth_spawn_cost: 50,
    no_growth_radius: 1,
    charge_decay_rate: 5,
    additional_decay_rate: 10,
};

struct ChargeCursor {
    pos: BlockPos,
    charge: i32,
    update_delay: i32,
    decay_delay: i32,
    facings: Option<Vec<Direction>>,
}

impl ChargeCursor {
    const fn new(pos: BlockPos, charge: i32) -> Self {
        Self {
            pos,
            charge,
            update_delay: 0,
            decay_delay: 1,
            facings: None,
        }
    }

    const fn is_pos_unreasonable(&self, origin_pos: BlockPos) -> bool {
        let dist = (self.pos.x() - origin_pos.x())
            .abs()
            .max((self.pos.y() - origin_pos.y()).abs())
            .max((self.pos.z() - origin_pos.z()).abs());
        dist > 1024
    }

    fn update(
        &mut self,
        region: &WorldGenRegion<'_>,
        origin_pos: BlockPos,
        random: &mut RandomSource,
        params: &SpreaderParams,
        spread_veins: bool,
    ) {
        if self.charge <= 0 {
            return;
        }
        if self.update_delay > 0 {
            self.update_delay -= 1;
            return;
        }

        let mut current_state = region.get_block(self.pos);
        let mut behaviour = get_block_behaviour(current_state);

        if spread_veins
            && attempt_spread_vein(
                behaviour,
                region,
                self.pos,
                current_state,
                self.facings.as_deref(),
            )
            && can_change_block_state_on_spread(behaviour)
        {
            current_state = region.get_block(self.pos);
            behaviour = get_block_behaviour(current_state);
        }

        self.charge = attempt_use_charge(
            behaviour,
            self.charge,
            self.decay_delay,
            self.pos,
            region,
            origin_pos,
            random,
            params,
        );

        if self.charge <= 0 {
            on_discharged(behaviour, region, current_state, self.pos);
            return;
        }

        if let Some(transfer_pos) = get_valid_movement_pos(region, self.pos, random) {
            on_discharged(behaviour, region, current_state, self.pos);
            self.pos = transfer_pos;
            let dx = f64::from(self.pos.x() - origin_pos.x());
            let dz = f64::from(self.pos.z() - origin_pos.z());
            if dx * dx + dz * dz >= 15.0 * 15.0 {
                self.charge = 0;
                return;
            }
            current_state = region.get_block(self.pos);
        }

        if is_sculk_behaviour_block(current_state) {
            self.facings = Some(multiface::available_faces(current_state));
        }
        self.decay_delay = update_decay_delay(behaviour, self.decay_delay);
        self.update_delay = 1;
    }
}

pub struct SculkSpreader {
    cursors: Vec<ChargeCursor>,
}

impl SculkSpreader {
    pub fn create_world_gen_spreader() -> Self {
        Self {
            cursors: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.cursors.clear();
    }

    pub fn add_cursors(&mut self, start_pos: BlockPos, mut charge: i32) {
        while charge > 0 {
            let current_charge = charge.min(1000);
            if self.cursors.len() < 32 {
                self.cursors
                    .push(ChargeCursor::new(start_pos, current_charge));
            }
            charge -= current_charge;
        }
    }

    pub fn update_cursors(
        &mut self,
        region: &WorldGenRegion<'_>,
        origin_pos: BlockPos,
        random: &mut RandomSource,
        spread_veins: bool,
    ) {
        if self.cursors.is_empty() {
            return;
        }

        let cursors = std::mem::take(&mut self.cursors);
        let mut processed = Vec::with_capacity(cursors.len());
        for mut cursor in cursors {
            if cursor.is_pos_unreasonable(origin_pos) {
                continue;
            }
            cursor.update(region, origin_pos, random, &WORLD_GEN_PARAMS, spread_veins);
            if cursor.charge > 0 {
                processed.push(cursor);
            }
        }
        self.cursors = processed;
    }
}

#[derive(Clone, Copy)]
enum Behaviour {
    Default,
    SculkBlock,
    SculkVein,
}

fn get_block_behaviour(state: BlockState) -> Behaviour {
    let block = state.get_block();
    if block == &vanilla_blocks::SCULK {
        Behaviour::SculkBlock
    } else if block == &vanilla_blocks::SCULK_VEIN {
        Behaviour::SculkVein
    } else {
        Behaviour::Default
    }
}

fn is_sculk_behaviour_block(state: BlockState) -> bool {
    let block = state.get_block();
    block == &vanilla_blocks::SCULK || block == &vanilla_blocks::SCULK_VEIN
}

const fn can_change_block_state_on_spread(behaviour: Behaviour) -> bool {
    !matches!(behaviour, Behaviour::SculkBlock)
}

const fn update_decay_delay(behaviour: Behaviour, age: i32) -> i32 {
    match behaviour {
        Behaviour::Default => (age - 1).max(0),
        _ => 1,
    }
}

fn on_discharged(
    behaviour: Behaviour,
    region: &WorldGenRegion<'_>,
    state: BlockState,
    pos: BlockPos,
) {
    if matches!(behaviour, Behaviour::SculkVein) {
        sculk_vein_on_discharged(region, state, pos);
    }
}

fn sculk_vein_on_discharged(region: &WorldGenRegion<'_>, mut state: BlockState, pos: BlockPos) {
    if state.get_block() != &vanilla_blocks::SCULK_VEIN {
        return;
    }

    for direction in multiface::DIRECTIONS {
        if multiface::has_face(state, direction)
            && region.get_block(direction.relative(pos)).get_block() == &vanilla_blocks::SCULK
        {
            state = multiface::set_face(state, direction, false);
        }
    }

    if !multiface::has_any_face(state) {
        state = if state.get_fluid_state().is_empty() {
            vanilla_blocks::AIR.default_state()
        } else {
            vanilla_blocks::WATER.default_state()
        };
    }

    region.set_block(pos, state);
}

fn attempt_spread_vein(
    behaviour: Behaviour,
    region: &WorldGenRegion<'_>,
    pos: BlockPos,
    state: BlockState,
    facings: Option<&[Direction]>,
) -> bool {
    match behaviour {
        Behaviour::Default => match facings {
            None => Spreader::SculkVeinSameSpace.spread_all(state, region, pos) > 0,
            Some([]) => Spreader::SculkVein.spread_all(state, region, pos) > 0,
            Some(faces) => {
                let fluid = state.get_fluid_state();
                if !state.is_air() && !is_water_fluid(fluid.fluid_id) {
                    false
                } else {
                    sculk_vein_regrow(region, pos, state, faces)
                }
            }
        },
        Behaviour::SculkBlock | Behaviour::SculkVein => {
            Spreader::SculkVein.spread_all(state, region, pos) > 0
        }
    }
}

fn sculk_vein_regrow(
    region: &WorldGenRegion<'_>,
    pos: BlockPos,
    existing: BlockState,
    faces: &[Direction],
) -> bool {
    let mut has_at_least_one_face = false;
    let mut new_state = vanilla_blocks::SCULK_VEIN.default_state();

    for &face in faces {
        if multiface::can_attach_to(region, pos, face) {
            new_state = multiface::set_face(new_state, face, true);
            has_at_least_one_face = true;
        }
    }

    if !has_at_least_one_face {
        return false;
    }

    if !existing.get_fluid_state().is_empty() {
        new_state = new_state.set_value(&Properties::WATERLOGGED, true);
    }

    region.set_block(pos, new_state);
    true
}

#[expect(clippy::too_many_arguments, reason = "vanilla parameter list")]
fn attempt_use_charge(
    behaviour: Behaviour,
    charge: i32,
    decay_delay: i32,
    pos: BlockPos,
    region: &WorldGenRegion<'_>,
    origin_pos: BlockPos,
    random: &mut RandomSource,
    params: &SpreaderParams,
) -> i32 {
    match behaviour {
        Behaviour::Default => {
            if decay_delay > 0 {
                charge
            } else {
                0
            }
        }
        Behaviour::SculkBlock => {
            sculk_block_attempt_use_charge(charge, pos, region, origin_pos, random, params)
        }
        Behaviour::SculkVein => sculk_vein_attempt_use_charge(charge, pos, region, random, params),
    }
}

fn sculk_block_attempt_use_charge(
    charge: i32,
    pos: BlockPos,
    region: &WorldGenRegion<'_>,
    origin_pos: BlockPos,
    random: &mut RandomSource,
    params: &SpreaderParams,
) -> i32 {
    if charge == 0 || random.next_i32_bounded(params.charge_decay_rate) != 0 {
        return charge;
    }

    let dx = f64::from(pos.x() - origin_pos.x());
    let dy = f64::from(pos.y() - origin_pos.y());
    let dz = f64::from(pos.z() - origin_pos.z());
    let is_close_to_catalyst =
        dx * dx + dy * dy + dz * dz < f64::from(params.no_growth_radius * params.no_growth_radius);

    if !is_close_to_catalyst && can_place_growth(region, pos) {
        let xp_per_growth_spawn = params.growth_spawn_cost;
        if random.next_i32_bounded(xp_per_growth_spawn) < charge {
            let growth_placement = pos.above();
            let growth_state = random_growth_state(region, growth_placement, random);
            region.set_block(growth_placement, growth_state);
        }
        (charge - xp_per_growth_spawn).max(0)
    } else if random.next_i32_bounded(params.additional_decay_rate) != 0 {
        charge
    } else {
        let penalty = if is_close_to_catalyst {
            1
        } else {
            get_decay_penalty(params, pos, origin_pos, charge)
        };
        charge - penalty
    }
}

fn get_decay_penalty(
    params: &SpreaderParams,
    pos: BlockPos,
    origin_pos: BlockPos,
    charge: i32,
) -> i32 {
    let dx = f64::from(pos.x() - origin_pos.x());
    let dy = f64::from(pos.y() - origin_pos.y());
    let dz = f64::from(pos.z() - origin_pos.z());
    let dist = (dx * dx + dy * dy + dz * dz).sqrt() as f32;
    let no_growth_radius = params.no_growth_radius as f32;
    let outer_distance_sq = (dist - no_growth_radius).powi(2);
    let max_reach_sq = (24.0 - no_growth_radius).powi(2);
    let factor = (outer_distance_sq / max_reach_sq).min(1.0);
    1.max((charge as f32 * factor * 0.5) as i32)
}

fn can_place_growth(region: &WorldGenRegion<'_>, pos: BlockPos) -> bool {
    let above = region.get_block(pos.above());
    let above_fluid = above.get_fluid_state();
    if !(above.is_air()
        || (above.get_block() == &vanilla_blocks::WATER && above_fluid.is_source()))
    {
        return false;
    }

    let mut growth_count = 0;
    for dz in -4..=4 {
        for dy in 0..=2 {
            for dx in -4..=4 {
                let block = region.get_block(pos.offset(dx, dy, dz)).get_block();
                if block == &vanilla_blocks::SCULK_SENSOR
                    || block == &vanilla_blocks::SCULK_SHRIEKER
                {
                    growth_count += 1;
                    if growth_count > 2 {
                        return false;
                    }
                }
            }
        }
    }

    true
}

fn random_growth_state(
    region: &WorldGenRegion<'_>,
    pos: BlockPos,
    random: &mut RandomSource,
) -> BlockState {
    let state = if random.next_i32_bounded(11) == 0 {
        vanilla_blocks::SCULK_SHRIEKER
            .default_state()
            .set_value(&Properties::CAN_SUMMON, true)
    } else {
        vanilla_blocks::SCULK_SENSOR.default_state()
    };

    if state.try_get_value(&Properties::WATERLOGGED).is_some()
        && !region.get_block(pos).get_fluid_state().is_empty()
    {
        state.set_value(&Properties::WATERLOGGED, true)
    } else {
        state
    }
}

fn sculk_vein_attempt_use_charge(
    charge: i32,
    pos: BlockPos,
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    params: &SpreaderParams,
) -> i32 {
    if attempt_place_sculk(region, pos, random) {
        charge - 1
    } else if random.next_i32_bounded(params.charge_decay_rate) == 0 {
        (charge as f32 * 0.5).floor() as i32
    } else {
        charge
    }
}

fn attempt_place_sculk(
    region: &WorldGenRegion<'_>,
    pos: BlockPos,
    random: &mut RandomSource,
) -> bool {
    let state = region.get_block(pos);
    let mut directions = multiface::DIRECTIONS;
    shuffle(&mut directions, random);

    for support in directions {
        if !multiface::has_face(state, support) {
            continue;
        }

        let support_pos = support.relative(pos);
        let support_state = region.get_block(support_pos);
        if !REGISTRY.blocks.is_in_tag(
            &support_state.get_block().key,
            &SCULK_REPLACEABLE_WORLD_GEN,
        ) {
            continue;
        }

        let default_sculk = vanilla_blocks::SCULK.default_state();
        region.set_block(support_pos, default_sculk);
        Spreader::SculkVein.spread_all(default_sculk, region, support_pos);

        let skip = support.opposite();
        for vein_direction in multiface::DIRECTIONS {
            if vein_direction == skip {
                continue;
            }
            let vein_pos = vein_direction.relative(support_pos);
            let possible_vein_block = region.get_block(vein_pos);
            if possible_vein_block.get_block() == &vanilla_blocks::SCULK_VEIN {
                sculk_vein_on_discharged(region, possible_vein_block, vein_pos);
            }
        }

        return true;
    }

    false
}

fn get_valid_movement_pos(
    region: &WorldGenRegion<'_>,
    pos: BlockPos,
    random: &mut RandomSource,
) -> Option<BlockPos> {
    let mut offsets = NON_CORNER_NEIGHBOURS;
    shuffle(&mut offsets, random);

    let mut sculk_position = pos;
    for offset in offsets {
        let neighbour = pos.offset(offset.0, offset.1, offset.2);
        let transferee = region.get_block(neighbour);
        if is_sculk_behaviour_block(transferee) && is_movement_unobstructed(region, pos, neighbour) {
            sculk_position = neighbour;
            if has_substrate_access(region, transferee, neighbour) {
                break;
            }
        }
    }

    if sculk_position == pos {
        None
    } else {
        Some(sculk_position)
    }
}

fn is_movement_unobstructed(region: &WorldGenRegion<'_>, from: BlockPos, to: BlockPos) -> bool {
    let delta_x = to.x() - from.x();
    let delta_y = to.y() - from.y();
    let delta_z = to.z() - from.z();
    if delta_x.abs() + delta_y.abs() + delta_z.abs() == 1 {
        return true;
    }

    let dir_x = if delta_x < 0 {
        Direction::West
    } else {
        Direction::East
    };
    let dir_y = if delta_y < 0 {
        Direction::Down
    } else {
        Direction::Up
    };
    let dir_z = if delta_z < 0 {
        Direction::North
    } else {
        Direction::South
    };

    if delta_x == 0 {
        is_unobstructed(region, from, dir_y) || is_unobstructed(region, from, dir_z)
    } else if delta_y == 0 {
        is_unobstructed(region, from, dir_x) || is_unobstructed(region, from, dir_z)
    } else {
        is_unobstructed(region, from, dir_x) || is_unobstructed(region, from, dir_y)
    }
}

fn is_unobstructed(region: &WorldGenRegion<'_>, from: BlockPos, direction: Direction) -> bool {
    let test_pos = direction.relative(from);
    !region
        .get_block(test_pos)
        .is_face_sturdy_at(test_pos, direction.opposite())
}

fn has_substrate_access(region: &WorldGenRegion<'_>, state: BlockState, pos: BlockPos) -> bool {
    if state.get_block() != &vanilla_blocks::SCULK_VEIN {
        return false;
    }

    multiface::DIRECTIONS.iter().any(|&direction| {
        multiface::has_face(state, direction)
            && REGISTRY.blocks.is_in_tag(
                &region.get_block(direction.relative(pos)).get_block().key,
                &SCULK_REPLACEABLE,
            )
    })
}
