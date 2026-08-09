use rustc_hash::{FxHashMap, FxHashSet};

use crate::worldgen::backend::ir::analysis::graph::unwrap_markers;
use crate::worldgen::backend::ir::{
    AnalysisContext, CubicSpline, DensityFunction, DensityProgram, SplineValue,
};

const INLINE_BUDGET: usize = 128;

pub(crate) struct InlinePolicy {
    inline: FxHashSet<String>,
}

impl InlinePolicy {
    pub(crate) fn new(program: &DensityProgram, analysis: &AnalysisContext) -> Self {
        let mut expanded: FxHashMap<&str, usize> = FxHashMap::default();
        let mut inline: FxHashSet<String> = FxHashSet::default();

        for name in &analysis.topo_order {
            let Some(df) = program.registry.get(name) else {
                continue;
            };

            if analysis.flat_cached.contains(name) {
                continue;
            }

            let size = Self::expanded_size(unwrap_markers(df), &expanded, &inline, analysis);
            expanded.insert(name.as_str(), size);
            if size <= INLINE_BUDGET {
                inline.insert(name.clone());
            }
        }

        Self { inline }
    }

    pub(crate) fn should_inline(&self, name: &str) -> bool {
        self.inline.contains(name)
    }

    fn expanded_size(
        df: &DensityFunction,
        expanded: &FxHashMap<&str, usize>,
        inline: &FxHashSet<String>,
        analysis: &AnalysisContext,
    ) -> usize {
        let recurse =
            |child: &DensityFunction| Self::expanded_size(child, expanded, inline, analysis);

        1 + match df {
            DensityFunction::Reference(r) => {
                if analysis.flat_cached.contains(&r.id) || !inline.contains(&r.id) {
                    0
                } else {
                    expanded.get(r.id.as_str()).copied().unwrap_or(0)
                }
            }

            DensityFunction::Constant(_)
            | DensityFunction::YClampedGradient(_)
            | DensityFunction::Noise(_)
            | DensityFunction::Shift(_)
            | DensityFunction::ShiftA(_)
            | DensityFunction::ShiftB(_)
            | DensityFunction::BlendAlpha(_)
            | DensityFunction::BlendOffset(_)
            | DensityFunction::EndIslands => 0,

            DensityFunction::BlendedNoise(_) => INLINE_BUDGET,

            DensityFunction::Clamp(c) => recurse(&c.input),
            DensityFunction::Mapped(m) => recurse(&m.input),
            DensityFunction::BlendDensity(bd) => recurse(&bd.input),
            DensityFunction::Marker(m) => recurse(&m.wrapped),
            DensityFunction::WeirdScaledSampler(ws) => recurse(&ws.input),
            DensityFunction::TwoArgumentSimple(t) => recurse(&t.argument1) + recurse(&t.argument2),
            DensityFunction::ShiftedNoise(sn) => {
                recurse(&sn.shift_x) + recurse(&sn.shift_y) + recurse(&sn.shift_z)
            }
            DensityFunction::RangeChoice(rc) => {
                recurse(&rc.input) + recurse(&rc.when_in_range) + recurse(&rc.when_out_of_range)
            }
            DensityFunction::IntervalSelect(interval) => {
                recurse(&interval.input)
                    + interval.functions.iter().map(|f| recurse(f)).sum::<usize>()
            }
            DensityFunction::FindTopSurface(fts) => {
                recurse(&fts.density) + recurse(&fts.upper_bound)
            }
            DensityFunction::Spline(s) => Self::spline_size(&s.spline, expanded, inline, analysis),
        }
    }

    fn spline_size(
        spline: &CubicSpline,
        expanded: &FxHashMap<&str, usize>,
        inline: &FxHashSet<String>,
        analysis: &AnalysisContext,
    ) -> usize {
        let mut size = Self::expanded_size(&spline.coordinate, expanded, inline, analysis);
        for point in &spline.points {
            size += match &point.value {
                SplineValue::Constant(_) => 1,
                SplineValue::Spline(nested) => {
                    Self::spline_size(nested, expanded, inline, analysis)
                }
            };
        }
        size
    }
}
