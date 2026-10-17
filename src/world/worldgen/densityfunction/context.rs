use crate::world::worldgen::beardifier::Beardifier;

use super::volume::DensityVolume;

pub fn block_pos_as_long(x: i32, y: i32, z: i32) -> i64 {
    ((i64::from(x) & 0x3FF_FFFF) << 38)
        | (i64::from(y) & 0xFFF)
        | ((i64::from(z) & 0x3FF_FFFF) << 12)
}

#[derive(Debug, Clone)]
pub struct CacheCell {
    pub volume: Option<DensityVolume>,
    pub buffer: Vec<f32>,
    pub value_key: i64,
    pub value: f32,
}

impl Default for CacheCell {
    fn default() -> Self {
        Self {
            volume: None,
            buffer: Vec::new(),
            value_key: 0,
            value: f32::NAN,
        }
    }
}

#[derive(Default)]
pub struct SamplerContext<'a> {
    pub beardifier: Option<&'a Beardifier>,
    pub cache_cells: Option<Vec<CacheCell>>,
}

impl<'a> SamplerContext<'a> {
    pub const fn uncached() -> Self {
        Self {
            beardifier: None,
            cache_cells: None,
        }
    }

    pub const fn cached() -> Self {
        Self {
            beardifier: None,
            cache_cells: Some(Vec::new()),
        }
    }

    #[must_use]
    pub const fn with_beardifier(mut self, beardifier: Option<&'a Beardifier>) -> Self {
        self.beardifier = beardifier;
        self
    }

    pub fn cache_cell(&mut self, id: usize) -> Option<&mut CacheCell> {
        let cells = self.cache_cells.as_mut()?;
        if id >= cells.len() {
            let new_size = (id + 1).div_ceil(16) * 16;
            cells.resize_with(new_size, CacheCell::default);
        }
        Some(&mut cells[id])
    }
}
