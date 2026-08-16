use crate::registry::REGISTRY;
use crate::registry::blocks::vanilla_blocks::BLOCK_TRANSFORMS;
use crate::util::axis::Axis;
use crate::util::direction::Direction;
use crate::util::ident::BlockState;
use crate::util::mirror::Mirror;
use crate::util::rotation::Rotation;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockTransform {
    None,
    Facing,
    FacingRod,
    Stairs,
    Door,
    Rotation16,
    Pillar,
    Connections,
    FaceMap,
    Rail,
    Orientation,
}

const HORIZONTALS: [Direction; 4] = [
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];

const ALL_FACES: [Direction; 6] = [
    Direction::Down,
    Direction::Up,
    Direction::North,
    Direction::South,
    Direction::West,
    Direction::East,
];

pub fn rotate(state: BlockState, rotation: Rotation) -> BlockState {
    if rotation == Rotation::None {
        return state;
    }
    match transform_of(state) {
        BlockTransform::None => state,
        BlockTransform::Facing | BlockTransform::FacingRod | BlockTransform::Stairs
        | BlockTransform::Door => rotate_facing(state, rotation),
        BlockTransform::Rotation16 => map_property(state, "rotation", |value| {
            let index = value.parse::<i32>().unwrap_or(0);
            rotation.rotate_index(index, 16).to_string()
        }),
        BlockTransform::Pillar => map_property(state, "axis", |value| match rotation {
            Rotation::Clockwise90 | Rotation::CounterClockwise90 => match value {
                "x" => "z".to_owned(),
                "z" => "x".to_owned(),
                other => other.to_owned(),
            },
            Rotation::None | Rotation::Clockwise180 => value.to_owned(),
        }),
        BlockTransform::Connections => map_faces(state, &HORIZONTALS, |face| rotation.rotate(face)),
        BlockTransform::FaceMap => map_faces(state, &ALL_FACES, |face| rotation.rotate(face)),
        BlockTransform::Rail => map_property(state, "shape", |value| {
            rotate_rail_shape(value, rotation).to_owned()
        }),
        BlockTransform::Orientation => {
            map_orientation(state, |direction| rotation.rotate(direction))
        }
    }
}

pub fn mirror(state: BlockState, mirror: Mirror) -> BlockState {
    if mirror == Mirror::None {
        return state;
    }
    match transform_of(state) {
        BlockTransform::None => state,
        BlockTransform::Facing => rotate(state, mirror.get_rotation(facing_of(state))),
        BlockTransform::FacingRod => {
            map_property(state, "facing", |value| {
                mirror.mirror(direction_from_str(value)).as_str().to_owned()
            })
        }
        BlockTransform::Stairs => mirror_stairs(state, mirror),
        BlockTransform::Door => {
            let rotated = rotate(state, mirror.get_rotation(facing_of(state)));
            map_property(rotated, "hinge", |value| match value {
                "left" => "right".to_owned(),
                _ => "left".to_owned(),
            })
        }
        BlockTransform::Rotation16 => map_property(state, "rotation", |value| {
            let index = value.parse::<i32>().unwrap_or(0);
            mirror.mirror_index(index, 16).to_string()
        }),
        BlockTransform::Pillar => state,
        BlockTransform::Connections => map_faces(state, &HORIZONTALS, |face| mirror.mirror(face)),
        BlockTransform::FaceMap => map_faces(state, &ALL_FACES, |face| mirror.mirror(face)),
        BlockTransform::Rail => map_property(state, "shape", |value| {
            mirror_rail_shape(value, mirror).to_owned()
        }),
        BlockTransform::Orientation => map_orientation(state, |direction| mirror.mirror(direction)),
    }
}

fn transform_of(state: BlockState) -> BlockTransform {
    REGISTRY
        .blocks
        .state_to_block_id
        .get(state.0 as usize)
        .and_then(|block_id| BLOCK_TRANSFORMS.get(*block_id))
        .copied()
        .unwrap_or(BlockTransform::None)
}

fn rotate_facing(state: BlockState, rotation: Rotation) -> BlockState {
    map_property(state, "facing", |value| {
        rotation.rotate(direction_from_str(value)).as_str().to_owned()
    })
}

fn facing_of(state: BlockState) -> Direction {
    REGISTRY
        .blocks
        .get_properties(state)
        .into_iter()
        .find(|(name, _)| *name == "facing")
        .map_or(Direction::North, |(_, value)| direction_from_str(value))
}

