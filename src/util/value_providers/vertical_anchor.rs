use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerticalAnchor {
    Absolute(i32),
    AboveBottom(i32),
    BelowTop(i32),
}

impl VerticalAnchor {
    pub const fn resolve_y(self, min_y: i32, height: i32) -> i32 {
        match self {
            Self::Absolute(y) => y,
            Self::AboveBottom(offset) => min_y + offset,
            Self::BelowTop(offset) => min_y + height - 1 - offset,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_absolute_ignoring_bounds() {
        assert_eq!(VerticalAnchor::Absolute(64).resolve_y(-64, 384), 64);
    }

    #[test]
    fn resolves_above_bottom_relative_to_min_y() {
        assert_eq!(VerticalAnchor::AboveBottom(10).resolve_y(-64, 384), -54);
    }

    #[test]
    fn resolves_below_top_relative_to_max_y() {
        assert_eq!(VerticalAnchor::BelowTop(0).resolve_y(-64, 384), 319);
        assert_eq!(VerticalAnchor::BelowTop(5).resolve_y(-64, 384), 314);
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
