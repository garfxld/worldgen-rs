use std::collections::{BTreeMap, BTreeSet};

use crate::worldgen::backend::ir::types::{CubicSpline, DensityFunction, MarkerType, SplineValue};

pub fn uses_y(function: &DensityFunction) -> bool {
    match function {
        DensityFunction::YClampedGradient(_)
        | DensityFunction::Shift(_)
        | DensityFunction::BlendedNoise(_) => true,
        DensityFunction::Noise(noise) => noise.y_scale != 0.0,
        DensityFunction::ShiftedNoise(shifted) => {
            shifted.y_scale != 0.0 || uses_y(&shifted.shift_y)
        }
        DensityFunction::WeirdScaledSampler(sampler) => uses_y(&sampler.input),
        DensityFunction::TwoArgumentSimple(two_arg) => {
            uses_y(&two_arg.argument1) || uses_y(&two_arg.argument2)
        }
        DensityFunction::Mapped(mapped) => uses_y(&mapped.input),
        DensityFunction::Clamp(clamp) => uses_y(&clamp.input),
        DensityFunction::RangeChoice(range) => {
            uses_y(&range.input) || uses_y(&range.when_in_range) || uses_y(&range.when_out_of_range)
        }
        DensityFunction::IntervalSelect(interval) => {
            uses_y(&interval.input) || interval.functions.iter().any(|branch| uses_y(branch))
        }
        DensityFunction::BlendDensity(blend) => uses_y(&blend.input),
        DensityFunction::Marker(marker) => uses_y(&marker.wrapped),
        DensityFunction::Spline(spline) => spline_uses_y(&spline.spline),
        DensityFunction::FindTopSurface(_)
        | DensityFunction::Reference(_)
        | DensityFunction::Constant(_)
        | DensityFunction::ShiftA(_)
        | DensityFunction::ShiftB(_)
        | DensityFunction::BlendAlpha(_)
        | DensityFunction::BlendOffset(_)
        | DensityFunction::EndIslands => false,
    }
}

pub(crate) fn spline_uses_y(spline: &CubicSpline) -> bool {
    if uses_y(&spline.coordinate) {
        return true;
    }
    spline.points.iter().any(|point| match &point.value {
        SplineValue::Spline(nested) => spline_uses_y(nested),
        SplineValue::Constant(_) => false,
    })
}

pub const fn is_flat_cached(function: &DensityFunction) -> bool {
    match function {
        DensityFunction::Marker(marker) => {
            matches!(marker.kind, MarkerType::FlatCache | MarkerType::Cache2D)
        }
        _ => false,
    }
}

pub fn unwrap_markers(function: &DensityFunction) -> &DensityFunction {
    match function {
        DensityFunction::Marker(marker) => unwrap_markers(&marker.wrapped),
        other => other,
    }
}

pub fn collect_references(function: &DensityFunction) -> Vec<String> {
    let mut references = Vec::new();
    visit_references(function, &mut references);
    references
}

pub(crate) fn visit_references(function: &DensityFunction, references: &mut Vec<String>) {
    match function {
        DensityFunction::Reference(reference) if !references.contains(&reference.id) => {
            references.push(reference.id.clone());
        }
        DensityFunction::Marker(marker) => visit_references(&marker.wrapped, references),
        DensityFunction::TwoArgumentSimple(two_arg) => {
            visit_references(&two_arg.argument1, references);
            visit_references(&two_arg.argument2, references);
        }
        DensityFunction::Mapped(mapped) => visit_references(&mapped.input, references),
        DensityFunction::Clamp(clamp) => visit_references(&clamp.input, references),
        DensityFunction::RangeChoice(range) => {
            visit_references(&range.input, references);
            visit_references(&range.when_in_range, references);
            visit_references(&range.when_out_of_range, references);
        }
        DensityFunction::IntervalSelect(interval) => {
            visit_references(&interval.input, references);
            for branch in &interval.functions {
                visit_references(branch, references);
            }
        }
        DensityFunction::ShiftedNoise(shifted) => {
            visit_references(&shifted.shift_x, references);
            visit_references(&shifted.shift_y, references);
            visit_references(&shifted.shift_z, references);
        }
        DensityFunction::BlendDensity(blend) => visit_references(&blend.input, references),
        DensityFunction::WeirdScaledSampler(sampler) => {
            visit_references(&sampler.input, references);
        }
        DensityFunction::Spline(spline) => visit_spline_references(&spline.spline, references),
        DensityFunction::FindTopSurface(find_top) => {
            visit_references(&find_top.density, references);
            visit_references(&find_top.upper_bound, references);
        }
        _ => {}
    }
}

