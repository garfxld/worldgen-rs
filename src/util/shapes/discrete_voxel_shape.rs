use crate::util::axis::Axis;

#[derive(Debug, Clone, PartialEq, Eq)]
struct BitSet {
    words: Vec<u64>,
}

impl BitSet {
    fn new(bits: usize) -> Self {
        Self {
            words: vec![0; bits.div_ceil(64)],
        }
    }

    fn ensure(&mut self, index: usize) {
        let word = index / 64;
        if word >= self.words.len() {
            self.words.resize(word + 1, 0);
        }
    }

    fn set(&mut self, index: usize) {
        self.ensure(index);
        self.words[index / 64] |= 1u64 << (index % 64);
    }

    fn get(&self, index: usize) -> bool {
        let word = index / 64;
        word < self.words.len() && (self.words[word] >> (index % 64)) & 1 == 1
    }

    fn clear_range(&mut self, from: usize, to: usize) {
        for index in from..to {
            let word = index / 64;
            if word < self.words.len() {
                self.words[word] &= !(1u64 << (index % 64));
            }
        }
    }

    fn next_clear_bit(&self, from: usize) -> usize {
        let mut index = from;
        loop {
            let word = index / 64;
            if word >= self.words.len() {
                return index;
            }
            if !self.get(index) {
                return index;
            }
            index += 1;
        }
    }

    fn is_empty(&self) -> bool {
        self.words.iter().all(|word| *word == 0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitSetDiscreteVoxelShape {
    x_size: usize,
    y_size: usize,
    z_size: usize,
    storage: BitSet,
    x_min: i32,
    y_min: i32,
    z_min: i32,
    x_max: i32,
    y_max: i32,
    z_max: i32,
}

impl BitSetDiscreteVoxelShape {
    pub fn new(x_size: usize, y_size: usize, z_size: usize) -> Self {
        Self {
            x_size,
            y_size,
            z_size,
            storage: BitSet::new(x_size * y_size * z_size),
            x_min: x_size as i32,
            y_min: y_size as i32,
            z_min: z_size as i32,
            x_max: 0,
            y_max: 0,
            z_max: 0,
        }
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "mirrors vanilla BitSetDiscreteVoxelShape.withFilledBounds"
    )]
    pub fn with_filled_bounds(
        x_size: usize,
        y_size: usize,
        z_size: usize,
        x_min: i32,
        y_min: i32,
        z_min: i32,
        x_max: i32,
        y_max: i32,
        z_max: i32,
    ) -> Self {
        let mut shape = Self::new(x_size, y_size, z_size);
        shape.x_min = x_min;
        shape.y_min = y_min;
        shape.z_min = z_min;
        shape.x_max = x_max;
        shape.y_max = y_max;
        shape.z_max = z_max;

        for x in x_min..x_max {
            for y in y_min..y_max {
                for z in z_min..z_max {
                    shape.fill_update_bounds(x, y, z, false);
                }
            }
        }

        shape
    }

    const fn index(&self, x: i32, y: i32, z: i32) -> usize {
        ((x as usize * self.y_size) + y as usize) * self.z_size + z as usize
    }

    pub const fn size(&self, axis: Axis) -> usize {
        match axis {
            Axis::X => self.x_size,
            Axis::Y => self.y_size,
            Axis::Z => self.z_size,
        }
    }

    pub fn is_full(&self, x: i32, y: i32, z: i32) -> bool {
        self.storage.get(self.index(x, y, z))
    }

    pub fn is_full_wide(&self, x: i32, y: i32, z: i32) -> bool {
        if x < 0 || y < 0 || z < 0 {
            return false;
        }
        if x < self.x_size as i32 && y < self.y_size as i32 && z < self.z_size as i32 {
            self.is_full(x, y, z)
        } else {
            false
        }
    }

    fn fill_update_bounds(&mut self, x: i32, y: i32, z: i32, update_bounds: bool) {
        let index = self.index(x, y, z);
        self.storage.set(index);
        if update_bounds {
            self.x_min = self.x_min.min(x);
            self.y_min = self.y_min.min(y);
            self.z_min = self.z_min.min(z);
            self.x_max = self.x_max.max(x + 1);
            self.y_max = self.y_max.max(y + 1);
            self.z_max = self.z_max.max(z + 1);
        }
    }

    pub fn fill(&mut self, x: i32, y: i32, z: i32) {
        self.fill_update_bounds(x, y, z, true);
    }

    pub fn is_empty(&self) -> bool {
        self.storage.is_empty()
    }

    pub const fn first_full(&self, axis: Axis) -> i32 {
        match axis {
            Axis::X => self.x_min,
            Axis::Y => self.y_min,
            Axis::Z => self.z_min,
        }
    }

    pub const fn last_full(&self, axis: Axis) -> i32 {
        match axis {
            Axis::X => self.x_max,
            Axis::Y => self.y_max,
            Axis::Z => self.z_max,
        }
    }

    pub fn for_all_boxes(&self, consumer: &mut dyn FnMut(i32, i32, i32, i32, i32, i32)) {
        let mut shape = self.clone();

        for y in 0..shape.y_size as i32 {
            for x in 0..shape.x_size as i32 {
                let mut last_start_z = -1;

                for z in 0..=shape.z_size as i32 {
                    if shape.is_full_wide(x, y, z) {
                        if last_start_z == -1 {
                            last_start_z = z;
                        }
                    } else if last_start_z != -1 {
                        let mut end_x = x;
                        let mut end_y = y;
                        shape.clear_z_strip(last_start_z, z, x, y);

                        while shape.is_z_strip_full(last_start_z, z, end_x + 1, y) {
                            shape.clear_z_strip(last_start_z, z, end_x + 1, y);
                            end_x += 1;
                        }

                        while shape.is_xz_rectangle_full(x, end_x + 1, last_start_z, z, end_y + 1) {
                            for cx in x..=end_x {
                                shape.clear_z_strip(last_start_z, z, cx, end_y + 1);
                            }

                            end_y += 1;
                        }

                        consumer(x, y, last_start_z, end_x + 1, end_y + 1, z);
                        last_start_z = -1;
                    }
                }
            }
        }
    }

    fn is_z_strip_full(&self, start_z: i32, end_z: i32, x: i32, y: i32) -> bool {
        if x < self.x_size as i32 && y < self.y_size as i32 {
            self.storage.next_clear_bit(self.index(x, y, start_z)) >= self.index(x, y, end_z)
        } else {
            false
        }
    }

    fn is_xz_rectangle_full(
        &self,
        start_x: i32,
        end_x: i32,
        start_z: i32,
        end_z: i32,
        y: i32,
    ) -> bool {
        (start_x..end_x).all(|x| self.is_z_strip_full(start_z, end_z, x, y))
    }

    fn clear_z_strip(&mut self, start_z: i32, end_z: i32, x: i32, y: i32) {
        let from = self.index(x, y, start_z);
        let to = self.index(x, y, end_z);
        self.storage.clear_range(from, to);
    }

    pub const fn set_bounds(
        &mut self,
        x_min: i32,
        y_min: i32,
        z_min: i32,
        x_max: i32,
        y_max: i32,
        z_max: i32,
    ) {
        self.x_min = x_min;
        self.y_min = y_min;
        self.z_min = z_min;
        self.x_max = x_max;
        self.y_max = y_max;
        self.z_max = z_max;
    }

    pub fn set(&mut self, x: i32, y: i32, z: i32) {
        let index = self.index(x, y, z);
        self.storage.set(index);
    }
}
