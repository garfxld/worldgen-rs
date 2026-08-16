use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum DoubleList {
    Array(Arc<[f64]>),
    CubePointRange(usize),
    Offset {
        delegate: Arc<DoubleList>,
        offset: f64,
    },
    NonOverlapping {
        lower: Arc<DoubleList>,
        upper: Arc<DoubleList>,
    },
}

impl DoubleList {
    pub fn from_slice(values: &[f64]) -> Self {
        Self::Array(values.into())
    }

    pub const fn cube_point_range(parts: usize) -> Self {
        assert!(parts > 0, "Need at least 1 part");
        Self::CubePointRange(parts)
    }

    pub fn size(&self) -> usize {
        match self {
            Self::Array(values) => values.len(),
            Self::CubePointRange(parts) => parts + 1,
            Self::Offset { delegate, .. } => delegate.size(),
            Self::NonOverlapping { lower, upper } => lower.size() + upper.size(),
        }
    }

    pub fn get(&self, index: usize) -> f64 {
        match self {
            Self::Array(values) => values[index],
            Self::CubePointRange(parts) => index as f64 / *parts as f64,
            Self::Offset { delegate, offset } => delegate.get(index) + offset,
            Self::NonOverlapping { lower, upper } => {
                if index < lower.size() {
                    lower.get(index)
                } else {
                    upper.get(index - lower.size())
                }
            }
        }
    }

    pub const fn is_cube_point_range(&self) -> bool {
        matches!(self, Self::CubePointRange(_))
    }
}

impl PartialEq for DoubleList {
    fn eq(&self, other: &Self) -> bool {
        if self.size() != other.size() {
            return false;
        }
        (0..self.size()).all(|index| self.get(index) == other.get(index))
    }
}
