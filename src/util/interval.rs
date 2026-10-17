#[derive(Debug, Clone, Copy)]
pub struct Interval {
    min: f32,
    max: f32,
}

pub fn java_min(a: f32, b: f32) -> f32 {
    if a.is_nan() {
        return a;
    }
    if a == 0.0 && b == 0.0 && b.to_bits() == (-0.0f32).to_bits() {
        return b;
    }
    if a <= b { a } else { b }
}

pub fn java_max(a: f32, b: f32) -> f32 {
    if a.is_nan() {
        return a;
    }
    if a == 0.0 && b == 0.0 && a.to_bits() == (-0.0f32).to_bits() {
        return b;
    }
    if a >= b { a } else { b }
}

pub fn java_signum(v: f32) -> f32 {
    if v == 0.0 || v.is_nan() {
        v
    } else {
        1.0f32.copysign(v)
    }
}

fn mul_bound(left: f32, right: f32) -> f32 {
    if left != 0.0 && right != 0.0 {
        left * right
    } else {
        0.0
    }
}

impl PartialEq for Interval {
    fn eq(&self, other: &Self) -> bool {
        (self.is_nai() && other.is_nai()) || (self.min == other.min && self.max == other.max)
    }
}

impl Interval {
    pub const NAI: Self = Self {
        min: f32::NAN,
        max: f32::NAN,
    };
    pub const INFINITE: Self = Self {
        min: f32::NEG_INFINITY,
        max: f32::INFINITY,
    };

    pub fn of(min: f32, max: f32) -> Self {
        assert!(
            !min.is_nan() && !max.is_nan(),
            "Bounds cannot include NaN [{min}; {max}]"
        );
        assert!(max >= min, "max ({max}) < min ({min})");
        Self { min, max }
    }

    pub fn of_symmetric(range: f32) -> Self {
        Self::of(-range, range)
    }

    pub fn of_exact(value: f32) -> Self {
        Self::of(value, value)
    }

    pub const fn min(self) -> f32 {
        self.min
    }

    pub const fn max(self) -> f32 {
        self.max
    }

    pub const fn is_nai(self) -> bool {
        self.min.is_nan()
    }

    pub fn contains(self, value: f32) -> bool {
        value >= self.min && value <= self.max
    }

    pub fn intersects(self, other: Self) -> bool {
        self.min <= other.max && self.max >= other.min
    }

    pub fn encapsulating_all(intervals: impl IntoIterator<Item = Self>) -> Self {
        let mut min = f32::INFINITY;
        let mut max = f32::NEG_INFINITY;
        let mut any = false;
        for interval in intervals {
            any = true;
            if !interval.is_nai() {
                min = java_min(interval.min, min);
                max = java_max(interval.max, max);
            }
        }
        assert!(any, "At least one interval required");
        if max < min {
            Self::NAI
        } else {
            Self::of(min, max)
        }
    }

    pub fn encapsulating(first: f32, second: f32) -> Self {
        if first.is_nan() && second.is_nan() {
            Self::NAI
        } else if first.is_nan() {
            Self::of_exact(second)
        } else if second.is_nan() {
            Self::of_exact(first)
        } else {
            Self::of(java_min(first, second), java_max(first, second))
        }
    }

    fn encapsulating_value(first: Self, second: f32) -> Self {
        if second.is_nan() {
            first
        } else if first.is_nai() {
            Self::of_exact(second)
        } else {
            Self::of(java_min(first.min, second), java_max(first.max, second))
        }
    }

