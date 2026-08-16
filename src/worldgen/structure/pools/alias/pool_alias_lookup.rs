use rustc_hash::FxHashMap;

use crate::util::ident::Identifier;
use crate::util::pos::BlockPos;
use crate::util::rng::xoroshiro::XoroshiroRandomSource;
use crate::worldgen::structure::pools::alias::pool_alias_binding::PoolAliasBinding;

#[derive(Debug, Default, Clone)]
pub struct PoolAliasLookup {
    mappings: FxHashMap<Identifier, Identifier>,
}

impl PoolAliasLookup {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn create(bindings: &[PoolAliasBinding], pos: BlockPos, seed: i64) -> Self {
        if bindings.is_empty() {
            return Self::empty();
        }

        let mut source = XoroshiroRandomSource::from_seed(seed as u64);
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

    pub fn lookup(&self, alias: &Identifier) -> Identifier {
        self.mappings.get(alias).unwrap_or(alias).clone()
    }
}
