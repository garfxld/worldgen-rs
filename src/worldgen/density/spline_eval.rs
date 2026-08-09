#[inline]
#[must_use]
pub fn find_interval(locations: &[f32], input: f32) -> i32 {
    let mut lo = 0i32;
    let mut hi = locations.len() as i32;
    while lo < hi {
        let mid = i32::midpoint(lo, hi);
        if input < locations[mid as usize] {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    lo - 1
}

#[inline]
#[must_use]
pub fn hermite_interpolate(
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
    d1: f32,
    d2: f32,
    input: f32,
) -> f32 {
    let t = (input - x1) / (x2 - x1);
    let h = x2 - x1;
    let a = d1 * h - (y2 - y1);
    let b = -d2 * h + (y2 - y1);
    let lerp_y = y1 + t * (y2 - y1);
    let lerp_ab = a + t * (b - a);
    lerp_y + t * (1.0 - t) * lerp_ab
}

#[inline]
pub fn evaluate_spline(
    locations: &[f32],
    derivatives: &[f32],
    input: f32,
    value_at: impl Fn(usize) -> f32,
) -> f32 {
    if locations.is_empty() {
        return 0.0;
    }

    let last = locations.len() - 1;
    let start = find_interval(locations, input);

    if start < 0 {
        let value = value_at(0);
        return value + derivatives[0] * (input - locations[0]);
    }

    let start = start as usize;
    if start == last {
        let value = value_at(last);
        return value + derivatives[last] * (input - locations[last]);
    }

    let y1 = value_at(start);
    let y2 = value_at(start + 1);
    hermite_interpolate(
        locations[start],
        locations[start + 1],
        y1,
        y2,
        derivatives[start],
        derivatives[start + 1],
        input,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_interval_before_all() {
        assert_eq!(find_interval(&[0.0, 1.0, 2.0], -1.0), -1);
    }

    #[test]
    fn find_interval_exact_match() {
        assert_eq!(find_interval(&[0.0, 1.0, 2.0], 1.0), 1);
    }

    #[test]
    fn find_interval_between() {
        assert_eq!(find_interval(&[0.0, 1.0, 2.0], 0.5), 0);
    }

    #[test]
    fn find_interval_after_all() {
        assert_eq!(find_interval(&[0.0, 1.0, 2.0], 3.0), 2);
    }

    #[test]
    fn hermite_linear() {
        let result = hermite_interpolate(0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.5);
        assert!((result - 0.5).abs() < 1e-6);
    }

    #[test]
    fn evaluate_spline_extrapolate_before() {
        let locs = [0.0_f32, 1.0];
        let derivs = [2.0_f32, 0.0];
        let result = evaluate_spline(&locs, &derivs, -1.0, |i| [0.0, 1.0][i]);
        assert!((result - (-2.0)).abs() < 1e-6);
    }

    #[test]
    fn evaluate_spline_extrapolate_after() {
        let locs = [0.0_f32, 1.0];
        let derivs = [0.0_f32, 3.0];
        let result = evaluate_spline(&locs, &derivs, 2.0, |i| [0.0, 1.0][i]);
        assert!((result - 4.0).abs() < 1e-6);
    }

    #[test]
    fn evaluate_spline_constant_values() {
        let locs = [0.0_f32, 1.0, 2.0];
        let derivs = [0.0_f32, 0.0, 0.0];
        let vals = [1.0_f32, 1.0, 1.0];
        let result = evaluate_spline(&locs, &derivs, 0.5, |i| vals[i]);
        assert!((result - 1.0).abs() < 1e-6);
    }
}
