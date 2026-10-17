use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::util::interval::{Interval, java_max, java_min};

use super::function::{Df, float_key};

#[derive(Debug)]
pub enum CubicSpline {
    Constant(f32),
    Multipoint {
        coordinate: Df,
        locations: Vec<f32>,
        values: Vec<Arc<CubicSpline>>,
        derivatives: Vec<f32>,
    },
}

impl PartialEq for CubicSpline {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Constant(a), Self::Constant(b)) => float_key(*a) == float_key(*b),
            (
                Self::Multipoint {
                    coordinate: c1,
                    locations: l1,
                    values: v1,
                    derivatives: d1,
                },
                Self::Multipoint {
                    coordinate: c2,
                    locations: l2,
                    values: v2,
                    derivatives: d2,
                },
            ) => {
                c1 == c2
                    && l1.len() == l2.len()
                    && l1
                        .iter()
                        .zip(l2)
                        .all(|(a, b)| float_key(*a) == float_key(*b))
                    && d1.len() == d2.len()
                    && d1
                        .iter()
                        .zip(d2)
                        .all(|(a, b)| float_key(*a) == float_key(*b))
                    && v1 == v2
            }
            _ => false,
        }
    }
}

impl Eq for CubicSpline {}

impl Hash for CubicSpline {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::Constant(v) => float_key(*v).hash(state),
            Self::Multipoint {
                coordinate,
                locations,
                values,
                derivatives,
            } => {
                coordinate.hash(state);
                for l in locations {
                    float_key(*l).hash(state);
                }
                values.hash(state);
                for d in derivatives {
                    float_key(*d).hash(state);
                }
            }
        }
    }
}

pub fn linear_extend(
    input: f32,
    locations: &[f32],
    value: f32,
    derivatives: &[f32],
    index: usize,
) -> f32 {
    let derivative = derivatives[index];
    if derivative == 0.0 {
        value
    } else {
        value + derivative * (input - locations[index])
    }
}

pub fn find_interval_start(locations: &[f32], input: f32) -> i32 {
    let mut low = 0i32;
    let mut size = locations.len() as i32;
    while size > 0 {
        let half = size / 2;
        let middle = low + half;
        if input < locations[middle as usize] {
            size = half;
        } else {
            low = middle + 1;
            size -= half + 1;
        }
    }
    low - 1
}

impl CubicSpline {
    pub fn for_each_coordinate(&self, f: &mut impl FnMut(&Df)) {
        if let Self::Multipoint {
            coordinate, values, ..
        } = self
        {
            f(coordinate);
            for value in values {
                value.for_each_coordinate(f);
            }
        }
    }

    pub fn map_coordinates(self: &Arc<Self>, f: &mut impl FnMut(&Df) -> Df) -> Arc<Self> {
        match &**self {
            Self::Constant(_) => self.clone(),
            Self::Multipoint {
                coordinate,
                locations,
                values,
                derivatives,
            } => {
                let coordinate = f(coordinate);
                let values = values.iter().map(|v| v.map_coordinates(f)).collect();
                Arc::new(Self::Multipoint {
                    coordinate,
                    locations: locations.clone(),
                    values,
                    derivatives: derivatives.clone(),
                })
            }
        }
    }

    pub fn range(&self) -> Interval {
        match self {
            Self::Constant(v) => Interval::of_exact(*v),
            Self::Multipoint {
                coordinate,
                locations,
                values,
                derivatives,
            } => {
                let last_index = locations.len() - 1;
                let mut min_value = f32::INFINITY;
                let mut max_value = f32::NEG_INFINITY;
                let input_range = coordinate.range();
                if input_range.is_nai() {
                    return input_range;
                }
                if input_range.min() < locations[0] {
                    let first = values[0].range();
                    let e1 =
                        linear_extend(input_range.min(), locations, first.min(), derivatives, 0);
                    let e2 =
                        linear_extend(input_range.min(), locations, first.max(), derivatives, 0);
                    min_value = java_min(min_value, java_min(e1, e2));
                    max_value = java_max(max_value, java_max(e1, e2));
                }
                if input_range.max() > locations[last_index] {
                    let last = values[last_index].range();
                    let e1 = linear_extend(
                        input_range.max(),
                        locations,
                        last.min(),
                        derivatives,
                        last_index,
                    );
                    let e2 = linear_extend(
                        input_range.max(),
                        locations,
                        last.max(),
                        derivatives,
                        last_index,
                    );
                    min_value = java_min(min_value, java_min(e1, e2));
                    max_value = java_max(max_value, java_max(e1, e2));
                }
                let value_ranges: Vec<Interval> = values.iter().map(|v| v.range()).collect();
                for range in &value_ranges {
                    min_value = java_min(min_value, range.min());
                    max_value = java_max(max_value, range.max());
                }
                for i in 0..last_index {
                    let x_diff = locations[i + 1] - locations[i];
                    let (min1, max1) = (value_ranges[i].min(), value_ranges[i].max());
                    let (min2, max2) = (value_ranges[i + 1].min(), value_ranges[i + 1].max());
                    let d1 = derivatives[i];
                    let d2 = derivatives[i + 1];
                    if d1 != 0.0 || d2 != 0.0 {
                        let p1 = d1 * x_diff;
                        let p2 = d2 * x_diff;
                        let min_lerp1 = java_min(min1, min2);
                        let max_lerp1 = java_max(max1, max2);
                        let min_a = p1 - max2 + min1;
                        let max_a = p1 - min2 + max1;
                        let min_b = -p2 + min2 - max1;
                        let max_b = -p2 + max2 - min1;
                        let min_lerp2 = java_min(min_a, min_b);
                        let max_lerp2 = java_max(max_a, max_b);
                        min_value = java_min(min_value, min_lerp1 + 0.25 * min_lerp2);
                        max_value = java_max(max_value, max_lerp1 + 0.25 * max_lerp2);
                    }
                }
                Interval::of(min_value, max_value)
            }
        }
    }
}
