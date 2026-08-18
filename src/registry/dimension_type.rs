use std::path::Path;

use serde::Deserialize;

use crate::datapack::Datapack;
use crate::datapack::{DatapackError, collect_json_files, path_to_id, read_and_parse_normalized};
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

#[derive(Deserialize)]
struct DimensionTypeJson {
    min_y: i32,
    height: i32,
    has_skylight: bool,
    #[serde(default)]
    ambient_light: f32,
    #[serde(default)]
    attributes: serde_json::Map<String, serde_json::Value>,
}

pub fn load_dimension_types(
    registry: &mut DimensionTypeRegistry,
    pack: &Datapack,
) -> Result<(), DatapackError> {
    for (namespace, dir) in pack.namespaced_dirs("dimension_type") {
        load_directory(registry, &namespace, &dir)?;
    }
    Ok(())
}

fn load_directory(
    registry: &mut DimensionTypeRegistry,
    namespace: &str,
    dir: &Path,
) -> Result<(), DatapackError> {
    for file in collect_json_files(dir) {
        let json: DimensionTypeJson = read_and_parse_normalized(&file)?;
        let key: Identifier = path_to_id(namespace, &file, dir)
            .parse()
            .expect("dimension type path should be a valid identifier");
        let fast_lava = json
            .attributes
            .get("minecraft:gameplay/fast_lava")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

        registry.register(
            key.clone(),
            Box::leak(Box::new(DimensionType {
                key,
                min_y: json.min_y,
                height: json.height,
                has_skylight: json.has_skylight,
                ambient_light: json.ambient_light,
                fast_lava,
            })),
        );
    }
    Ok(())
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "dimension type registry tests assert required vanilla properties are present"
)]
mod tests {
    use super::{Datapack, DimensionTypeRegistry, Identifier, load_dimension_types};

    fn create_test_registry() -> DimensionTypeRegistry {
        let mut registry = DimensionTypeRegistry::new();
        load_dimension_types(&mut registry, &Datapack::builtin()).unwrap();
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