fn map_property(
    state: BlockState,
    name: &str,
    transform: impl Fn(&str) -> String,
) -> BlockState {
    let Some(block) = REGISTRY.blocks.by_state_id(state) else {
        return state;
    };
    let properties = REGISTRY.blocks.get_properties(state);
    let mut replacement = None;
    let mut updated: Vec<(&str, &str)> = Vec::with_capacity(properties.len());
    for (property, value) in &properties {
        if *property == name {
            replacement = Some(transform(value));
        }
        updated.push((property, value));
    }
    let Some(replacement) = replacement else {
        return state;
    };
    for entry in &mut updated {
        if entry.0 == name {
            entry.1 = replacement.as_str();
        }
    }
    REGISTRY
        .blocks
        .state_id_from_block_properties(block, &updated)
        .unwrap_or(state)
}

fn map_faces(
    state: BlockState,
    faces: &[Direction],
    transform: impl Fn(Direction) -> Direction,
) -> BlockState {
    let Some(block) = REGISTRY.blocks.by_state_id(state) else {
        return state;
    };
    let properties = REGISTRY.blocks.get_properties(state);
    if !faces
        .iter()
        .all(|face| properties.iter().any(|(name, _)| *name == face.as_str()))
    {
        return state;
    }
    let mut updated: Vec<(&str, &str)> = properties.clone();
    for face in faces {
        let value = properties
            .iter()
            .find(|(name, _)| *name == face.as_str())
            .map_or("", |(_, value)| *value);
        let target = transform(*face).as_str();
        for entry in &mut updated {
            if entry.0 == target {
                entry.1 = value;
            }
        }
    }
    REGISTRY
        .blocks
        .state_id_from_block_properties(block, &updated)
        .unwrap_or(state)
}

fn map_orientation(state: BlockState, transform: impl Fn(Direction) -> Direction) -> BlockState {
    map_property(state, "orientation", |value| {
        let Some((front, top)) = value.split_once('_') else {
            return value.to_owned();
        };
        format!(
            "{}_{}",
            transform(direction_from_str(front)).as_str(),
            transform(direction_from_str(top)).as_str()
        )
    })
}

fn mirror_stairs(state: BlockState, mirror: Mirror) -> BlockState {
    let facing = facing_of(state);
    let shape = REGISTRY
        .blocks
        .get_properties(state)
        .into_iter()
        .find(|(name, _)| *name == "shape")
        .map_or("straight", |(_, value)| value);

    let flip = match mirror {
        Mirror::LeftRight => facing.get_axis() == Axis::Z,
        Mirror::FrontBack => facing.get_axis() == Axis::X,
        Mirror::None => false,
    };
    if !flip {
        return state;
    }

    let rotated = rotate(state, Rotation::Clockwise180);
    let new_shape = match (mirror, shape) {
        (_, "straight") => return rotated,
        (Mirror::LeftRight, "outer_left") => "outer_right",
        (Mirror::LeftRight, "outer_right") => "outer_left",
        (Mirror::LeftRight, "inner_left") => "inner_right",
        (Mirror::LeftRight, "inner_right") => "inner_left",
        (Mirror::FrontBack, "outer_left") => "outer_right",
        (Mirror::FrontBack, "outer_right") => "outer_left",
        (Mirror::FrontBack, "inner_left") => "inner_left",
        (Mirror::FrontBack, "inner_right") => "inner_right",
        _ => return rotated,
    };
    map_property(rotated, "shape", |_| new_shape.to_owned())
}

const fn rotate_rail_shape(shape: &str, rotation: Rotation) -> &'static str {
    match rotation {
        Rotation::None => leak_rail(shape),
        Rotation::Clockwise180 => match shape {
            "ascending_east" => "ascending_west",
            "ascending_west" => "ascending_east",
            "ascending_north" => "ascending_south",
            "ascending_south" => "ascending_north",
            "south_east" => "north_west",
            "south_west" => "north_east",
            "north_west" => "south_east",
            "north_east" => "south_west",
            other => leak_rail(other),
        },
        Rotation::CounterClockwise90 => match shape {
            "ascending_east" => "ascending_north",
            "ascending_west" => "ascending_south",
            "ascending_north" => "ascending_west",
            "ascending_south" => "ascending_east",
            "north_south" => "east_west",
            "east_west" => "north_south",
            "south_east" => "north_east",
            "south_west" => "south_east",
            "north_west" => "south_west",
            "north_east" => "north_west",
            other => leak_rail(other),
        },
        Rotation::Clockwise90 => match shape {
            "ascending_east" => "ascending_south",
            "ascending_west" => "ascending_north",
            "ascending_north" => "ascending_east",
            "ascending_south" => "ascending_west",
            "north_south" => "east_west",
            "east_west" => "north_south",
            "south_east" => "south_west",
            "south_west" => "north_west",
            "north_west" => "north_east",
            "north_east" => "south_east",
            other => leak_rail(other),
        },
    }
}

