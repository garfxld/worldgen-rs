use crate::worldgen::backend::jit::RouterEntry;
use crate::worldgen::density::Noises;

pub struct ColumnCache {
    active: Vec<f64>,
    active_xz: Option<(i32, i32)>,
    grid: Vec<f64>,
    grid_origin: Option<(i32, i32)>,
}

impl ColumnCache {
    pub fn new(noises: &Noises) -> Self {
        Self {
            active: vec![0.0; noises.compiled().slots()],
            active_xz: None,
            grid: Vec::new(),
            grid_origin: None,
        }
    }

    pub fn ensure(&mut self, x: i32, z: i32, noises: &Noises) {
        if self.grid_slice(x, z, noises).is_some() {
            return;
        }
        if self.active_xz == Some((x, z)) {
            return;
        }
        Self::fill_slot(&mut self.active, x, z, noises);
        self.active_xz = Some((x, z));
    }

    pub fn init_grid(&mut self, chunk_block_x: i32, chunk_block_z: i32, noises: &Noises) {
        let side = noises.compiled().grid_side();
        let slots = noises.compiled().slots();
        let cell_width = noises.cell_width();

        self.grid.clear();
        self.grid.resize(side * side * slots, 0.0);
        for gx in 0..side {
            let x = chunk_block_x + gx as i32 * cell_width;
            for gz in 0..side {
                let z = chunk_block_z + gz as i32 * cell_width;
                let base = (gx * side + gz) * slots;
                Self::fill_slot(&mut self.grid[base..base + slots], x, z, noises);
            }
        }
        self.grid_origin = Some((chunk_block_x, chunk_block_z));
    }

    fn fill_slot(slot: &mut [f64], x: i32, z: i32, noises: &Noises) {
        for step in noises.compiled().flat_steps() {
            // SAFETY: `step.func` was compiled with the `Full` shape, matching
            // `(*const f64, f64, f64, f64) -> f64`. Earlier slots in `flat_steps`'
            // topological order are already populated when a later step reads
            // them through `cache`; the step being written is never itself read.
            let value = unsafe { (step.func)(slot.as_ptr(), f64::from(x), 0.0, f64::from(z)) };
            slot[step.slot] = value;
        }
    }

    fn grid_slice(&self, x: i32, z: i32, noises: &Noises) -> Option<&[f64]> {
        let (origin_x, origin_z) = self.grid_origin?;
        let side = noises.compiled().grid_side();
        let slots = noises.compiled().slots();
        let cell_width = noises.cell_width();

        let dx = x - origin_x;
        let dz = z - origin_z;
        if dx < 0 || dz < 0 || dx % cell_width != 0 || dz % cell_width != 0 {
            return None;
        }
        let gx = (dx / cell_width) as usize;
        let gz = (dz / cell_width) as usize;
        if gx >= side || gz >= side {
            return None;
        }
        let base = (gx * side + gz) * slots;
        Some(&self.grid[base..base + slots])
    }

    pub(crate) fn ptr(&self, x: i32, z: i32, noises: &Noises) -> *const f64 {
        self.grid_slice(x, z, noises).map_or_else(
            || {
                debug_assert_eq!(
                    self.active_xz,
                    Some((x, z)),
                    "ensure() not called for ({x}, {z})"
                );
                self.active.as_ptr()
            },
            <[f64]>::as_ptr,
        )
    }

    pub fn router(&self, name: &str, x: i32, y: i32, z: i32, noises: &Noises) -> f64 {
        let entry = noises
            .compiled()
            .router(name)
            .unwrap_or_else(|| panic!("no router entry named {name}"));
        let cache = self.ptr(x, z, noises);
        match entry {
            RouterEntry::Cached(slot) => {
                // SAFETY: `cache` points at a buffer of `noises.compiled().slots()`
                // entries, populated up through this router's flat step by `ensure`
                // or `init_grid`, and `slot` is within that range.
                unsafe { *cache.add(*slot) }
            }
            // SAFETY: compiled with the `Full` shape, matching `cache`'s layout.
            RouterEntry::Computed(func) => unsafe {
                func(cache, f64::from(x), f64::from(y), f64::from(z))
            },
        }
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test loads the checked-in vanilla datapack"
)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::worldgen::backend::ir::loader::DatapackSource;

    fn datapack_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/datapack/worldgen")
    }

    #[test]
    fn lazy_and_grid_paths_agree() {
        let source = DatapackSource::new(datapack_root());
        let registry = source.density_function_registry().unwrap();
        let (program, settings) = source.program("overworld", "Overworld", &registry).unwrap();
        let params = source.noise_parameters().unwrap();
        let noises = Noises::new(&program, &settings, &params, 42).unwrap();

        let mut lazy = ColumnCache::new(&noises);
        let mut gridded = ColumnCache::new(&noises);
        gridded.init_grid(0, 0, &noises);

        for &(x, z) in &[(0, 0), (4, 0), (0, 4), (12, 12)] {
            lazy.ensure(x, z, &noises);
            for name in [
                "temperature",
                "vegetation",
                "continentalness",
                "erosion",
                "depth",
                "ridges",
            ] {
                let a = lazy.router(name, x, 64, z, &noises);
                let b = gridded.router(name, x, 64, z, &noises);
                assert_eq!(a, b, "{name} mismatch at ({x}, {z})");
                assert!(a.is_finite());
            }
        }
    }
}
