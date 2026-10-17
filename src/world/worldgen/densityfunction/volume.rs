#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DensityVolume {
    pub size_x: i32,
    pub size_y: i32,
    pub size_z: i32,
    pub min_block_x: i32,
    pub min_block_y: i32,
    pub min_block_z: i32,
    pub step_block_x: i32,
    pub step_block_y: i32,
    pub step_block_z: i32,
}

impl DensityVolume {
    pub fn stepped(size: [i32; 3], min_block: [i32; 3], step_block: [i32; 3]) -> Self {
        assert!(
            size.iter().all(|s| *s > 0),
            "Size must be positive, was: {}x{}x{}",
            size[0],
            size[1],
            size[2]
        );
        assert!(
            step_block.iter().all(|s| *s > 0),
            "Step must be positive, was: {}; {}; {}",
            step_block[0],
            step_block[1],
            step_block[2]
        );
        Self {
            size_x: size[0],
            size_y: size[1],
            size_z: size[2],
            min_block_x: min_block[0],
            min_block_y: min_block[1],
            min_block_z: min_block[2],
            step_block_x: step_block[0],
            step_block_y: step_block[1],
            step_block_z: step_block[2],
        }
    }

    pub fn new(size: [i32; 3], min_block: [i32; 3]) -> Self {
        Self::stepped(size, min_block, [1, 1, 1])
    }

    pub const fn index_unchecked(&self, x: i32, y: i32, z: i32) -> usize {
        (y + (x + z * self.size_x) * self.size_y) as usize
    }

    pub const fn block_x(&self, x: i32) -> i32 {
        self.min_block_x + x * self.step_block_x
    }

    pub const fn block_y(&self, y: i32) -> i32 {
        self.min_block_y + y * self.step_block_y
    }

    pub const fn block_z(&self, z: i32) -> i32 {
        self.min_block_z + z * self.step_block_z
    }

    pub const fn max_block_x(&self) -> i32 {
        self.min_block_x + self.size_x * self.step_block_x - 1
    }

    pub const fn max_block_y(&self) -> i32 {
        self.min_block_y + self.size_y * self.step_block_y - 1
    }

    pub const fn max_block_z(&self) -> i32 {
        self.min_block_z + self.size_z * self.step_block_z - 1
    }

    pub const fn size(&self) -> usize {
        (self.size_x * self.size_y * self.size_z) as usize
    }

    const fn contains_block_relative(&self, rx: i32, ry: i32, rz: i32) -> bool {
        rx >= 0
            && ry >= 0
            && rz >= 0
            && rx < self.size_x * self.step_block_x
            && ry < self.size_y * self.step_block_y
            && rz < self.size_z * self.step_block_z
            && rx.rem_euclid(self.step_block_x) == 0
            && ry.rem_euclid(self.step_block_y) == 0
            && rz.rem_euclid(self.step_block_z) == 0
    }

    pub const fn index_of_block(&self, block_x: i32, block_y: i32, block_z: i32) -> Option<usize> {
        let rx = block_x - self.min_block_x;
        let ry = block_y - self.min_block_y;
        let rz = block_z - self.min_block_z;
        if self.step_block_x == 1 && self.step_block_y == 1 && self.step_block_z == 1 {
            if rx >= 0
                && ry >= 0
                && rz >= 0
                && rx < self.size_x
                && ry < self.size_y
                && rz < self.size_z
            {
                return Some(self.index_unchecked(rx, ry, rz));
            }
        } else if self.contains_block_relative(rx, ry, rz) {
            return Some(self.index_unchecked(
                rx.div_euclid(self.step_block_x),
                ry.div_euclid(self.step_block_y),
                rz.div_euclid(self.step_block_z),
            ));
        }
        None
    }
}
