use crate::worldgen::backend::ir::DensityProgram;
use crate::worldgen::backend::ir::types::{DensityFunction, MappedType, TwoArgType};

#[must_use]
pub fn compute_bounds(df: &DensityFunction, input: &DensityProgram) -> (f64, f64) {
    compute_bounds_inner(df, input, &mut Vec::new())
}

#[expect(
    clippy::too_many_lines,
    reason = "one match arm per DensityFunction variant; splitting the dispatch would obscure the per-variant bounds analysis"
)]
pub(crate) fn compute_bounds_inner(
    df: &DensityFunction,
    input: &DensityProgram,
    visiting: &mut Vec<String>,
) -> (f64, f64) {
    match df {
        DensityFunction::Constant(c) => (c.value, c.value),

        DensityFunction::Reference(r) => {
            if visiting.iter().any(|n| n == &r.id) {
                return (f64::NEG_INFINITY, f64::INFINITY);
            }
            let Some(target) = input.registry.get(&r.id) else {
                return (f64::NEG_INFINITY, f64::INFINITY);
            };
            visiting.push(r.id.clone());
            let bounds = compute_bounds_inner(target, input, visiting);
            visiting.pop();
            bounds
        }

        DensityFunction::YClampedGradient(g) => {
            let lo = g.from_value.min(g.to_value);
            let hi = g.from_value.max(g.to_value);
            (lo, hi)
        }

        DensityFunction::Noise(_)
        | DensityFunction::ShiftedNoise(_)
        | DensityFunction::ShiftA(_)
        | DensityFunction::ShiftB(_)
        | DensityFunction::Shift(_)
        | DensityFunction::Spline(_)
        | DensityFunction::BlendedNoise(_) => (f64::NEG_INFINITY, f64::INFINITY),

        DensityFunction::TwoArgumentSimple(t) => {
            let (a_lo, a_hi) = compute_bounds_inner(&t.argument1, input, visiting);
            let (b_lo, b_hi) = compute_bounds_inner(&t.argument2, input, visiting);
            match t.op {
                TwoArgType::Add => (a_lo + b_lo, a_hi + b_hi),
                TwoArgType::Mul => {
                    let candidates = [a_lo * b_lo, a_lo * b_hi, a_hi * b_lo, a_hi * b_hi];
                    let mut lo = f64::INFINITY;
                    let mut hi = f64::NEG_INFINITY;
                    for c in candidates {
                        if c.is_nan() {
                            return (f64::NEG_INFINITY, f64::INFINITY);
                        }
                        if c < lo {
                            lo = c;
                        }
                        if c > hi {
                            hi = c;
                        }
                    }
                    (lo, hi)
                }
                TwoArgType::Min => (a_lo.min(b_lo), a_hi.min(b_hi)),
                TwoArgType::Max => (a_lo.max(b_lo), a_hi.max(b_hi)),
            }
        }

        DensityFunction::Mapped(m) => {
            let (lo, hi) = compute_bounds_inner(&m.input, input, visiting);
            match m.op {
                MappedType::Abs => {
                    if lo >= 0.0 {
                        (lo, hi)
                    } else if hi <= 0.0 {
                        (-hi, -lo)
                    } else {
                        (0.0, lo.abs().max(hi.abs()))
                    }
                }
                MappedType::Square => {
                    if lo >= 0.0 {
                        (lo * lo, hi * hi)
                    } else if hi <= 0.0 {
                        (hi * hi, lo * lo)
                    } else {
                        (0.0, (lo * lo).max(hi * hi))
                    }
                }
                MappedType::Cube => (lo * lo * lo, hi * hi * hi),
                MappedType::HalfNegative => {
                    let map = |v: f64| if v > 0.0 { v } else { v * 0.5 };
                    (map(lo), map(hi))
                }
                MappedType::QuarterNegative => {
                    let map = |v: f64| if v > 0.0 { v } else { v * 0.25 };
                    (map(lo), map(hi))
                }
                MappedType::Invert => {
                    if lo > 0.0 || hi < 0.0 {
                        let a = 1.0 / lo;
                        let b = 1.0 / hi;
                        (a.min(b), a.max(b))
                    } else {
                        (f64::NEG_INFINITY, f64::INFINITY)
                    }
                }
                MappedType::Squeeze => {
                    let map = |v: f64| {
                        let c = v.clamp(-1.0, 1.0);
                        c / 2.0 - c * c * c / 24.0
                    };
                    let lo_c = lo.clamp(-1.0, 1.0);
                    let hi_c = hi.clamp(-1.0, 1.0);
                    (map(lo_c), map(hi_c))
                }
            }
        }

        DensityFunction::Clamp(c) => (c.min, c.max),

        DensityFunction::RangeChoice(rc) => {
            let (in_lo, in_hi) = compute_bounds_inner(&rc.when_in_range, input, visiting);
            let (out_lo, out_hi) = compute_bounds_inner(&rc.when_out_of_range, input, visiting);
            (in_lo.min(out_lo), in_hi.max(out_hi))
        }

        DensityFunction::IntervalSelect(interval) => {
            let mut lo = f64::INFINITY;
            let mut hi = f64::NEG_INFINITY;
            for function in &interval.functions {
                let (function_lo, function_hi) = compute_bounds_inner(function, input, visiting);
                lo = lo.min(function_lo);
                hi = hi.max(function_hi);
            }
            if lo > hi {
                (f64::NEG_INFINITY, f64::INFINITY)
            } else {
                (lo, hi)
            }
        }

        DensityFunction::WeirdScaledSampler(_) => (0.0, f64::INFINITY),

        DensityFunction::EndIslands => (-100.0, 80.0),

        DensityFunction::BlendAlpha(_) => (1.0, 1.0),
        DensityFunction::BlendOffset(_) => (0.0, 0.0),
        DensityFunction::BlendDensity(bd) => compute_bounds_inner(&bd.input, input, visiting),

        DensityFunction::Marker(m) => compute_bounds_inner(&m.wrapped, input, visiting),

        DensityFunction::FindTopSurface(fts) => {
            let (_, upper) = compute_bounds_inner(&fts.upper_bound, input, visiting);
            (f64::from(fts.lower_bound), upper)
        }
    }
}
