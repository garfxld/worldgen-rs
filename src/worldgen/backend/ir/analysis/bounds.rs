use crate::worldgen::backend::ir::DensityProgram;
use crate::worldgen::backend::ir::types::{DensityFunction, MappedType, TwoArgType};

pub fn compute_bounds(df: &DensityFunction, program: &DensityProgram) -> (f64, f64) {
    let mut visiting = Vec::new();
    bounds_of(df, program, &mut visiting)
}

fn bounds_of(df: &DensityFunction, program: &DensityProgram, visiting: &mut Vec<String>) -> (f64, f64) {
    let unbounded = (f64::NEG_INFINITY, f64::INFINITY);

    match df {
        DensityFunction::Constant(c) => (c.value, c.value),

        DensityFunction::Reference(r) => {
            if visiting.contains(&r.id) {
                return unbounded;
            }
            let Some(target) = program.registry.get(&r.id) else {
                return unbounded;
            };
            visiting.push(r.id.clone());
            let result = bounds_of(target, program, visiting);
            visiting.pop();
            result
        }

        DensityFunction::YClampedGradient(g) => {
            (g.from_value.min(g.to_value), g.from_value.max(g.to_value))
        }

        DensityFunction::Noise(_)
        | DensityFunction::ShiftedNoise(_)
        | DensityFunction::ShiftA(_)
        | DensityFunction::ShiftB(_)
        | DensityFunction::Shift(_)
        | DensityFunction::Spline(_)
        | DensityFunction::BlendedNoise(_) => unbounded,

        DensityFunction::TwoArgumentSimple(t) => {
            let lhs = bounds_of(&t.argument1, program, visiting);
            let rhs = bounds_of(&t.argument2, program, visiting);
            two_arg_bounds(t.op, lhs, rhs)
        }

        DensityFunction::Mapped(m) => {
            let input = bounds_of(&m.input, program, visiting);
            mapped_bounds(m.op, input)
        }

        DensityFunction::Clamp(c) => (c.min, c.max),

        DensityFunction::RangeChoice(rc) => {
            let (in_lo, in_hi) = bounds_of(&rc.when_in_range, program, visiting);
            let (out_lo, out_hi) = bounds_of(&rc.when_out_of_range, program, visiting);
            (in_lo.min(out_lo), in_hi.max(out_hi))
        }

        DensityFunction::IntervalSelect(interval) => {
            let mut lo = f64::INFINITY;
            let mut hi = f64::NEG_INFINITY;
            for function in &interval.functions {
                let (branch_lo, branch_hi) = bounds_of(function, program, visiting);
                lo = lo.min(branch_lo);
                hi = hi.max(branch_hi);
            }
            if lo > hi { unbounded } else { (lo, hi) }
        }

        DensityFunction::WeirdScaledSampler(_) => (0.0, f64::INFINITY),

        DensityFunction::EndIslands => (-100.0, 80.0),

        DensityFunction::BlendAlpha(_) => (1.0, 1.0),
        DensityFunction::BlendOffset(_) => (0.0, 0.0),
        DensityFunction::BlendDensity(bd) => bounds_of(&bd.input, program, visiting),

        DensityFunction::Marker(m) => bounds_of(&m.wrapped, program, visiting),

        DensityFunction::FindTopSurface(fts) => {
            let (_, upper) = bounds_of(&fts.upper_bound, program, visiting);
            (f64::from(fts.lower_bound), upper)
        }
    }
}

fn two_arg_bounds(op: TwoArgType, lhs: (f64, f64), rhs: (f64, f64)) -> (f64, f64) {
    let (a_lo, a_hi) = lhs;
    let (b_lo, b_hi) = rhs;
    match op {
        TwoArgType::Add => (a_lo + b_lo, a_hi + b_hi),
        TwoArgType::Mul => {
            let corners = [a_lo * b_lo, a_lo * b_hi, a_hi * b_lo, a_hi * b_hi];
            if corners.iter().any(|c| c.is_nan()) {
                return (f64::NEG_INFINITY, f64::INFINITY);
            }
            let lo = corners.iter().copied().fold(f64::INFINITY, f64::min);
            let hi = corners.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            (lo, hi)
        }
        TwoArgType::Min => (a_lo.min(b_lo), a_hi.min(b_hi)),
        TwoArgType::Max => (a_lo.max(b_lo), a_hi.max(b_hi)),
    }
}

fn mapped_bounds(op: MappedType, input: (f64, f64)) -> (f64, f64) {
    let (lo, hi) = input;
    match op {
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
        MappedType::HalfNegative => (half_negative(lo), half_negative(hi)),
        MappedType::QuarterNegative => (quarter_negative(lo), quarter_negative(hi)),
        MappedType::Invert => {
            if lo > 0.0 || hi < 0.0 {
                let a = 1.0 / lo;
                let b = 1.0 / hi;
                (a.min(b), a.max(b))
            } else {
                (f64::NEG_INFINITY, f64::INFINITY)
            }
        }
        MappedType::Squeeze => (squeeze(lo.clamp(-1.0, 1.0)), squeeze(hi.clamp(-1.0, 1.0))),
    }
}

fn half_negative(v: f64) -> f64 {
    if v > 0.0 { v } else { v * 0.5 }
}

fn quarter_negative(v: f64) -> f64 {
    if v > 0.0 { v } else { v * 0.25 }
}

fn squeeze(c: f64) -> f64 {
    c / 2.0 - c * c * c / 24.0
}
