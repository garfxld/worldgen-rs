use std::sync::Arc;

use crate::util::shapes::double_list::DoubleList;

pub enum IndexMerger {
    Identical {
        coords: DoubleList,
    },
    DiscreteCube {
        result: DoubleList,
        first_div: usize,
        second_div: usize,
    },
    NonOverlapping {
        lower: Arc<DoubleList>,
        upper: Arc<DoubleList>,
        swap: bool,
    },
    Indirect {
        result: Vec<f64>,
        first_indices: Vec<i32>,
        second_indices: Vec<i32>,
        result_length: usize,
    },
}

const fn gcd(a: usize, b: usize) -> usize {
    if b == 0 { a } else { gcd(b, a % b) }
}

pub const fn lcm(first: usize, second: usize) -> u64 {
    first as u64 * (second / gcd(first, second)) as u64
}

impl IndexMerger {
    pub const fn discrete_cube(first_size: usize, second_size: usize) -> Self {
        let gcd = gcd(first_size, second_size);
        Self::DiscreteCube {
            result: DoubleList::cube_point_range(lcm(first_size, second_size) as usize),
            first_div: first_size / gcd,
            second_div: second_size / gcd,
        }
    }

    pub fn non_overlapping(lower: DoubleList, upper: DoubleList, swap: bool) -> Self {
        Self::NonOverlapping {
            lower: Arc::new(lower),
            upper: Arc::new(upper),
            swap,
        }
    }

    pub fn indirect(
        first: &DoubleList,
        second: &DoubleList,
        first_only_matters: bool,
        second_only_matters: bool,
    ) -> Self {
        let mut last_value = f64::NAN;
        let first_size = first.size();
        let second_size = second.size();
        let capacity = first_size + second_size;
        let mut result = vec![0.0; capacity];
        let mut first_indices = vec![0; capacity];
        let mut second_indices = vec![0; capacity];
        let can_skip_first = !first_only_matters;
        let can_skip_second = !second_only_matters;
        let mut result_index = 0usize;
        let mut first_index = 0usize;
        let mut second_index = 0usize;

        loop {
            let ran_out_of_first = first_index >= first_size;
            let ran_out_of_second = second_index >= second_size;
            if ran_out_of_first && ran_out_of_second {
                return Self::Indirect {
                    result,
                    first_indices,
                    second_indices,
                    result_length: result_index.max(1),
                };
            }

            let chose_first = !ran_out_of_first
                && (ran_out_of_second
                    || first.get(first_index) < second.get(second_index) + 1.0E-7);
            if chose_first {
                first_index += 1;
                if can_skip_first && (second_index == 0 || ran_out_of_second) {
                    continue;
                }
            } else {
                second_index += 1;
                if can_skip_second && (first_index == 0 || ran_out_of_first) {
                    continue;
                }
            }

            let current_first_index = first_index as i32 - 1;
            let current_second_index = second_index as i32 - 1;
            let next_value = if chose_first {
                first.get(current_first_index as usize)
            } else {
                second.get(current_second_index as usize)
            };

            if last_value.is_nan() || last_value < next_value - 1.0E-7 {
                first_indices[result_index] = current_first_index;
                second_indices[result_index] = current_second_index;
                result[result_index] = next_value;
                result_index += 1;
                last_value = next_value;
            } else {
                first_indices[result_index - 1] = current_first_index;
                second_indices[result_index - 1] = current_second_index;
            }
        }
    }

    pub fn size(&self) -> usize {
        match self {
            Self::Identical { coords } => coords.size(),
            Self::DiscreteCube { result, .. } => result.size(),
            Self::NonOverlapping { lower, upper, .. } => lower.size() + upper.size(),
            Self::Indirect { result_length, .. } => *result_length,
        }
    }

    pub fn get_list(&self) -> DoubleList {
        match self {
            Self::Identical { coords } => coords.clone(),
            Self::DiscreteCube { result, .. } => result.clone(),
            Self::NonOverlapping { lower, upper, .. } => DoubleList::NonOverlapping {
                lower: lower.clone(),
                upper: upper.clone(),
            },
            Self::Indirect {
                result,
                result_length,
                ..
            } => {
                if *result_length <= 1 {
                    DoubleList::from_slice(&[0.0])
                } else {
                    DoubleList::from_slice(&result[..*result_length])
                }
            }
        }
    }

    pub fn for_merged_indexes(&self, consumer: &mut dyn FnMut(i32, i32, i32) -> bool) -> bool {
        match self {
            Self::Identical { coords } => {
                let size = coords.size() - 1;
                for index in 0..size as i32 {
                    if !consumer(index, index, index) {
                        return false;
                    }
                }
                true
            }
            Self::DiscreteCube {
                result,
                first_div,
                second_div,
            } => {
                let size = result.size() - 1;
                for index in 0..size as i32 {
                    if !consumer(index / *second_div as i32, index / *first_div as i32, index) {
                        return false;
                    }
                }
                true
            }
            Self::NonOverlapping { lower, upper, swap } => {
                let lower_size = lower.size() as i32;
                let upper_size = upper.size() as i32 - 1;
                let mut merge = |first_index: i32, second_index: i32, result_index: i32| {
                    if *swap {
                        consumer(second_index, first_index, result_index)
                    } else {
                        consumer(first_index, second_index, result_index)
                    }
                };

                for index in 0..lower_size {
                    if !merge(index, -1, index) {
                        return false;
                    }
                }
                for index in 0..upper_size {
                    if !merge(lower_size - 1, index, lower_size + index) {
                        return false;
                    }
                }
                true
            }
            Self::Indirect {
                first_indices,
                second_indices,
                result_length,
                ..
            } => {
                let length = *result_length - 1;
                for index in 0..length {
                    if !consumer(first_indices[index], second_indices[index], index as i32) {
                        return false;
                    }
                }
                true
            }
        }
    }

    pub const fn is_discrete_cube(&self) -> bool {
        matches!(self, Self::DiscreteCube { .. })
    }
}
