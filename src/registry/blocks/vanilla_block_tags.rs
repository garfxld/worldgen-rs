use crate::registry::blocks::BlockRegistry;
use crate::util::ident::Identifier;

pub const LEAVES: Identifier = Identifier::const_vanilla("leaves");

const LEAVES_KEYS: &[&str] = &[
    "jungle_leaves",
    "oak_leaves",
    "spruce_leaves",
    "pale_oak_leaves",
    "dark_oak_leaves",
    "acacia_leaves",
    "birch_leaves",
    "azalea_leaves",
    "flowering_azalea_leaves",
    "mangrove_leaves",
    "cherry_leaves",
];

pub fn register_tags(registry: &mut BlockRegistry) {
    registry.register_tag(LEAVES, LEAVES_KEYS);
}
