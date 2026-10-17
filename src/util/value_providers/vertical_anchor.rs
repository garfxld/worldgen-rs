use serde::Deserialize;

use crate::world::worldgen::worldgen_context::WorldGenContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerticalAnchor {
    Absolute(i32),
    AboveBottom(i32),
    BelowTop(i32),
    RelativeToSeaLevel(i32),
}

impl VerticalAnchor {
    pub const fn resolve_y(self, context: &WorldGenContext) -> i32 {
        match self {
            Self::Absolute(y) => y,
            Self::AboveBottom(offset) => context.get_min_gen_y() + offset,
            Self::BelowTop(offset) => {
                context.get_gen_depth() - 1 + context.get_min_gen_y() - offset
            }
            Self::RelativeToSeaLevel(offset) => context.sea_level() + offset,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONTEXT: WorldGenContext = WorldGenContext::new(-64, 384, 63, -64, 384);

    #[test]
    fn resolves_absolute_ignoring_bounds() {
        assert_eq!(VerticalAnchor::Absolute(64).resolve_y(&CONTEXT), 64);
    }

    #[test]
    fn resolves_above_bottom_relative_to_min_y() {
        assert_eq!(VerticalAnchor::AboveBottom(10).resolve_y(&CONTEXT), -54);
    }

    #[test]
    fn resolves_below_top_relative_to_max_y() {
        assert_eq!(VerticalAnchor::BelowTop(0).resolve_y(&CONTEXT), 319);
        assert_eq!(VerticalAnchor::BelowTop(5).resolve_y(&CONTEXT), 314);
    }

    #[test]
    fn deserializes_each_variant_from_json() {
        let absolute: VerticalAnchor =
            serde_json::from_str(r#"{"absolute": 100}"#).expect("test value");
        assert_eq!(absolute, VerticalAnchor::Absolute(100));

        let above: VerticalAnchor =
            serde_json::from_str(r#"{"above_bottom": 5}"#).expect("test value");
        assert_eq!(above, VerticalAnchor::AboveBottom(5));

        let below: VerticalAnchor =
            serde_json::from_str(r#"{"below_top": 1}"#).expect("test value");
        assert_eq!(below, VerticalAnchor::BelowTop(1));

        let sea: VerticalAnchor =
            serde_json::from_str(r#"{"relative_to_sea_level": -3}"#).expect("test value");
        assert_eq!(sea, VerticalAnchor::RelativeToSeaLevel(-3));
        assert_eq!(sea.resolve_y(&CONTEXT), 60);
    }

    #[test]
    fn rejects_ambiguous_or_empty_json() {
        assert!(serde_json::from_str::<VerticalAnchor>("{}").is_err());
        assert!(
            serde_json::from_str::<VerticalAnchor>(r#"{"absolute": 1, "above_bottom": 2}"#)
                .is_err()
        );
    }
}
