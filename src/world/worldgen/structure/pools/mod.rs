pub mod alias;
pub mod dimension_padding;
pub mod jigsaw_junction;
pub mod jigsaw_placement;
pub mod structure_pool_element;
pub mod structure_template_pool;

use rustc_hash::FxHashMap;
use shulkr::util::Key;

use crate::world::worldgen::structure::pools::structure_template_pool::StructureTemplatePool;
use crate::world::worldgen::structure::registry;

pub struct TemplatePoolRegistry {
    pools: FxHashMap<Key, &'static StructureTemplatePool>,
}

impl TemplatePoolRegistry {
    pub fn get(&self, key: &Key) -> Option<&'static StructureTemplatePool> {
        self.pools.get(key).copied()
    }

    pub const fn new(pools: FxHashMap<Key, &'static StructureTemplatePool>) -> Self {
        Self { pools }
    }
}

pub fn template_pools() -> &'static TemplatePoolRegistry {
    registry().template_pools()
}