pub(crate) fn visit_spline_references(spline: &CubicSpline, references: &mut Vec<String>) {
    visit_references(&spline.coordinate, references);
    for point in &spline.points {
        if let SplineValue::Spline(nested) = &point.value {
            visit_spline_references(nested, references);
        }
    }
}

pub fn collect_interpolated_inners(
    function: &DensityFunction,
    registry: &BTreeMap<String, DensityFunction>,
) -> Vec<DensityFunction> {
    let mut inners = Vec::new();
    visit_interpolated_inners(function, registry, &mut inners);
    inners
}

pub(crate) fn visit_interpolated_inners(
    function: &DensityFunction,
    registry: &BTreeMap<String, DensityFunction>,
    inners: &mut Vec<DensityFunction>,
) {
    match function {
        DensityFunction::Marker(marker) if marker.kind == MarkerType::Interpolated => {
            inners.push((*marker.wrapped).clone());
        }
        DensityFunction::Marker(marker) => {
            visit_interpolated_inners(&marker.wrapped, registry, inners);
        }
        DensityFunction::TwoArgumentSimple(two_arg) => {
            visit_interpolated_inners(&two_arg.argument1, registry, inners);
            visit_interpolated_inners(&two_arg.argument2, registry, inners);
        }
        DensityFunction::Mapped(mapped) => {
            visit_interpolated_inners(&mapped.input, registry, inners);
        }
        DensityFunction::Clamp(clamp) => visit_interpolated_inners(&clamp.input, registry, inners),
        DensityFunction::RangeChoice(range) => {
            visit_interpolated_inners(&range.input, registry, inners);
            visit_interpolated_inners(&range.when_in_range, registry, inners);
            visit_interpolated_inners(&range.when_out_of_range, registry, inners);
        }
        DensityFunction::IntervalSelect(interval) => {
            visit_interpolated_inners(&interval.input, registry, inners);
            for branch in &interval.functions {
                visit_interpolated_inners(branch, registry, inners);
            }
        }
        DensityFunction::BlendDensity(blend) => {
            visit_interpolated_inners(&blend.input, registry, inners);
        }
        DensityFunction::WeirdScaledSampler(sampler) => {
            visit_interpolated_inners(&sampler.input, registry, inners);
        }
        DensityFunction::Spline(spline) => {
            visit_spline_interpolated_inners(&spline.spline, registry, inners);
        }
        DensityFunction::ShiftedNoise(shifted) => {
            visit_interpolated_inners(&shifted.shift_x, registry, inners);
            visit_interpolated_inners(&shifted.shift_y, registry, inners);
            visit_interpolated_inners(&shifted.shift_z, registry, inners);
        }
        DensityFunction::FindTopSurface(find_top) => {
            visit_interpolated_inners(&find_top.density, registry, inners);
            visit_interpolated_inners(&find_top.upper_bound, registry, inners);
        }
        DensityFunction::Reference(reference) => {
            if let Some(target) = registry.get(&reference.id) {
                visit_interpolated_inners(target, registry, inners);
            }
        }
        _ => {}
    }
}

pub(crate) fn visit_spline_interpolated_inners(
    spline: &CubicSpline,
    registry: &BTreeMap<String, DensityFunction>,
    inners: &mut Vec<DensityFunction>,
) {
    visit_interpolated_inners(&spline.coordinate, registry, inners);
    for point in &spline.points {
        if let SplineValue::Spline(nested) = &point.value {
            visit_spline_interpolated_inners(nested, registry, inners);
        }
    }
}

pub fn has_blended_noise(
    function: &DensityFunction,
    registry: &BTreeMap<String, DensityFunction>,
    visited: &mut BTreeSet<String>,
) -> bool {
    match function {
        DensityFunction::BlendedNoise(_) => true,
        DensityFunction::TwoArgumentSimple(two_arg) => {
            has_blended_noise(&two_arg.argument1, registry, visited)
                || has_blended_noise(&two_arg.argument2, registry, visited)
        }
        DensityFunction::Mapped(mapped) => has_blended_noise(&mapped.input, registry, visited),
        DensityFunction::Clamp(clamp) => has_blended_noise(&clamp.input, registry, visited),
        DensityFunction::Marker(marker) => has_blended_noise(&marker.wrapped, registry, visited),
        DensityFunction::RangeChoice(range) => {
            has_blended_noise(&range.input, registry, visited)
                || has_blended_noise(&range.when_in_range, registry, visited)
                || has_blended_noise(&range.when_out_of_range, registry, visited)
        }
        DensityFunction::IntervalSelect(interval) => {
            has_blended_noise(&interval.input, registry, visited)
                || interval
                    .functions
                    .iter()
                    .any(|branch| has_blended_noise(branch, registry, visited))
        }
        DensityFunction::BlendDensity(blend) => has_blended_noise(&blend.input, registry, visited),
        DensityFunction::WeirdScaledSampler(sampler) => {
            has_blended_noise(&sampler.input, registry, visited)
        }
        DensityFunction::ShiftedNoise(shifted) => {
            has_blended_noise(&shifted.shift_x, registry, visited)
                || has_blended_noise(&shifted.shift_y, registry, visited)
                || has_blended_noise(&shifted.shift_z, registry, visited)
        }
        DensityFunction::FindTopSurface(find_top) => {
            has_blended_noise(&find_top.density, registry, visited)
                || has_blended_noise(&find_top.upper_bound, registry, visited)
        }
        DensityFunction::Spline(spline) => {
            spline_has_blended_noise(&spline.spline, registry, visited)
        }
        DensityFunction::Reference(reference) => {
            if visited.contains(&reference.id) {
                return false;
            }
            visited.insert(reference.id.clone());
            registry
                .get(&reference.id)
                .is_some_and(|target| has_blended_noise(target, registry, visited))
        }
        _ => false,
    }
}

