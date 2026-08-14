use std::collections::{BTreeMap, BTreeSet};

use crate::worldgen::backend::ir::types::BlendedNoise as BlendedNoiseConfig;

mod analyze;
pub mod bounds;
pub mod fingerprint;
pub mod graph;

pub use bounds::compute_bounds;
pub use fingerprint::{collect_expensive_subexprs, fingerprint};
pub use graph::{
    collect_interpolated_inners, collect_references, has_blended_noise, has_interpolated_markers,
    is_flat_cached, unwrap_markers, uses_y,
};

pub struct AnalysisContext {
    pub noise_ids: BTreeSet<String>,
    pub flat_cached: BTreeSet<String>,
    pub flat_routers: BTreeSet<String>,
    pub topo_order: Vec<String>,
    pub used_names: BTreeSet<String>,
    pub blended_noise_config: Option<BlendedNoiseConfig>,
    pub legacy_random_source: bool,
    pub uses_end_islands: bool,
    pub interpolated_refs: BTreeSet<String>,
    pub blended_noise_refs: BTreeSet<String>,
    pub inline_flat_noises: BTreeMap<u64, (usize, String, f64)>,
}

impl AnalysisContext {
    pub const fn new(legacy_random_source: bool) -> Self {
        Self {
            noise_ids: BTreeSet::new(),
            flat_cached: BTreeSet::new(),
            flat_routers: BTreeSet::new(),
            topo_order: Vec::new(),
            used_names: BTreeSet::new(),
            blended_noise_config: None,
            legacy_random_source,
            uses_end_islands: false,
            interpolated_refs: BTreeSet::new(),
            blended_noise_refs: BTreeSet::new(),
            inline_flat_noises: BTreeMap::new(),
        }
    }
}