    #[expect(
        clippy::should_implement_trait,
        reason = "called as `Interval::add(a, b)`; NaN bounds widen to infinite"
    )]
    pub fn add(left: Self, right: Self) -> Self {
        let min = left.min + right.min;
        let max = left.max + right.max;
        if !min.is_nan() && !max.is_nan() {
            Self::of(min, max)
        } else {
            Self::NAI
        }
    }

    #[expect(
        clippy::should_implement_trait,
        reason = "called as `Interval::sub(a, b)`; NaN bounds widen to infinite"
    )]
    pub fn sub(left: Self, right: Self) -> Self {
        let min = left.min - right.max;
        let max = left.max - right.min;
        if !min.is_nan() && !max.is_nan() {
            Self::of(min, max)
        } else {
            Self::NAI
        }
    }

    #[expect(
        clippy::should_implement_trait,
        reason = "called as `Interval::mul(a, b)`; NaN bounds widen to infinite"
    )]
    pub fn mul(left: Self, right: Self) -> Self {
        if left.is_nai() || right.is_nai() {
            return Self::NAI;
        }
        let min_min = mul_bound(left.min, right.min);
        let min_max = mul_bound(left.min, right.max);
        let max_min = mul_bound(left.max, right.min);
        let max_max = mul_bound(left.max, right.max);
        Self::of(
            java_min(java_min(min_min, min_max), java_min(max_min, max_max)),
            java_max(java_max(min_min, min_max), java_max(max_min, max_max)),
        )
    }

    pub fn reciprocal(input: Self) -> Self {
        if input.is_nai() || (input.min == 0.0 && input.max == 0.0) {
            return Self::NAI;
        }
        if !input.contains(0.0) {
            Self::of(1.0 / input.max, 1.0 / input.min)
        } else if input.max == 0.0 {
            Self::of(f32::NEG_INFINITY, 1.0 / input.min)
        } else if input.min == 0.0 {
            Self::of(1.0 / input.max, f32::INFINITY)
        } else {
            Self::INFINITE
        }
    }

    #[expect(
        clippy::should_implement_trait,
        reason = "called as `Interval::div(a, b)`; NaN bounds widen to infinite"
    )]
    pub fn div(left: Self, right: Self) -> Self {
        Self::mul(left, Self::reciprocal(right))
    }

    pub fn min_of(left: Self, right: Self) -> Self {
        if left.is_nai() || right.is_nai() {
            Self::NAI
        } else {
            Self::of(java_min(left.min, right.min), java_min(left.max, right.max))
        }
    }

    pub fn max_of(left: Self, right: Self) -> Self {
        if left.is_nai() || right.is_nai() {
            Self::NAI
        } else {
            Self::of(java_max(left.min, right.min), java_max(left.max, right.max))
        }
    }

    pub fn clamp(input: Self, min: f32, max: f32) -> Self {
        assert!(min <= max, "min ({min}) > max ({max})");
        if input.is_nai() {
            Self::NAI
        } else if input.min >= max {
            Self::of(max, max)
        } else if input.max <= min {
            Self::of(min, min)
        } else {
            Self::of(java_max(input.min, min), java_min(input.max, max))
        }
    }

    pub fn abs(input: Self) -> Self {
        if input.is_nai() {
            return Self::NAI;
        }
        let max = java_max(input.min.abs(), input.max.abs());
        if input.contains(0.0) {
            Self::of(0.0, max)
        } else {
            Self::of(java_min(input.min.abs(), input.max.abs()), max)
        }
    }

    pub fn square(input: Self) -> Self {
        if input.is_nai() {
            return Self::NAI;
        }
        let a = input.min * input.min;
        let b = input.max * input.max;
        let max = java_max(a, b);
        if input.contains(0.0) {
            Self::of(0.0, max)
        } else {
            Self::of(java_min(a, b), max)
        }
    }

    pub fn pow(base: Self, exponent: Self) -> Self {
        if base.is_nai() || exponent.is_nai() {
            return Self::NAI;
        }
        if base.min == base.max {
            return Self::pow_value(base.min, exponent);
        }
        let mut result = Self::encapsulating_all([
            Self::pow_value(base.min, exponent),
            Self::pow_value(base.max, exponent),
        ]);
        if base.contains(0.0) {
            if base.max > 0.0 {
                result = Self::encapsulating_all([result, Self::pow_value(0.0, exponent)]);
            }
            if base.min < 0.0 {
                result = Self::encapsulating_all([result, Self::pow_value(-0.0, exponent)]);
            }
        }
        result
    }

    fn pow_value(base: f32, exponent: Self) -> Self {
        if base.is_nan() || exponent.is_nai() {
            return Self::NAI;
        }
        if exponent.min == exponent.max {
            let value = f64::from(base).powf(f64::from(exponent.min)) as f32;
            return if value.is_nan() {
                Self::NAI
            } else {
                Self::of_exact(value)
            };
        }
        if base == 0.0 {
            Self::mul(
                Self::pow_zero_base(exponent),
                Self::of_exact(1.0f32.copysign(base)),
            )
        } else if base == 1.0 {
            Self::of_exact(1.0)
        } else if base > 0.0 {
            Self::pow_positive_base(base, exponent)
        } else {
            Self::pow_negative_base(base, exponent)
        }
    }

    fn pow_positive_base(base: f32, exponent: Self) -> Self {
        if exponent.min.is_finite() && exponent.max.is_finite() {
            Self::encapsulating(
                java_pow(base, exponent.min) as f32,
                java_pow(base, exponent.max) as f32,
            )
        } else {
            Self::pow_infinite_exponent(base, exponent)
        }
    }

    fn pow_zero_base(exponent: Self) -> Self {
        if exponent.contains(0.0) {
            if exponent.max == 0.0 {
                Self::of(1.0, f32::INFINITY)
            } else if exponent.min == 0.0 {
                Self::of(0.0, 1.0)
            } else {
                Self::of(0.0, f32::INFINITY)
            }
        } else if exponent.max < 0.0 {
            Self::of_exact(f32::INFINITY)
        } else {
            Self::of_exact(0.0)
        }
    }

    fn pow_infinite_exponent(base: f32, exponent: Self) -> Self {
        if exponent.min.is_infinite() && exponent.max.is_infinite() {
            Self::of(0.0, f32::INFINITY)
        } else if exponent.min.is_infinite() {
            if base < 1.0 {
                Self::of(java_pow(base, exponent.max) as f32, f32::INFINITY)
            } else {
                Self::of(0.0, java_pow(base, exponent.max) as f32)
            }
        } else if base < 1.0 {
            Self::of(0.0, java_pow(base, exponent.min) as f32)
        } else {
            Self::of(java_pow(base, exponent.min) as f32, f32::INFINITY)
        }
    }

    fn pow_negative_base(base: f32, exponent: Self) -> Self {
        let exponent_min_int = f64::from(exponent.min).ceil() as f32;
        let exponent_max_int = f64::from(exponent.max).floor() as f32;
        if exponent_max_int < exponent_min_int {
            return Self::NAI;
        }
        let base_to_min_int = java_pow(base, exponent_min_int) as f32;
        let base_to_max_int = java_pow(base, exponent_max_int) as f32;
        let mut result = Self::encapsulating(base_to_min_int, base_to_max_int);
        if exponent_min_int.is_infinite() {
            result = Self::encapsulating_value(result, -base_to_min_int);
        } else if exponent_min_int + 1.0 < exponent_max_int {
            result =
                Self::encapsulating_value(result, java_pow(base, exponent_min_int + 1.0) as f32);
        }
        if exponent_max_int.is_infinite() {
            result = Self::encapsulating_value(result, -base_to_max_int);
        } else if exponent_max_int - 1.0 > exponent_min_int {
            result =
                Self::encapsulating_value(result, java_pow(base, exponent_max_int - 1.0) as f32);
        }
        result
    }

    pub fn log(input: Self) -> Self {
        if input.max < 0.0 {
            return Self::NAI;
        }
        let clipped = Self::max_of(input, Self::of_exact(0.0));
        Self::map_monotonic(clipped, |x| f64::from(x).ln() as f32)
    }

    pub fn map_monotonic(input: Self, op: impl Fn(f32) -> f32) -> Self {
        if input.is_nai() {
            return Self::NAI;
        }
        let mapped_min = op(input.min);
        let mapped_max = op(input.max);
        assert!(
            !mapped_min.is_nan() && !mapped_max.is_nan(),
            "Monotonic operator should not produce NaN"
        );
        Self::of(
            java_min(mapped_min, mapped_max),
            java_max(mapped_min, mapped_max),
        )
    }

    pub fn lerp(alpha: Self, first: Self, second: Self) -> Self {
        if alpha.is_nai() || first.is_nai() || second.is_nai() {
            return Self::NAI;
        }
        Self::encapsulating_all([
            Self::lerp_values(alpha, first.min, second.min),
            Self::lerp_values(alpha, first.max, second.min),
            Self::lerp_values(alpha, first.min, second.max),
            Self::lerp_values(alpha, first.max, second.max),
        ])
    }

    pub fn lerp_values(alpha: Self, first: f32, second: f32) -> Self {
        if alpha.is_nai() || first.is_nan() || second.is_nan() {
            return Self::NAI;
        }
        if first.is_finite() && second.is_finite() {
            Self::encapsulating(
                first + mul_bound(alpha.min, second - first),
                first + mul_bound(alpha.max, second - first),
            )
        } else {
            if first == second {
                return Self::of_exact(first);
            }
            let new_min = lerp_infinite_bound(alpha.min, first, second);
            let new_max = lerp_infinite_bound(alpha.max, first, second);
            if !new_min.is_nan() && !new_max.is_nan() {
                Self::encapsulating(new_min, new_max)
            } else {
                Self::NAI
            }
        }
    }

    pub fn sign(input: Self) -> Self {
        if input.is_nai() {
            return Self::NAI;
        }
        if input.min == input.max {
            return Self::of_exact(java_signum(input.min));
        }
        if input.contains(0.0) {
            if input.min == 0.0 {
                Self::of(0.0, 1.0)
            } else if input.max == 0.0 {
                Self::of(-1.0, 0.0)
            } else {
                Self::of(-1.0, 1.0)
            }
        } else {
            Self::of_exact(if input.min > 0.0 { 1.0 } else { -1.0 })
        }
    }
}

fn lerp_infinite_bound(alpha: f32, first: f32, second: f32) -> f32 {
    let first_part = mul_bound(1.0 - alpha, first);
    let second_part = mul_bound(alpha, second);
    if !first_part.is_infinite() || !second_part.is_infinite() {
        first_part + second_part
    } else if alpha <= 0.0 {
        if second > first {
            f32::NEG_INFINITY
        } else {
            f32::INFINITY
        }
    } else if alpha >= 1.0 {
        if second > first {
            f32::INFINITY
        } else {
            f32::NEG_INFINITY
        }
    } else {
        f32::NAN
    }
}

pub fn java_pow(base: f32, exponent: f32) -> f64 {
    f64::from(base).powf(f64::from(exponent))
}
