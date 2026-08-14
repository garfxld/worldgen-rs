use rustc_hash::FxHashMap;

use crate::worldgen::backend::ir::{AnalysisContext, DensityProgram};

const SLOT_SIZE: usize = size_of::<f64>();

pub(crate) struct CacheLayout {
    named: FxHashMap<String, usize>,
    routers: FxHashMap<String, usize>,
    inline_noises: FxHashMap<u64, usize>,
    slots: usize,
    grid_side: usize,
}

impl CacheLayout {
    pub(crate) fn new(program: &DensityProgram, analysis: &AnalysisContext) -> Self {
        let mut slots = 0;
        let mut next = || {
            let slot = slots;
            slots += 1;
            slot
        };

        let named: FxHashMap<String, usize> = analysis
            .topo_order
            .iter()
            .filter(|name| analysis.flat_cached.contains(*name))
            .map(|name| (name.clone(), next()))
            .collect();

        let routers: FxHashMap<String, usize> = analysis
            .flat_routers
            .iter()
            .map(|name| (name.clone(), next()))
            .collect();

        let inline_noises: FxHashMap<u64, usize> = analysis
            .inline_flat_noises
            .keys()
            .map(|fingerprint| (*fingerprint, next()))
            .collect();

        Self {
            named,
            routers,
            inline_noises,
            slots,
            grid_side: (16 / program.cell_width + 1) as usize,
        }
    }

    pub(crate) fn named_offset(&self, name: &str) -> Option<i32> {
        self.named.get(name).map(|slot| Self::offset(*slot))
    }

    pub(crate) fn inline_noise_offset(&self, fingerprint: u64) -> Option<i32> {
        self.inline_noises
            .get(&fingerprint)
            .map(|slot| Self::offset(*slot))
    }

    pub(crate) fn router_slot(&self, name: &str) -> Option<usize> {
        self.routers.get(name).copied()
    }

    pub(crate) const fn slots(&self) -> usize {
        self.slots
    }

    pub(crate) const fn grid_side(&self) -> usize {
        self.grid_side
    }

    const fn offset(slot: usize) -> i32 {
        (slot * SLOT_SIZE) as i32
    }
}

pub(crate) enum FlatSlot {
    Named(String),
    Router(String),
    InlineNoise(u64),
}

impl CacheLayout {
    pub(crate) fn flat_order(&self, analysis: &AnalysisContext) -> Vec<FlatSlot> {
        let mut order = Vec::with_capacity(self.slots);
        order.extend(
            analysis
                .topo_order
                .iter()
                .filter(|name| analysis.flat_cached.contains(*name))
                .map(|name| FlatSlot::Named(name.clone())),
        );
        order.extend(
            analysis
                .flat_routers
                .iter()
                .map(|name| FlatSlot::Router(name.clone())),
        );
        order.extend(
            analysis
                .inline_flat_noises
                .keys()
                .map(|fingerprint| FlatSlot::InlineNoise(*fingerprint)),
        );
        order
    }
}
