use std::path::Path;

use rustc_hash::{FxHashMap, FxHashSet};
use serde::Deserialize;

use crate::datapack::{DatapackError, collect_json_files, path_to_id, read_and_parse_normalized};
use crate::util::ident::Identifier;

#[derive(Debug, Default)]
pub struct RegistryTags {
    tags: FxHashMap<Identifier, RegistryTag>,
}

#[derive(Debug)]
struct RegistryTag {
    ordered_keys: Vec<Identifier>,
    member_keys: FxHashSet<Identifier>,
}

impl RegistryTag {
    fn new(ordered_keys: Vec<Identifier>) -> Self {
        let member_keys = ordered_keys.iter().cloned().collect();
        Self {
            ordered_keys,
            member_keys,
        }
    }
}

impl RegistryTags {
    pub fn insert(&mut self, tag: Identifier, ordered_keys: Vec<Identifier>) {
        self.tags.insert(tag, RegistryTag::new(ordered_keys));
    }

    pub fn remove(&mut self, tag: &Identifier) -> Option<Vec<Identifier>> {
        self.tags.remove(tag).map(|tag| tag.ordered_keys)
    }

    pub fn contains(&self, tag: &Identifier, entry_key: &Identifier) -> bool {
        self.tags
            .get(tag)
            .is_some_and(|entries| entries.member_keys.contains(entry_key))
    }

    pub fn get(&self, tag: &Identifier) -> Option<&[Identifier]> {
        self.tags
            .get(tag)
            .map(|entries| entries.ordered_keys.as_slice())
    }

    pub fn keys(&self) -> impl Iterator<Item = &Identifier> + '_ {
        self.tags.keys()
    }
}

#[derive(Deserialize)]
struct TagJson {
    #[serde(default)]
    values: Vec<TagEntryJson>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum TagEntryJson {
    Plain(String),
    Object { id: String },
}

impl TagEntryJson {
    fn id(&self) -> &str {
        match self {
            Self::Plain(id) | Self::Object { id } => id,
        }
    }
}

enum TagEntry {
    Member(Identifier),
    Reference(Identifier),
}

fn parse_entry(raw: &str) -> TagEntry {
    match raw.strip_prefix('#') {
        Some(reference) => TagEntry::Reference(parse_id(reference)),
        None => TagEntry::Member(parse_id(raw)),
    }
}

fn parse_id(raw: &str) -> Identifier {
    raw.parse()
        .unwrap_or_else(|e| panic!("invalid id {raw}: {e}"))
}

pub fn load_tag_directory(
    namespace: &str,
    directory: &Path,
) -> Result<Vec<(Identifier, Vec<Identifier>)>, DatapackError> {
    let mut raw: FxHashMap<Identifier, Vec<TagEntry>> = FxHashMap::default();
    let mut order: Vec<Identifier> = Vec::new();

    for file in collect_json_files(directory) {
        let json: TagJson = read_and_parse_normalized(&file)?;
        let tag = parse_id(&path_to_id(namespace, &file, directory));
        let entries = json.values.iter().map(|entry| parse_entry(entry.id()));
        match raw.get_mut(&tag) {
            Some(existing) => existing.extend(entries),
            None => {
                order.push(tag.clone());
                raw.insert(tag, entries.collect());
            }
        }
    }

    let mut resolved = Vec::with_capacity(order.len());
    for tag in &order {
        let mut members = Vec::new();
        let mut seen = FxHashSet::default();
        let mut visiting = FxHashSet::default();
        resolve_tag(tag, &raw, &mut visiting, &mut seen, &mut members);
        resolved.push((tag.clone(), members));
    }
    Ok(resolved)
}

fn resolve_tag(
    tag: &Identifier,
    raw: &FxHashMap<Identifier, Vec<TagEntry>>,
    visiting: &mut FxHashSet<Identifier>,
    seen: &mut FxHashSet<Identifier>,
    out: &mut Vec<Identifier>,
) {
    if !visiting.insert(tag.clone()) {
        return;
    }
    let Some(entries) = raw.get(tag) else {
        return;
    };
    for entry in entries {
        match entry {
            TagEntry::Member(key) => {
                if seen.insert(key.clone()) {
                    out.push(key.clone());
                }
            }
            TagEntry::Reference(reference) => resolve_tag(reference, raw, visiting, seen, out),
        }
    }
    visiting.remove(tag);
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
        tags.insert(tag.clone(), vec![stone.clone()]);

        assert!(tags.contains(&tag, &stone));
        assert!(!tags.contains(&tag, &dirt));
    }

    #[test]
    fn remove_drops_the_tag() {
        let stone: Identifier = "minecraft:stone".parse().expect("test value");
        let tag: Identifier = "minecraft:mineable/pickaxe".parse().expect("test value");

        let mut tags = RegistryTags::default();
        tags.insert(tag.clone(), vec![stone.clone()]);
        assert!(tags.get(&tag).is_some());

        tags.remove(&tag);
        assert!(tags.get(&tag).is_none());
    }

    #[test]
    fn get_preserves_insertion_order() {
        let a: Identifier = "minecraft:a".parse().expect("test value");
        let b: Identifier = "minecraft:b".parse().expect("test value");
        let tag: Identifier = "minecraft:ordered".parse().expect("test value");

        let mut tags = RegistryTags::default();
        tags.insert(tag.clone(), vec![a.clone(), b.clone()]);

        assert_eq!(tags.get(&tag), Some([a, b].as_slice()));
    }
}
