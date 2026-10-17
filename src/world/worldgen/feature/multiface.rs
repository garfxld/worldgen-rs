use shulkr::world::block::{Block, BlockState, BoolProperty, Properties};

use crate::registry::fluid::is_water_fluid;
use shulkr::util::Direction;
use shulkr::util::is_offset_face_full;

use crate::util::rng::RandomSource;
use crate::util::shuffle;

use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util::BlockPosition;

pub const DIRECTIONS: [Direction; 6] = Direction::ALL;

const fn face_property(direction: Direction) -> BoolProperty {
    match direction {
        Direction::Down => Properties::DOWN,
        Direction::Up => Properties::UP,
        Direction::North => Properties::NORTH,
        Direction::South => Properties::SOUTH,
        Direction::West => Properties::WEST,
        Direction::East => Properties::EAST,
    }
}

pub fn has_face(state: BlockState, direction: Direction) -> bool {
    state.get(face_property(direction)) == Some(true)
}

pub fn has_any_face(state: BlockState) -> bool {
    DIRECTIONS.iter().any(|&d| has_face(state, d))
}

pub fn available_faces(state: BlockState) -> Vec<Direction> {
    DIRECTIONS
        .into_iter()
        .filter(|&d| has_face(state, d))
        .collect()
}

pub fn set_face(state: BlockState, direction: Direction, value: bool) -> BlockState {
    state.with(face_property(direction), value)
}

pub fn can_attach_to(
    region: &WorldGenRegion<'_>,
    pos: BlockPosition,
    direction: Direction,
) -> bool {
    let neighbour_pos = pos.relative(direction);
    let neighbour = region.get_block(neighbour_pos);
    is_offset_face_full(
        neighbour.get_support_shape_at(neighbour_pos),
        direction.opposite(),
    ) || is_offset_face_full(
        neighbour.get_collision_shape_at(neighbour_pos),
        direction.opposite(),
    )
}

pub fn is_valid_state_for_placement(
    region: &WorldGenRegion<'_>,
    block: Block,
    old_state: BlockState,
    placement_pos: BlockPosition,
    placement_direction: Direction,
) -> bool {
    if old_state.as_block() == block && has_face(old_state, placement_direction) {
        return false;
    }
    can_attach_to(region, placement_pos, placement_direction)
}

pub fn state_for_placement(
    region: &WorldGenRegion<'_>,
    block: Block,
    old_state: BlockState,
    placement_pos: BlockPosition,
    placement_direction: Direction,
) -> Option<BlockState> {
    if !is_valid_state_for_placement(region, block, old_state, placement_pos, placement_direction) {
        return None;
    }

    let fluid = old_state.get_fluid_state();
    let new_state = if old_state.as_block() == block {
        old_state
    } else if fluid.is_source() && is_water_fluid(fluid.fluid()) {
        block.default_state().with(Properties::WATERLOGGED, true)
    } else {
        block.default_state()
    };

    Some(set_face(new_state, placement_direction, true))
}

#[derive(Clone, Copy)]
enum SpreadType {
    SamePosition,
    SamePlane,
    WrapAround,
}

const DEFAULT_SPREAD_ORDER: [SpreadType; 3] = [
    SpreadType::SamePosition,
    SpreadType::SamePlane,
    SpreadType::WrapAround,
];

impl SpreadType {
    fn spread_pos(
        self,
        pos: BlockPosition,
        spread_direction: Direction,
        from_face: Direction,
    ) -> (BlockPosition, Direction) {
        let relative = pos.relative(spread_direction);
        match self {
            Self::SamePosition => (pos, spread_direction),
            Self::SamePlane => (relative, from_face),
            Self::WrapAround => (relative.relative(from_face), spread_direction.opposite()),
        }
    }
}

#[derive(Clone, Copy)]
pub enum Spreader {
    Default(Block),
    SculkVein,
    SculkVeinSameSpace,
}

impl Spreader {
    const fn block(self) -> Block {
        match self {
            Self::Default(block) => block,
            Self::SculkVein | Self::SculkVeinSameSpace => Block::SCULK_VEIN,
        }
    }

