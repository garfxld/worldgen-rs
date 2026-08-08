use rustc_hash::{FxHashMap, FxHashSet};

use crate::util::ident::Identifier;

#[derive(Debug, Default)]
pub struct RegistryTags {
    tags: FxHashMap<Identifier, RegistryTag>,
}

#[derive(Debug)]
struct RegistryTag {
    ordered_keys: Vec<&'static Identifier>,
    member_keys: FxHashSet<&'static Identifier>,
}

impl RegistryTag {
    fn new(ordered_keys: Vec<&'static Identifier>) -> Self {
        let member_keys = ordered_keys.iter().copied().collect();
        Self {
            ordered_keys,
            member_keys,
        }
    }
}

impl RegistryTags {
    pub fn insert(&mut self, tag: Identifier, ordered_keys: Vec<&'static Identifier>) {
        self.tags.insert(tag, RegistryTag::new(ordered_keys));
    }

    pub fn remove(&mut self, tag: &Identifier) -> Option<Vec<&'static Identifier>> {
        self.tags.remove(tag).map(|tag| tag.ordered_keys)
    }

    #[must_use]
    pub fn contains(&self, tag: &Identifier, entry_key: &Identifier) -> bool {
        self.tags
            .get(tag)
            .is_some_and(|entries| entries.member_keys.contains(entry_key))
    }

    #[must_use]
    pub fn get(&self, tag: &Identifier) -> Option<&[&'static Identifier]> {
        self.tags
            .get(tag)
            .map(|entries| entries.ordered_keys.as_slice())
    }

    pub fn keys(&self) -> impl Iterator<Item = &Identifier> + '_ {
        self.tags.keys()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_reflects_membership() {
        let stone: Identifier = "minecraft:stone".parse().expect("test value");
        let dirt: Identifier = "minecraft:dirt".parse().expect("test value");
        let tag: Identifier = "minecraft:mineable/pickaxe".parse().expect("test value");

        let mut tags = RegistryTags::default();
        tags.insert(tag.clone(), vec![Box::leak(Box::new(stone.clone()))]);

        assert!(tags.contains(&tag, &stone));
        assert!(!tags.contains(&tag, &dirt));
    }

    #[test]
    fn remove_drops_the_tag() {
        let stone: Identifier = "minecraft:stone".parse().expect("test value");
        let tag: Identifier = "minecraft:mineable/pickaxe".parse().expect("test value");

        let mut tags = RegistryTags::default();
        tags.insert(tag.clone(), vec![Box::leak(Box::new(stone.clone()))]);
        assert!(tags.get(&tag).is_some());

        tags.remove(&tag);
        assert!(tags.get(&tag).is_none());
    }

    #[test]
    fn get_preserves_insertion_order() {
        let a: &'static Identifier =
            Box::leak(Box::new("minecraft:a".parse().expect("test value")));
        let b: &'static Identifier =
            Box::leak(Box::new("minecraft:b".parse().expect("test value")));
        let tag: Identifier = "minecraft:ordered".parse().expect("test value");

        let mut tags = RegistryTags::default();
        tags.insert(tag.clone(), vec![a, b]);

        assert_eq!(tags.get(&tag), Some([a, b].as_slice()));
    }
}
