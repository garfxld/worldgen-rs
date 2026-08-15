pub mod biome;
pub mod blocks;
pub mod carver;
pub mod dimension_type;
pub mod fluid;
pub mod tags;

use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use rustc_hash::FxHashMap;

use crate::util::datapack::LoadError;
use crate::util::ident::Identifier;

pub use tags::RegistryTags;

pub struct Registry<T: 'static> {
    entries: Vec<(Identifier, &'static T)>,
    by_key: FxHashMap<Identifier, usize>,
    tags: RegistryTags,
    frozen: bool,
}

impl<T: 'static> Default for Registry<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: 'static> Registry<T> {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            by_key: FxHashMap::default(),
            tags: RegistryTags::default(),
            frozen: false,
        }
    }

    pub fn register(&mut self, key: Identifier, entry: &'static T) -> usize {
        assert!(
            !self.frozen,
            "cannot register an entry after the registry has been frozen"
        );
        if let Some(&id) = self.by_key.get(&key) {
            self.entries[id] = (key, entry);
            return id;
        }
        let id = self.entries.len();
        self.by_key.insert(key.clone(), id);
        self.entries.push((key, entry));
        id
    }

    pub const fn freeze(&mut self) {
        self.frozen = true;
    }

    pub fn by_id(&self, id: usize) -> Option<&'static T> {
        self.entries.get(id).map(|&(_, entry)| entry)
    }

    pub fn by_key(&self, key: &Identifier) -> Option<&'static T> {
        self.by_key.get(key).and_then(|&id| self.by_id(id))
    }

    pub fn id_from_key(&self, key: &Identifier) -> Option<usize> {
        self.by_key.get(key).copied()
    }

    pub fn key_of(&self, id: usize) -> Option<&Identifier> {
        self.entries.get(id).map(|(key, _)| key)
    }

    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (usize, &'static T)> + '_ {
        self.entries
            .iter()
            .enumerate()
            .map(|(id, &(_, entry))| (id, entry))
    }

    pub fn register_tag(&mut self, tag: Identifier, keys: &[&'static str]) {
        assert!(
            !self.frozen,
            "cannot register a tag after the registry has been frozen"
        );
        let entry_keys = keys
            .iter()
            .filter_map(|key| {
                let ident = registry_vanilla_or_custom_tag(key);
                self.by_key.contains_key(&ident).then_some(ident)
            })
            .collect();
        self.tags.insert(tag, entry_keys);
    }

    pub fn modify_tag(
        &mut self,
        tag: &Identifier,
        f: impl FnOnce(Vec<Identifier>) -> Vec<Identifier>,
    ) {
        let existing = self.tags.remove(tag).unwrap_or_default();
        let entry_keys = f(existing)
            .into_iter()
            .filter(|key| {
                let known = self.by_key.contains_key(key);
                if !known {
                    tracing::error!("{key} not found in registry, skipping from tag {tag}");
                }
                known
            })
            .collect();
        self.tags.insert(tag.clone(), entry_keys);
    }

    pub fn load_tags_from_datapack(
        &mut self,
        namespace: &str,
        directory: &Path,
    ) -> Result<(), LoadError> {
        for (tag, members) in tags::load_tag_directory(namespace, directory)? {
            let known = members
                .into_iter()
                .filter(|key| self.by_key.contains_key(key))
                .collect();
            self.tags.insert(tag, known);
        }
        Ok(())
    }

    pub fn is_in_tag(&self, key: &Identifier, tag: &Identifier) -> bool {
        self.tags.contains(tag, key)
    }

    pub fn get_tag(&self, tag: &Identifier) -> Option<Vec<&'static T>> {
        self.tags
            .get(tag)
            .map(|keys| keys.iter().filter_map(|key| self.by_key(key)).collect())
    }

    pub fn iter_tag(&self, tag: &Identifier) -> impl Iterator<Item = &'static T> + '_ {
        self.tags
            .get(tag)
            .into_iter()
            .flat_map(|keys| keys.iter().filter_map(|key| self.by_key(key)))
    }

    pub fn tag_keys(&self) -> impl Iterator<Item = &Identifier> + '_ {
        self.tags.keys()
    }
}

pub fn registry_vanilla_or_custom_tag(key: &'static str) -> Identifier {
    if let Some(key) = key.strip_prefix("c:") {
        Identifier::new(String::from("c"), key)
    } else {
        Identifier::new(Identifier::VANILLA_NAMESPACE, key)
    }
}

pub struct RegistryLock(OnceLock<Registries>);

impl RegistryLock {
    #[expect(
        clippy::result_large_err,
        reason = "returning the whole Registries on double-init is a cold path"
    )]
    pub fn init(&self, value: Registries) -> Result<(), Registries> {
        self.0.set(value)
    }
}

impl Deref for RegistryLock {
    type Target = Registries;

    fn deref(&self) -> &Self::Target {
        self.0.get().expect("Registries not init")
    }
}

pub static REGISTRY: RegistryLock = RegistryLock(OnceLock::new());

pub struct Registries {
    pub blocks: blocks::BlockRegistry,
    pub fluids: fluid::FluidRegistry,
    pub biomes: biome::BiomeRegistry,
    pub carvers: carver::ConfiguredCarverRegistry,
    pub dimension_types: dimension_type::DimensionTypeRegistry,
}

fn vanilla_datapack_worldgen_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/datapack/worldgen")
}

fn vanilla_datapack_tags_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/datapack/tags")
}

impl Registries {
    pub fn new_vanilla() -> Result<Self, LoadError> {
        Self::new_with_overlay(None)
    }

    pub fn new_with_datapack(overlay_worldgen_dir: &Path) -> Result<Self, LoadError> {
        Self::new_with_overlay(Some(overlay_worldgen_dir))
    }

    fn new_with_overlay(overlay_worldgen_dir: Option<&Path>) -> Result<Self, LoadError> {
        let mut fluids = fluid::FluidRegistry::new();
        fluid::vanilla_fluids::register_fluids(&mut fluids);
        fluids.load_tags_from_datapack("minecraft", &vanilla_datapack_tags_dir().join("fluid"))?;

        let mut blocks = blocks::BlockRegistry::new();
        blocks::vanilla_blocks::register_blocks(&mut blocks);
        blocks.load_tags_from_datapack("minecraft", &vanilla_datapack_tags_dir())?;

        let mut biomes = biome::BiomeRegistry::new();
        biomes.load_from_datapack("minecraft", &vanilla_datapack_worldgen_dir())?;
        if let Some(overlay) = overlay_worldgen_dir {
            biomes.load_from_datapack("minecraft", overlay)?;
        }

        let mut carvers = carver::ConfiguredCarverRegistry::new();
        carver::vanilla_carvers::register_carvers(&mut carvers);

        let mut dimension_types = dimension_type::DimensionTypeRegistry::new();
        dimension_type::vanilla_dimension_types::register_dimension_types(&mut dimension_types);

        Ok(Self {
            blocks,
            fluids,
            biomes,
            carvers,
            dimension_types,
        })
    }

    pub const fn freeze(&mut self) {
        self.blocks.freeze();
        self.fluids.freeze();
        self.biomes.freeze();
        self.carvers.freeze();
        self.dimension_types.freeze();
    }
}
