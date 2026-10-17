use rustc_hash::FxHashMap;
use shulkr::util::Key;

use crate::util::rng::legacy_random::LegacyRandomSource;
use crate::world::worldgen::structure::pools::alias::pool_alias_binding::PoolAliasBinding;
use shulkr::util::BlockPosition;

#[derive(Debug, Default, Clone)]
pub struct PoolAliasLookup {
    mappings: FxHashMap<Key, Key>,
}

impl PoolAliasLookup {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn create(bindings: &[PoolAliasBinding], pos: BlockPosition, seed: i64) -> Self {
        if bindings.is_empty() {
            return Self::empty();
        }

        let mut source = LegacyRandomSource::from_seed(seed as u64);
        let factory = source.next_positional();
        let mut random = factory.at(pos.x(), pos.y(), pos.z());

        let mut mappings = FxHashMap::default();
        for binding in bindings {
            binding.for_each_resolved(&mut random, &mut |alias, target| {
                mappings.entry(alias).or_insert(target);
            });
        }
        Self { mappings }
    }

    pub fn lookup(&self, alias: &Key) -> Key {
        self.mappings.get(alias).unwrap_or(alias).clone()
    }
}
