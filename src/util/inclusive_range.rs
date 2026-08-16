use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "RawInclusiveRange")]
pub struct InclusiveRange {
    pub min_inclusive: i32,
    pub max_inclusive: i32,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum RawInclusiveRange {
    Point(i32),
    Bounds([i32; 2]),
    Object {
        min_inclusive: i32,
        max_inclusive: i32,
    },
}

impl TryFrom<RawInclusiveRange> for InclusiveRange {
    type Error = &'static str;

    fn try_from(raw: RawInclusiveRange) -> Result<Self, Self::Error> {
        let (min_inclusive, max_inclusive) = match raw {
            RawInclusiveRange::Point(value) => (value, value),
            RawInclusiveRange::Bounds([min_inclusive, max_inclusive])
            | RawInclusiveRange::Object {
                min_inclusive,
                max_inclusive,
            } => (min_inclusive, max_inclusive),
        };

        if min_inclusive > max_inclusive {
            return Err("min_inclusive must be less than or equal to max_inclusive");
        }

        Ok(Self {
            min_inclusive,
            max_inclusive,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_every_vanilla_interval_form() {
        let point: InclusiveRange = serde_json::from_str("5").expect("test value");
        assert_eq!(point.min_inclusive, 5);
        assert_eq!(point.max_inclusive, 5);

        let bounds: InclusiveRange = serde_json::from_str("[1, 3]").expect("test value");
        assert_eq!(bounds.min_inclusive, 1);
        assert_eq!(bounds.max_inclusive, 3);

        let object: InclusiveRange =
            serde_json::from_str(r#"{"min_inclusive": 2, "max_inclusive": 7}"#)
                .expect("test value");
        assert_eq!(object.min_inclusive, 2);
        assert_eq!(object.max_inclusive, 7);
    }

    #[test]
    fn rejects_inverted_bounds() {
        assert!(serde_json::from_str::<InclusiveRange>("[9, 2]").is_err());
    }
}
