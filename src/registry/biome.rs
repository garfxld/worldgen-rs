use std::sync::OnceLock;

use crate::registry::{REGISTRY, Registry};
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
    pub carvers: &'static [Identifier],
    pub features: &'static [&'static [Identifier]],
    pub id: OnceLock<usize>,
}

impl Biome {
    #[must_use]
    pub fn has_tag(&'static self, tag: &Identifier) -> bool {
        REGISTRY.biomes.is_in_tag(&self.key, tag)
    }

    #[must_use]
    pub fn id(&self) -> usize {
        *self.id.get().expect("biome not registered")
    }
}

pub type BiomeRef = &'static Biome;

pub struct BiomeRegistry {
    registry: Registry<Biome>,
}

impl Default for BiomeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl BiomeRegistry {
    #[must_use]
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

    pub const fn freeze(&mut self) {
        self.registry.freeze();
    }

    #[must_use]
    pub fn by_id(&self, id: usize) -> Option<BiomeRef> {
        self.registry.by_id(id)
    }

    #[must_use]
    pub fn by_key(&self, key: &Identifier) -> Option<BiomeRef> {
        self.registry.by_key(key)
    }

    #[must_use]
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
#[expect(
    clippy::float_cmp,
    reason = "temperature is an exact f32 literal parsed from vanilla JSON, not a computed value"
)]
mod tests {
    use super::*;
    use crate::registry::vanilla_biomes;

    fn create_test_registry() -> BiomeRegistry {
        let mut registry = BiomeRegistry::new();
        vanilla_biomes::register_biomes(&mut registry);
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
        assert!(
            plains
                .carvers
                .contains(&Identifier::const_vanilla("cave"))
        );
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
            &[Identifier::const_vanilla("nether_cave")]
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