const fn mirror_rail_shape(shape: &str, mirror: Mirror) -> &'static str {
    match mirror {
        Mirror::None => leak_rail(shape),
        Mirror::LeftRight => match shape {
            "ascending_north" => "ascending_south",
            "ascending_south" => "ascending_north",
            "south_east" => "north_east",
            "south_west" => "north_west",
            "north_west" => "south_west",
            "north_east" => "south_east",
            other => leak_rail(other),
        },
        Mirror::FrontBack => match shape {
            "ascending_east" => "ascending_west",
            "ascending_west" => "ascending_east",
            "south_east" => "south_west",
            "south_west" => "south_east",
            "north_west" => "north_east",
            "north_east" => "north_west",
            other => leak_rail(other),
        },
    }
}

const fn leak_rail(shape: &str) -> &'static str {
    match shape {
        "north_south" => "north_south",
        "east_west" => "east_west",
        "ascending_east" => "ascending_east",
        "ascending_west" => "ascending_west",
        "ascending_north" => "ascending_north",
        "ascending_south" => "ascending_south",
        "south_east" => "south_east",
        "south_west" => "south_west",
        "north_west" => "north_west",
        _ => "north_east",
    }
}

const fn direction_from_str(value: &str) -> Direction {
    match value {
        "down" => Direction::Down,
        "up" => Direction::Up,
        "south" => Direction::South,
        "west" => Direction::West,
        "east" => Direction::East,
        _ => Direction::North,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Once;

    use super::*;
    use crate::registry::Registries;
    use crate::registry::blocks::{BlockRef, vanilla_blocks};

    fn init_global_registry() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let mut registries = Registries::new_vanilla().expect("vanilla datapack should load");
            registries.freeze();
            let _ = REGISTRY.init(registries);
        });
    }

    fn state_of(block: BlockRef, properties: &[(&str, &str)]) -> BlockState {
        REGISTRY
            .blocks
            .state_id_from_block_defaulted_properties(block, properties.iter().copied())
            .expect("state should exist")
    }

    fn property_of(state: BlockState, name: &str) -> String {
        REGISTRY
            .blocks
            .get_properties(state)
            .into_iter()
            .find(|(property, _)| *property == name)
            .map(|(_, value)| value.to_owned())
            .expect("property should exist")
    }

    #[test]
    fn pillar_axis_swaps_on_quarter_turns_only() {
        init_global_registry();
        let x_axis = state_of(&vanilla_blocks::BONE_BLOCK, &[("axis", "x")]);
        assert_eq!(
            property_of(rotate(x_axis, Rotation::Clockwise90), "axis"),
            "z"
        );
        assert_eq!(
            property_of(rotate(x_axis, Rotation::Clockwise180), "axis"),
            "x"
        );
        let y_axis = state_of(&vanilla_blocks::BONE_BLOCK, &[("axis", "y")]);
        assert_eq!(
            property_of(rotate(y_axis, Rotation::Clockwise90), "axis"),
            "y"
        );
    }

    #[test]
    fn facing_blocks_rotate_and_mirror_like_vanilla() {
        init_global_registry();
        let chest = state_of(&vanilla_blocks::CHEST, &[("facing", "north")]);
        assert_eq!(
            property_of(rotate(chest, Rotation::Clockwise90), "facing"),
            "east"
        );
        assert_eq!(
            property_of(mirror(chest, Mirror::LeftRight), "facing"),
            "south"
        );
        assert_eq!(
            property_of(mirror(chest, Mirror::FrontBack), "facing"),
            "north"
        );
    }

    #[test]
    fn blocks_without_a_vanilla_override_are_untouched() {
        init_global_registry();
        let piston = state_of(&vanilla_blocks::PISTON, &[("facing", "north")]);
        assert_eq!(rotate(piston, Rotation::Clockwise90), piston);
        let fire = state_of(&vanilla_blocks::FIRE, &[("north", "true")]);
        assert_eq!(rotate(fire, Rotation::Clockwise90), fire);
    }

    #[test]
    fn connection_flags_follow_their_face() {
        init_global_registry();
        let bars = state_of(
            &vanilla_blocks::IRON_BARS,
            &[
                ("north", "true"),
                ("east", "false"),
                ("south", "false"),
                ("west", "false"),
            ],
        );
        let rotated = rotate(bars, Rotation::Clockwise90);
        assert_eq!(property_of(rotated, "east"), "true");
        assert_eq!(property_of(rotated, "north"), "false");
    }

    #[test]
    fn sign_rotation_wraps_in_sixteenths() {
        init_global_registry();
        let sign = state_of(&vanilla_blocks::OAK_SIGN, &[("rotation", "0")]);
        assert_eq!(
            property_of(rotate(sign, Rotation::Clockwise90), "rotation"),
            "4"
        );
        assert_eq!(
            property_of(mirror(sign, Mirror::LeftRight), "rotation"),
            "8"
        );
    }
}
