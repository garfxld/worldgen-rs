#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BooleanOp {
    False,
    NotOr,
    OnlySecond,
    NotFirst,
    OnlyFirst,
    NotSecond,
    NotSame,
    NotAnd,
    And,
    Same,
    Second,
    Causes,
    First,
    CausedBy,
    Or,
    True,
}

impl BooleanOp {
    pub const fn apply(self, first: bool, second: bool) -> bool {
        match self {
            Self::False => false,
            Self::NotOr => !first && !second,
            Self::OnlySecond => second && !first,
            Self::NotFirst => !first,
            Self::OnlyFirst => first && !second,
            Self::NotSecond => !second,
            Self::NotSame => first != second,
            Self::NotAnd => !first || !second,
            Self::And => first && second,
            Self::Same => first == second,
            Self::Second => second,
            Self::Causes => !first || second,
            Self::First => first,
            Self::CausedBy => first || !second,
            Self::Or => first || second,
            Self::True => true,
        }
    }
}
