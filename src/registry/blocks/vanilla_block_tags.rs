use crate::registry::blocks::BlockRegistry;
use crate::util::ident::Identifier;

pub const LEAVES: Identifier = Identifier::const_vanilla("leaves");
pub const OVERWORLD_CARVER_REPLACEABLES: Identifier =
    Identifier::const_vanilla("overworld_carver_replaceables");
pub const NETHER_CARVER_REPLACEABLES: Identifier =
    Identifier::const_vanilla("nether_carver_replaceables");

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

const OVERWORLD_CARVER_REPLACEABLES_KEYS: &[&str] = &[
    "stone",
    "granite",
    "diorite",
    "andesite",
    "tuff",
    "deepslate",
    "dirt",
    "coarse_dirt",
    "rooted_dirt",
    "mud",
    "muddy_mangrove_roots",
    "moss_block",
    "pale_moss_block",
    "grass_block",
    "podzol",
    "mycelium",
    "sand",
    "red_sand",
    "suspicious_sand",
    "terracotta",
    "white_terracotta",
    "orange_terracotta",
    "magenta_terracotta",
    "light_blue_terracotta",
    "yellow_terracotta",
    "lime_terracotta",
    "pink_terracotta",
    "gray_terracotta",
    "light_gray_terracotta",
    "cyan_terracotta",
    "purple_terracotta",
    "blue_terracotta",
    "brown_terracotta",
    "green_terracotta",
    "red_terracotta",
    "black_terracotta",
    "iron_ore",
    "deepslate_iron_ore",
    "copper_ore",
    "deepslate_copper_ore",
    "snow",
    "snow_block",
    "powder_snow",
    "water",
    "gravel",
    "suspicious_gravel",
    "sandstone",
    "red_sandstone",
    "calcite",
    "packed_ice",
    "raw_iron_block",
    "raw_copper_block",
    "cinnabar",
    "sulfur",
    "potent_sulfur",
];

const NETHER_CARVER_REPLACEABLES_KEYS: &[&str] = &[
    "stone",
    "granite",
    "diorite",
    "andesite",
    "tuff",
    "deepslate",
    "netherrack",
    "basalt",
    "blackstone",
    "dirt",
    "coarse_dirt",
    "rooted_dirt",
    "mud",
    "muddy_mangrove_roots",
    "moss_block",
    "pale_moss_block",
    "grass_block",
    "podzol",
    "mycelium",
    "crimson_nylium",
    "warped_nylium",
    "nether_wart_block",
    "warped_wart_block",
    "soul_sand",
    "soul_soil",
];

pub fn register_tags(registry: &mut BlockRegistry) {
    registry.register_tag(LEAVES, LEAVES_KEYS);
    registry.register_tag(
        OVERWORLD_CARVER_REPLACEABLES,
        OVERWORLD_CARVER_REPLACEABLES_KEYS,
    );
    registry.register_tag(NETHER_CARVER_REPLACEABLES, NETHER_CARVER_REPLACEABLES_KEYS);
}
