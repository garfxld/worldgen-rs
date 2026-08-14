use std::collections::BTreeMap;

pub mod analysis;
pub mod json;
pub mod loader;
pub mod types;

pub use analysis::AnalysisContext;
pub use types::*;

pub struct DensityProgram {
    pub registry: BTreeMap<String, DensityFunction>,
    pub router_entries: BTreeMap<String, DensityFunction>,
    pub prefix: String,
    pub cell_width: i32,
    pub legacy_random_source: bool,
}

impl DensityProgram {
    pub fn analyze(&self) -> AnalysisContext {
        let mut ctx = AnalysisContext::new(self.legacy_random_source);
        ctx.analyze(self);
        ctx
    }
}
