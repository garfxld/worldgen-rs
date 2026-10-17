pub struct CarvingMask {
    min_y: i32,
    max_y: i32,
    height: i32,
    mask: Vec<u64>,
}

impl CarvingMask {
    pub fn new(min_y: i32, max_y: i32) -> Self {
        let height = max_y - min_y + 1;
        Self {
            min_y,
            max_y,
            height,
            mask: vec![0; (256 * height as usize).div_ceil(64)],
        }
    }

    const fn get_index(&self, x: i32, y: i32, z: i32) -> usize {
        (y - self.min_y + (z + (x << 4)) * self.height) as usize
    }

    pub const fn min_y(&self) -> i32 {
        self.min_y
    }

    pub const fn max_y(&self) -> i32 {
        self.max_y
    }

    pub fn carve(&mut self, x: i32, y: i32, z: i32) {
        let index = self.get_index(x, y, z);
        self.mask[index / 64] |= 1 << (index % 64);
    }

    fn next_bit(&self, from: usize, set: bool) -> Option<usize> {
        let mut word_index = from / 64;
        if word_index >= self.mask.len() {
            return if set { None } else { Some(from) };
        }
        let flip = if set { 0 } else { u64::MAX };
        let mut word = (self.mask[word_index] ^ flip) & (u64::MAX << (from % 64));
        loop {
            if word != 0 {
                return Some(word_index * 64 + word.trailing_zeros() as usize);
            }
            word_index += 1;
            if word_index == self.mask.len() {
                return if set { None } else { Some(word_index * 64) };
            }
            word = self.mask[word_index] ^ flip;
        }
    }

    pub fn visit(&self, mut visitor: impl FnMut(i32, i32, i32, i32)) {
        let mut start_index = self.next_bit(0, true);
        while let Some(start) = start_index {
            let end = self.next_bit(start, false).unwrap_or(start + 1) - 1;
            self.visit_segment(&mut visitor, start as i32, end as i32);
            start_index = self.next_bit(end + 1, true);
        }
    }

    fn visit_segment(
        &self,
        visitor: &mut impl FnMut(i32, i32, i32, i32),
        start_index: i32,
        end_index: i32,
    ) {
        let start_column = start_index / self.height;
        let end_column = end_index / self.height;
        for column in start_column..=end_column {
            let column_x = column >> 4 & 15;
            let column_z = column & 15;
            let column_base_index = column * self.height;
            let bottom_y = (start_index - column_base_index).max(0) + self.min_y;
            let top_y = (end_index - column_base_index).min(self.height - 1) + self.min_y;
            visitor(column_x, column_z, bottom_y, top_y);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.mask.iter().all(|word| *word == 0)
    }
}
