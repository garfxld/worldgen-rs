use std::collections::{BTreeMap, BTreeSet};

use super::AnalysisContext;
use super::fingerprint::collect_inline_flat_noises;
use super::graph::{
    collect_references, has_blended_noise, has_interpolated_markers, is_flat_cached,
    unwrap_markers, uses_y,
};
use crate::worldgen::backend::ir::DensityProgram;
use crate::worldgen::backend::ir::types::{CubicSpline, DensityFunction, SplineValue};

impl AnalysisContext {
    pub fn analyze(&mut self, input: &DensityProgram) {
        for df in input.router_entries.values() {
            self.walk_df(df, input);
        }

        for name in &self.used_names {
            if let Some(df) = input.registry.get(name)
                && is_flat_cached(df)
            {
                self.flat_cached.insert(name.clone());
            }
        }

        loop {
            let mut changed = false;
            for name in &self.used_names.clone() {
                if self.flat_cached.contains(name) {
                    continue;
                }
                let Some(df) = input.registry.get(name) else {
                    continue;
                };
                let inner = unwrap_markers(df);
                if !uses_y(inner)
                    && collect_references(inner)
                        .iter()
                        .all(|dep| self.flat_cached.contains(dep))
                {
                    self.flat_cached.insert(name.clone());
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }

        for (name, df) in &input.router_entries {
            let inner = unwrap_markers(df);
            if is_flat_cached(df)
                || (!uses_y(inner)
                    && collect_references(inner)
                        .iter()
                        .all(|dep| self.flat_cached.contains(dep)))
            {
                self.flat_routers.insert(name.clone());
            }
        }

        {
            let mut seen = BTreeMap::new();
            for name in &self.used_names {
                if self.flat_cached.contains(name) {
                    continue;
                }
                if let Some(df) = input.registry.get(name) {
                    collect_inline_flat_noises(unwrap_markers(df), &mut seen);
                }
            }
            for (name, df) in &input.router_entries {
                if self.flat_routers.contains(name) {
                    continue;
                }
                collect_inline_flat_noises(unwrap_markers(df), &mut seen);
            }
            for (fp, (noise_id, xz_scale)) in seen {
                let idx = self.inline_flat_noises.len();
                self.inline_flat_noises
                    .insert(fp, (idx, noise_id, xz_scale));
            }
        }

        for name in &self.used_names {
            if let Some(df) = input.registry.get(name) {
                let mut visited = BTreeSet::new();
                if has_interpolated_markers(df, &input.registry, &mut visited) {
                    self.interpolated_refs.insert(name.clone());
                }
                let mut visited = BTreeSet::new();
                if has_blended_noise(df, &input.registry, &mut visited) {
                    self.blended_noise_refs.insert(name.clone());
                }
            }
        }

        self.topo_order = self.topological_sort(input);
    }

    pub(crate) fn walk_df(&mut self, df: &DensityFunction, input: &DensityProgram) {
        match df {
            DensityFunction::Constant(_)
            | DensityFunction::BlendAlpha(_)
            | DensityFunction::BlendOffset(_)
            | DensityFunction::YClampedGradient(_) => {}

            DensityFunction::EndIslands => {
                self.uses_end_islands = true;
            }

            DensityFunction::Noise(n) => {
                self.noise_ids.insert(n.noise_id.clone());
            }
            DensityFunction::ShiftedNoise(sn) => {
                self.walk_df(&sn.shift_x, input);
                self.walk_df(&sn.shift_y, input);
                self.walk_df(&sn.shift_z, input);
                self.noise_ids.insert(sn.noise_id.clone());
            }
            DensityFunction::ShiftA(s) => {
                self.noise_ids.insert(s.noise_id.clone());
            }
            DensityFunction::ShiftB(s) => {
                self.noise_ids.insert(s.noise_id.clone());
            }
            DensityFunction::Shift(s) => {
                self.noise_ids.insert(s.noise_id.clone());
            }
            DensityFunction::TwoArgumentSimple(t) => {
                self.walk_df(&t.argument1, input);
                self.walk_df(&t.argument2, input);
            }
            DensityFunction::Mapped(m) => self.walk_df(&m.input, input),
            DensityFunction::Clamp(c) => self.walk_df(&c.input, input),
            DensityFunction::RangeChoice(rc) => {
                self.walk_df(&rc.input, input);
                self.walk_df(&rc.when_in_range, input);
                self.walk_df(&rc.when_out_of_range, input);
            }
            DensityFunction::IntervalSelect(interval) => {
                self.walk_df(&interval.input, input);
                for function in &interval.functions {
                    self.walk_df(function, input);
                }
            }
            DensityFunction::Spline(s) => self.walk_spline(&s.spline, input),
            DensityFunction::BlendedNoise(bn) => {
                self.blended_noise_config = Some(bn.clone());
            }
            DensityFunction::WeirdScaledSampler(ws) => {
                self.walk_df(&ws.input, input);
                self.noise_ids.insert(ws.noise_id.clone());
            }
            DensityFunction::BlendDensity(bd) => self.walk_df(&bd.input, input),
            DensityFunction::Marker(m) => self.walk_df(&m.wrapped, input),
            DensityFunction::FindTopSurface(fts) => {
                self.walk_df(&fts.density, input);
                self.walk_df(&fts.upper_bound, input);
            }
            DensityFunction::Reference(r) => {
                if !self.used_names.contains(&r.id) {
                    self.used_names.insert(r.id.clone());
                    if let Some(ref_df) = input.registry.get(&r.id) {
                        self.walk_df(ref_df, input);
                    }
                }
            }
        }
    }

    pub(crate) fn walk_spline(&mut self, spline: &CubicSpline, input: &DensityProgram) {
        self.walk_df(&spline.coordinate, input);
        for point in &spline.points {
            if let SplineValue::Spline(nested) = &point.value {
                self.walk_spline(nested, input);
            }
        }
    }

    pub(crate) fn topological_sort(&self, input: &DensityProgram) -> Vec<String> {
        let mut visited = BTreeSet::new();
        let mut order = Vec::new();
        for name in &self.used_names {
            self.topo_visit(name, input, &mut visited, &mut order);
        }
        order
    }

    pub(crate) fn topo_visit(
        &self,
        name: &str,
        input: &DensityProgram,
        visited: &mut BTreeSet<String>,
        order: &mut Vec<String>,
    ) {
        if visited.contains(name) {
            return;
        }
        visited.insert(name.to_string());
        if let Some(df) = input.registry.get(name) {
            for dep in collect_references(df) {
                if self.used_names.contains(&dep) {
                    self.topo_visit(&dep, input, visited, order);
                }
            }
        }
        order.push(name.to_string());
    }
}
