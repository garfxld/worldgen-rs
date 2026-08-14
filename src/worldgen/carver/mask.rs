#[derive(Debug, Clone)]
pub struct CarvingMask {
    min_y: i32,
    height: i32,
    bits: Vec<u64>,
}

impl CarvingMask {
    pub fn new(height: i32, min_y: i32) -> Self {
        let total_bits = (256 * height) as usize;
        let lanes = total_bits.div_ceil(64);
        Self {
            min_y,
            height,
            bits: vec![0; lanes],
        }
    }

    #[inline]
    const fn index(&self, x: i32, y: i32, z: i32) -> usize {
        let xi = (x & 15) as u32;
        let zi = ((z & 15) as u32) << 4;
        let yi = ((y - self.min_y) as u32) << 8;
        (xi | zi | yi) as usize
    }

    #[inline]
    pub fn set(&mut self, x: i32, y: i32, z: i32) {
        let idx = self.index(x, y, z);
        let lane = idx / 64;
        let bit = idx % 64;
        self.bits[lane] |= 1u64 << bit;
    }

    #[inline]
    pub fn set_if_unset(&mut self, x: i32, y: i32, z: i32) -> bool {
        let idx = self.index(x, y, z);
        let lane = idx / 64;
        let bit = 1u64 << (idx % 64);
        if self.bits[lane] & bit != 0 {
            return false;
        }
        self.bits[lane] |= bit;
        true
    }

    #[inline]
    pub fn get(&self, x: i32, y: i32, z: i32) -> bool {
        let idx = self.index(x, y, z);
        let lane = idx / 64;
        let bit = idx % 64;
        (self.bits[lane] >> bit) & 1 != 0
    }

    pub const fn min_y(&self) -> i32 {
        self.min_y
    }

    pub const fn height(&self) -> i32 {
        self.height
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn set_and_get_roundtrip() {
        let mut mask = CarvingMask::new(384, -64);
        assert!(!mask.get(5, 10, 7));
        mask.set(5, 10, 7);
        assert!(mask.get(5, 10, 7));
        assert!(!mask.get(4, 10, 7));
        assert!(!mask.get(5, 11, 7));
        assert!(!mask.get(5, 10, 8));
    }

    #[test]
    fn set_if_unset_reports_first_visit() {
        let mut mask = CarvingMask::new(384, -64);
        assert!(mask.set_if_unset(5, 10, 7));
        assert!(!mask.set_if_unset(5, 10, 7));
        assert!(mask.get(5, 10, 7));
    }

    #[test]
    fn indexing_matches_vanilla_layout() {
        let mask = CarvingMask::new(384, -64);
        assert_eq!(mask.index(0, -64, 0), 0);
        assert_eq!(mask.index(15, -64, 0), 15);
        assert_eq!(mask.index(0, -64, 1), 16);
        assert_eq!(mask.index(0, -63, 0), 256);
    }

    #[test]
    fn x_and_z_are_masked_to_chunk_local() {
        let mut mask = CarvingMask::new(384, -64);
        mask.set(17, 0, 18);
        assert!(mask.get(1, 0, 2));
        assert!(mask.get(17, 0, 18));
    }
}