pub(crate) fn spline_has_blended_noise(
    spline: &CubicSpline,
    registry: &BTreeMap<String, DensityFunction>,
    visited: &mut BTreeSet<String>,
) -> bool {
    if has_blended_noise(&spline.coordinate, registry, visited) {
        return true;
    }
    spline.points.iter().any(|point| match &point.value {
        SplineValue::Spline(nested) => spline_has_blended_noise(nested, registry, visited),
        SplineValue::Constant(_) => false,
    })
}

pub fn has_interpolated_markers(
    function: &DensityFunction,
    registry: &BTreeMap<String, DensityFunction>,
    visited: &mut BTreeSet<String>,
) -> bool {
    match function {
        DensityFunction::Marker(marker) if marker.kind == MarkerType::Interpolated => true,
        DensityFunction::Marker(marker) => {
            has_interpolated_markers(&marker.wrapped, registry, visited)
        }
        DensityFunction::TwoArgumentSimple(two_arg) => {
            has_interpolated_markers(&two_arg.argument1, registry, visited)
                || has_interpolated_markers(&two_arg.argument2, registry, visited)
        }
        DensityFunction::Mapped(mapped) => {
            has_interpolated_markers(&mapped.input, registry, visited)
        }
        DensityFunction::Clamp(clamp) => has_interpolated_markers(&clamp.input, registry, visited),
        DensityFunction::RangeChoice(range) => {
            has_interpolated_markers(&range.input, registry, visited)
                || has_interpolated_markers(&range.when_in_range, registry, visited)
                || has_interpolated_markers(&range.when_out_of_range, registry, visited)
        }
        DensityFunction::IntervalSelect(interval) => {
            has_interpolated_markers(&interval.input, registry, visited)
                || interval
                    .functions
                    .iter()
                    .any(|branch| has_interpolated_markers(branch, registry, visited))
        }
        DensityFunction::BlendDensity(blend) => {
            has_interpolated_markers(&blend.input, registry, visited)
        }
        DensityFunction::WeirdScaledSampler(sampler) => {
            has_interpolated_markers(&sampler.input, registry, visited)
        }
        DensityFunction::ShiftedNoise(shifted) => {
            has_interpolated_markers(&shifted.shift_x, registry, visited)
                || has_interpolated_markers(&shifted.shift_y, registry, visited)
                || has_interpolated_markers(&shifted.shift_z, registry, visited)
        }
        DensityFunction::FindTopSurface(find_top) => {
            has_interpolated_markers(&find_top.density, registry, visited)
                || has_interpolated_markers(&find_top.upper_bound, registry, visited)
        }
        DensityFunction::Reference(reference) => {
            if visited.contains(&reference.id) {
                return false;
            }
            visited.insert(reference.id.clone());
            registry
                .get(&reference.id)
                .is_some_and(|target| has_interpolated_markers(target, registry, visited))
        }
        DensityFunction::Spline(spline) => {
            spline_has_interpolated_markers(&spline.spline, registry, visited)
        }
        _ => false,
    }
}

pub(crate) fn spline_has_interpolated_markers(
    spline: &CubicSpline,
    registry: &BTreeMap<String, DensityFunction>,
    visited: &mut BTreeSet<String>,
) -> bool {
    if has_interpolated_markers(&spline.coordinate, registry, visited) {
        return true;
    }
    spline.points.iter().any(|point| match &point.value {
        SplineValue::Spline(nested) => spline_has_interpolated_markers(nested, registry, visited),
        SplineValue::Constant(_) => false,
    })
}
