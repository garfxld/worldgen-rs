use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::mem;

use rustc_hash::{FxHashMap, FxHasher};

use crate::worldgen::backend::ir::types::DensityFunction;

pub fn collect_inline_flat_noises(df: &DensityFunction, out: &mut BTreeMap<u64, (String, f64)>) {
    if let DensityFunction::Noise(n) = df
        && n.y_scale == 0.0
    {
        let fp = fingerprint(df);
        out.entry(fp)
            .or_insert_with(|| (n.noise_id.clone(), n.xz_scale));
    }
    match df {
        DensityFunction::TwoArgumentSimple(t) => {
            collect_inline_flat_noises(&t.argument1, out);
            collect_inline_flat_noises(&t.argument2, out);
        }
        DensityFunction::Mapped(m) => collect_inline_flat_noises(&m.input, out),
        DensityFunction::Clamp(c) => collect_inline_flat_noises(&c.input, out),
        DensityFunction::RangeChoice(rc) => {
            collect_inline_flat_noises(&rc.input, out);
            collect_inline_flat_noises(&rc.when_in_range, out);
            collect_inline_flat_noises(&rc.when_out_of_range, out);
        }
        DensityFunction::IntervalSelect(interval) => {
            collect_inline_flat_noises(&interval.input, out);
            for function in &interval.functions {
                collect_inline_flat_noises(function, out);
            }
        }
        DensityFunction::WeirdScaledSampler(ws) => collect_inline_flat_noises(&ws.input, out),
        DensityFunction::BlendDensity(bd) => collect_inline_flat_noises(&bd.input, out),
        DensityFunction::Marker(m) => collect_inline_flat_noises(&m.wrapped, out),
        DensityFunction::ShiftedNoise(sn) => {
            collect_inline_flat_noises(&sn.shift_x, out);
            collect_inline_flat_noises(&sn.shift_y, out);
            collect_inline_flat_noises(&sn.shift_z, out);
        }
        _ => {}
    }
}

#[must_use]
pub const fn is_cse_candidate(df: &DensityFunction) -> bool {
    matches!(
        df,
        DensityFunction::Reference(_)
            | DensityFunction::Noise(_)
            | DensityFunction::ShiftedNoise(_)
    )
}

#[must_use]
pub fn collect_expensive_subexprs(df: &DensityFunction) -> FxHashMap<u64, DensityFunction> {
    let mut result = FxHashMap::default();
    collect_expensive_inner(df, &mut result);
    result
}

pub(crate) fn collect_expensive_inner(
    df: &DensityFunction,
    out: &mut FxHashMap<u64, DensityFunction>,
) {
    if is_cse_candidate(df) {
        let fp = fingerprint(df);
        out.entry(fp).or_insert_with(|| df.clone());
    }
    match df {
        DensityFunction::TwoArgumentSimple(t) => {
            collect_expensive_inner(&t.argument1, out);
            collect_expensive_inner(&t.argument2, out);
        }
        DensityFunction::Mapped(m) => collect_expensive_inner(&m.input, out),
        DensityFunction::Clamp(c) => collect_expensive_inner(&c.input, out),
        DensityFunction::RangeChoice(rc) => {
            collect_expensive_inner(&rc.input, out);
            collect_expensive_inner(&rc.when_in_range, out);
            collect_expensive_inner(&rc.when_out_of_range, out);
        }
        DensityFunction::IntervalSelect(interval) => {
            collect_expensive_inner(&interval.input, out);
            for function in &interval.functions {
                collect_expensive_inner(function, out);
            }
        }
        DensityFunction::WeirdScaledSampler(ws) => collect_expensive_inner(&ws.input, out),
        DensityFunction::BlendDensity(bd) => collect_expensive_inner(&bd.input, out),
        DensityFunction::Marker(m) => collect_expensive_inner(&m.wrapped, out),
        _ => {}
    }
}

#[must_use]
pub fn fingerprint(df: &DensityFunction) -> u64 {
    let mut hasher = FxHasher::default();
    hash_df(df, &mut hasher);
    hasher.finish()
}

pub(crate) fn hash_df(df: &DensityFunction, h: &mut impl Hasher) {
    mem::discriminant(df).hash(h);
    match df {
        DensityFunction::Constant(c) => c.value.to_bits().hash(h),
        DensityFunction::Reference(r) => r.id.hash(h),
        DensityFunction::YClampedGradient(g) => {
            g.from_y.hash(h);
            g.to_y.hash(h);
            g.from_value.to_bits().hash(h);
            g.to_value.to_bits().hash(h);
        }
        DensityFunction::Noise(n) => {
            n.noise_id.hash(h);
            n.xz_scale.to_bits().hash(h);
            n.y_scale.to_bits().hash(h);
        }
        DensityFunction::ShiftedNoise(sn) => {
            hash_df(&sn.shift_x, h);
            hash_df(&sn.shift_y, h);
            hash_df(&sn.shift_z, h);
            sn.xz_scale.to_bits().hash(h);
            sn.y_scale.to_bits().hash(h);
            sn.noise_id.hash(h);
        }
        DensityFunction::ShiftA(s) => s.noise_id.hash(h),
        DensityFunction::ShiftB(s) => s.noise_id.hash(h),
        DensityFunction::Shift(s) => s.noise_id.hash(h),
        DensityFunction::TwoArgumentSimple(t) => {
            mem::discriminant(&t.op).hash(h);
            hash_df(&t.argument1, h);
            hash_df(&t.argument2, h);
        }
        DensityFunction::Mapped(m) => {
            mem::discriminant(&m.op).hash(h);
            hash_df(&m.input, h);
        }
        DensityFunction::Clamp(c) => {
            c.min.to_bits().hash(h);
            c.max.to_bits().hash(h);
            hash_df(&c.input, h);
        }
        DensityFunction::RangeChoice(rc) => {
            rc.min_inclusive.to_bits().hash(h);
            rc.max_exclusive.to_bits().hash(h);
            hash_df(&rc.input, h);
            hash_df(&rc.when_in_range, h);
            hash_df(&rc.when_out_of_range, h);
        }
        DensityFunction::IntervalSelect(interval) => {
            hash_df(&interval.input, h);
            for threshold in &interval.thresholds {
                threshold.to_bits().hash(h);
            }
            for function in &interval.functions {
                hash_df(function, h);
            }
        }
        DensityFunction::WeirdScaledSampler(ws) => {
            mem::discriminant(&ws.rarity_value_mapper).hash(h);
            ws.noise_id.hash(h);
            hash_df(&ws.input, h);
        }
        DensityFunction::Spline(_)
        | DensityFunction::BlendedNoise(_)
        | DensityFunction::EndIslands
        | DensityFunction::BlendAlpha(_)
        | DensityFunction::BlendOffset(_) => {}
        DensityFunction::BlendDensity(bd) => hash_df(&bd.input, h),
        DensityFunction::Marker(m) => {
            mem::discriminant(&m.kind).hash(h);
            hash_df(&m.wrapped, h);
        }
        DensityFunction::FindTopSurface(fts) => {
            hash_df(&fts.density, h);
            hash_df(&fts.upper_bound, h);
        }
    }
}
