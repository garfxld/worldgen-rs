use crate::util::ident::Identifier;
use crate::worldgen::structure::placement::structure_placement::StructurePlacement;
use crate::worldgen::structure::structure::Structure;

#[derive(Debug)]
pub struct StructureSelectionEntry {
    pub key: Identifier,
    pub structure: &'static Structure,
    pub weight: i32,
}

#[derive(Debug)]
pub struct StructureSet {
    pub key: Identifier,
    pub structures: Vec<StructureSelectionEntry>,
    pub placement: StructurePlacement,
}
