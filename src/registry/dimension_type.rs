use crate::registry::Registry;
use crate::util::ident::Identifier;

#[derive(Debug, Clone)]
pub struct DimensionType {
    pub key: Identifier,
    pub min_y: i32,
    pub height: i32,
    pub has_skylight: bool,
    pub ambient_light: f32,
    pub fast_lava: bool,
}

pub type DimensionTypeRef = &'static DimensionType;
pub type DimensionTypeRegistry = Registry<DimensionType>;

pub mod vanilla_dimension_types {
    use super::{DimensionType, DimensionTypeRegistry};
    use crate::util::ident::Identifier;

    pub static OVERWORLD: DimensionType = DimensionType {
        key: Identifier::const_vanilla("overworld"),
        min_y: -64,
        height: 384,
        has_skylight: true,
        ambient_light: 0.0,
        fast_lava: false,
    };

    pub static OVERWORLD_CAVES: DimensionType = DimensionType {
        key: Identifier::const_vanilla("overworld_caves"),
        min_y: -64,
        height: 384,
        has_skylight: true,
        ambient_light: 0.0,
        fast_lava: false,
    };

    pub static THE_END: DimensionType = DimensionType {
        key: Identifier::const_vanilla("the_end"),
        min_y: 0,
        height: 256,
        has_skylight: true,
        ambient_light: 0.25,
        fast_lava: false,
    };

    pub static THE_NETHER: DimensionType = DimensionType {
        key: Identifier::const_vanilla("the_nether"),
        min_y: 0,
        height: 256,
        has_skylight: false,
        ambient_light: 0.1,
        fast_lava: true,
    };

    pub fn register_dimension_types(registry: &mut DimensionTypeRegistry) {
        registry.register(OVERWORLD.key.clone(), &OVERWORLD);
        registry.register(OVERWORLD_CAVES.key.clone(), &OVERWORLD_CAVES);
        registry.register(THE_END.key.clone(), &THE_END);
        registry.register(THE_NETHER.key.clone(), &THE_NETHER);
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "dimension type registry tests assert required vanilla properties are present"
)]
mod tests {
    use super::vanilla_dimension_types;
    use super::{DimensionTypeRegistry, Identifier};

    fn create_test_registry() -> DimensionTypeRegistry {
        let mut registry = DimensionTypeRegistry::new();
        vanilla_dimension_types::register_dimension_types(&mut registry);
        registry.freeze();
        registry
    }

    #[test]
    fn overworld_build_bounds_match_vanilla() {
        let registry = create_test_registry();
        let overworld = registry
            .by_key(&Identifier::const_vanilla("overworld"))
            .unwrap();

        assert_eq!(overworld.min_y, -64);
        assert_eq!(overworld.height, 384);
        assert!(overworld.has_skylight);
    }

    #[test]
    fn nether_has_no_skylight_and_fast_lava() {
        let registry = create_test_registry();
        let nether = registry
            .by_key(&Identifier::const_vanilla("the_nether"))
            .unwrap();

        assert!(!nether.has_skylight);
        assert!(nether.fast_lava);
    }
}
