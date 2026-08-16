pub mod alias;
pub mod dimension_padding;
pub mod jigsaw_junction;
pub mod jigsaw_placement;
pub mod structure_pool_element;
pub mod structure_template_pool;

use rustc_hash::FxHashMap;

use crate::util::ident::Identifier;
use crate::worldgen::structure::registry;
use crate::worldgen::structure::pools::structure_template_pool::StructureTemplatePool;


pub struct TemplatePoolRegistry {
    pools: FxHashMap<Identifier, &'static StructureTemplatePool>,
}

impl TemplatePoolRegistry {
    pub fn get(&self, key: &Identifier) -> Option<&'static StructureTemplatePool> {
        self.pools.get(key).copied()
    }

    pub const fn new(pools: FxHashMap<Identifier, &'static StructureTemplatePool>) -> Self {
        Self { pools }
    }
}

pub fn template_pools() -> &'static TemplatePoolRegistry {
    registry().template_pools()
}
