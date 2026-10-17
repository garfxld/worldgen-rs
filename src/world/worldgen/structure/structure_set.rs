use shulkr::util::Key;

use crate::world::worldgen::structure::placement::structure_placement::StructurePlacement;
use crate::world::worldgen::structure::structure::Structure;

#[derive(Debug)]
pub struct StructureSelectionEntry {
    pub key: Key,
    pub structure: &'static Structure,
    pub weight: i32,
}

#[derive(Debug)]
pub struct StructureSet {
    pub key: Key,
    pub structures: Vec<StructureSelectionEntry>,
    pub placement: StructurePlacement,
}
