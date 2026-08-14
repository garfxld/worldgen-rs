use std::path::Path;
use std::sync::OnceLock;

use serde::{Deserialize, Deserializer};

use crate::registry::{REGISTRY, Registry};
use crate::util::datapack::{LoadError, collect_json_files, path_to_id, read_and_parse_normalized};
use crate::util::ident::Identifier;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemperatureModifier {
    None,
    Frozen,
}

#[derive(Debug)]
pub struct Biome {
    pub key: Identifier,
    pub has_precipitation: bool,
    pub temperature: f32,
    pub temperature_modifier: TemperatureModifier,
    pub carvers: Vec<Identifier>,
    pub features: Vec<Vec<Identifier>>,
    pub id: OnceLock<usize>,
}

impl Biome {
    pub fn has_tag(&'static self, tag: &Identifier) -> bool {
        REGISTRY.biomes.is_in_tag(&self.key, tag)
    }

    pub fn id(&self) -> usize {
        *self.id.get().expect("biome not registered")
    }
}

pub type BiomeRef = &'static Biome;

#[derive(Deserialize)]
struct BiomeJson {
    has_precipitation: bool,
    temperature: f32,
    #[serde(default)]
    temperature_modifier: Option<String>,
    #[serde(deserialize_with = "one_or_many")]
    carvers: Vec<String>,
    features: Vec<Vec<String>>,
}

fn one_or_many<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(String),
        Many(Vec<String>),
    }
    Ok(match OneOrMany::deserialize(d)? {
        OneOrMany::One(s) => vec![s],
        OneOrMany::Many(v) => v,
    })
}

fn identifier(namespace: &str, raw: &str) -> Identifier {
    if let Some((ns, path)) = raw.split_once(':') {
        Identifier::new(ns.to_owned(), path.to_owned())
    } else {
        Identifier::new(namespace.to_owned(), raw.to_owned())
    }
}

pub struct BiomeRegistry {
    registry: Registry<Biome>,
}

impl Default for BiomeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl BiomeRegistry {
    pub fn new() -> Self {
        Self {
            registry: Registry::new(),
        }
    }

    pub fn register(&mut self, biome: BiomeRef) -> usize {
        let id = self.registry.register(biome.key.clone(), biome);
        let cached = biome.id.get_or_init(|| id);
        assert_eq!(*cached, id, "biome registered with conflicting id");
        id
    }

    pub fn load_from_datapack(
        &mut self,
        namespace: &str,
        worldgen_dir: &Path,
    ) -> Result<(), LoadError> {
        let dir = worldgen_dir.join("biome");
        for file in collect_json_files(&dir) {
            let json: BiomeJson = read_and_parse_normalized(&file)?;
            let key = identifier(namespace, &path_to_id(namespace, &file, &dir));

            let temperature_modifier = match json.temperature_modifier.as_deref() {
                Some("frozen") => TemperatureModifier::Frozen,
                _ => TemperatureModifier::None,
            };
            let carvers = json
                .carvers
                .iter()
                .map(|raw| identifier(namespace, raw))
                .collect();
            let features = json
                .features
                .into_iter()
                .map(|step| step.iter().map(|raw| identifier(namespace, raw)).collect())
                .collect();

            let biome = Box::leak(Box::new(Biome {
                key,
                has_precipitation: json.has_precipitation,
                temperature: json.temperature,
                temperature_modifier,
                carvers,
                features,
                id: OnceLock::new(),
            }));
            self.register(biome);
        }
        Ok(())
    }

    pub const fn freeze(&mut self) {
        self.registry.freeze();
    }

    pub fn by_id(&self, id: usize) -> Option<BiomeRef> {
        self.registry.by_id(id)
    }

    pub fn by_key(&self, key: &Identifier) -> Option<BiomeRef> {
        self.registry.by_key(key)
    }

    pub fn is_in_tag(&self, key: &Identifier, tag: &Identifier) -> bool {
        self.registry.is_in_tag(key, tag)
    }

    pub fn register_tag(&mut self, tag: Identifier, keys: &[&'static str]) {
        self.registry.register_tag(tag, keys);
    }

    pub fn iter(&self) -> impl Iterator<Item = (usize, BiomeRef)> + '_ {
        self.registry.iter()
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "biome registry tests assert required vanilla properties are present"
)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn datapack_worldgen_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/datapack/worldgen")
    }

    fn create_test_registry() -> BiomeRegistry {
        let mut registry = BiomeRegistry::new();
        registry
            .load_from_datapack("minecraft", &datapack_worldgen_dir())
            .unwrap();
        registry.freeze();
        registry
    }

    #[test]
    fn plains_has_expected_climate() {
        let registry = create_test_registry();
        let plains = registry
            .by_key(&Identifier::const_vanilla("plains"))
            .unwrap();

        assert!(plains.has_precipitation);
        assert_eq!(plains.temperature, 0.8);
        assert_eq!(plains.temperature_modifier, TemperatureModifier::None);
        assert!(plains.carvers.contains(&Identifier::const_vanilla("cave")));
    }

    #[test]
    fn frozen_ocean_uses_frozen_temperature_modifier() {
        let registry = create_test_registry();
        let frozen_ocean = registry
            .by_key(&Identifier::const_vanilla("frozen_ocean"))
            .unwrap();

        assert_eq!(
            frozen_ocean.temperature_modifier,
            TemperatureModifier::Frozen
        );
    }

    #[test]
    fn nether_wastes_single_carver_is_not_dropped() {
        let registry = create_test_registry();
        let nether_wastes = registry
            .by_key(&Identifier::const_vanilla("nether_wastes"))
            .unwrap();

        assert_eq!(
            nether_wastes.carvers,
            vec![Identifier::const_vanilla("nether_cave")]
        );
    }

    #[test]
    fn ids_are_stable_and_cached() {
        let registry = create_test_registry();
        let plains = registry
            .by_key(&Identifier::const_vanilla("plains"))
            .unwrap();

        assert_eq!(registry.by_id(plains.id()).unwrap().key, plains.key);
    }
}