    const fn spread_types(self) -> &'static [SpreadType] {
        match self {
            Self::SculkVeinSameSpace => &[SpreadType::SamePosition],
            Self::Default(_) | Self::SculkVein => &DEFAULT_SPREAD_ORDER,
        }
    }

    fn is_other_block_valid_as_source(self, state: BlockState) -> bool {
        match self {
            Self::Default(_) => false,
            Self::SculkVein | Self::SculkVeinSameSpace => !state.is(Block::SCULK_VEIN),
        }
    }

    fn can_spread_from(self, state: BlockState, face: Direction) -> bool {
        self.is_other_block_valid_as_source(state) || has_face(state, face)
    }

    fn state_can_be_replaced(
        self,
        region: &WorldGenRegion<'_>,
        source_pos: BlockPosition,
        placement_pos: BlockPosition,
        placement_direction: Direction,
        existing_state: BlockState,
    ) -> bool {
        if let Self::Default(block) = self {
            return existing_state.is_air()
                || existing_state.is(block)
                || (existing_state.is(Block::WATER)
                    && existing_state.get_fluid_state().is_source());
        }
        let against_state = region.get_block(placement_pos.relative(placement_direction));
        let against_block = against_state.as_block();

        if matches!(
            against_block,
            Block::SCULK | Block::SCULK_CATALYST | Block::MOVING_PISTON
        ) {
            return false;
        }

        let manhattan = (source_pos.x() - placement_pos.x()).abs()
            + (source_pos.y() - placement_pos.y()).abs()
            + (source_pos.z() - placement_pos.z()).abs();
        if manhattan == 2 {
            let neighbour_pos = source_pos.relative(placement_direction.opposite());
            if region
                .get_block(neighbour_pos)
                .is_side_solid_at(neighbour_pos, placement_direction)
            {
                return false;
            }
        }

        let fluid = existing_state.get_fluid_state();
        if !fluid.is_empty() && !is_water_fluid(fluid.fluid()) {
            return false;
        }

        let existing_block = existing_state.as_block();
        if matches!(existing_block, Block::FIRE | Block::SOUL_FIRE) {
            return false;
        }

        existing_state.is_replaceable()
            || existing_state.is_air()
            || existing_block == self.block()
            || (existing_block == Block::WATER && fluid.is_source())
    }

    fn can_spread_into(
        self,
        region: &WorldGenRegion<'_>,
        source_pos: BlockPosition,
        spread_pos: BlockPosition,
        spread_face: Direction,
    ) -> bool {
        let existing_state = region.get_block(spread_pos);
        self.state_can_be_replaced(region, source_pos, spread_pos, spread_face, existing_state)
            && is_valid_state_for_placement(
                region,
                self.block(),
                existing_state,
                spread_pos,
                spread_face,
            )
    }

    fn place_block(
        self,
        region: &WorldGenRegion<'_>,
        spread_pos: BlockPosition,
        spread_face: Direction,
        old_state: BlockState,
        post_process: bool,
    ) -> bool {
        match state_for_placement(region, self.block(), old_state, spread_pos, spread_face) {
            Some(new_state) => {
                if post_process {
                    region.mark_pos_for_postprocessing(spread_pos);
                }
                region.set_block(spread_pos, new_state)
            }
            None => false,
        }
    }

    fn get_spread_from_face_toward_direction(
        self,
        state: BlockState,
        region: &WorldGenRegion<'_>,
        pos: BlockPosition,
        from_face: Direction,
        spread_direction: Direction,
    ) -> Option<(BlockPosition, Direction)> {
        if spread_direction.axis() == from_face.axis() {
            return None;
        }

        if self.is_other_block_valid_as_source(state)
            || (has_face(state, from_face) && !has_face(state, spread_direction))
        {
            for &spread_type in self.spread_types() {
                let (spread_pos, spread_face) =
                    spread_type.spread_pos(pos, spread_direction, from_face);
                if self.can_spread_into(region, pos, spread_pos, spread_face) {
                    return Some((spread_pos, spread_face));
                }
            }
        }

        None
    }

    fn spread_from_face_toward_direction(
        self,
        state: BlockState,
        region: &WorldGenRegion<'_>,
        pos: BlockPosition,
        from_face: Direction,
        spread_direction: Direction,
        post_process: bool,
    ) -> bool {
        match self.get_spread_from_face_toward_direction(
            state,
            region,
            pos,
            from_face,
            spread_direction,
        ) {
            Some((spread_pos, spread_face)) => {
                let old_state = region.get_block(spread_pos);
                self.place_block(region, spread_pos, spread_face, old_state, post_process)
            }
            None => false,
        }
    }

    fn spread_from_face_toward_all_directions(
        self,
        state: BlockState,
        region: &WorldGenRegion<'_>,
        pos: BlockPosition,
        from_face: Direction,
        post_process: bool,
    ) -> i64 {
        DIRECTIONS
            .iter()
            .filter(|&&spread_direction| {
                self.spread_from_face_toward_direction(
                    state,
                    region,
                    pos,
                    from_face,
                    spread_direction,
                    post_process,
                )
            })
            .count() as i64
    }

    pub fn spread_all(
        self,
        state: BlockState,
        region: &WorldGenRegion<'_>,
        pos: BlockPosition,
        post_process: bool,
    ) -> i64 {
        DIRECTIONS
            .iter()
            .filter(|&&from_face| self.can_spread_from(state, from_face))
            .map(|&from_face| {
                self.spread_from_face_toward_all_directions(
                    state,
                    region,
                    pos,
                    from_face,
                    post_process,
                )
            })
            .sum()
    }

    pub fn spread_from_face_toward_random_direction(
        self,
        state: BlockState,
        region: &WorldGenRegion<'_>,
        pos: BlockPosition,
        starting_face: Direction,
        random: &mut RandomSource,
        post_process: bool,
    ) -> bool {
        let mut directions = DIRECTIONS;
        shuffle(&mut directions, random);
        directions.into_iter().any(|spread_direction| {
            self.spread_from_face_toward_direction(
                state,
                region,
                pos,
                starting_face,
                spread_direction,
                post_process,
            )
        })
    }
}
