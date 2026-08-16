use crate::util::pos::BlockPos;

pub struct JavaHashSet {
    table: Vec<Vec<BlockPos>>,
    size: usize,
}

impl JavaHashSet {
    const DEFAULT_CAPACITY: usize = 16;

    pub fn new() -> JavaHashSet {
        JavaHashSet {
            table: vec![Vec::new(); Self::DEFAULT_CAPACITY],
            size: 0,
        }
    }

    fn hash(pos: BlockPos) -> u32 {
        let raw = (pos.y().wrapping_add(pos.z().wrapping_mul(31)))
            .wrapping_mul(31)
            .wrapping_add(pos.x()) as u32;
        raw ^ (raw >> 16)
    }

    fn bucket(&self, pos: BlockPos) -> usize {
        Self::hash(pos) as usize & (self.table.len() - 1)
    }

    pub fn len(&self) -> usize {
        self.size
    }

    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    pub fn contains(&self, pos: BlockPos) -> bool {
        self.table[self.bucket(pos)].contains(&pos)
    }

    pub fn insert(&mut self, pos: BlockPos) -> bool {
        let bucket = self.bucket(pos);
        if self.table[bucket].contains(&pos) {
            return false;
        }
        self.table[bucket].push(pos);
        self.size += 1;
        if self.size > self.table.len() * 3 / 4 {
            self.resize();
        }
        true
    }

    fn resize(&mut self) {
        let capacity = self.table.len() * 2;
        let old = std::mem::replace(&mut self.table, vec![Vec::new(); capacity]);
        for bucket in old {
            for pos in bucket {
                let index = self.bucket(pos);
                self.table[index].push(pos);
            }
        }
    }

    pub fn remove_first(&mut self) -> Option<BlockPos> {
        let bucket = self.table.iter().position(|bucket| !bucket.is_empty())?;
        self.size -= 1;
        Some(self.table[bucket].remove(0))
    }

    pub fn iter(&self) -> impl Iterator<Item = BlockPos> + '_ {
        self.table.iter().flatten().copied()
    }
}

impl Default for JavaHashSet {
    fn default() -> JavaHashSet {
        JavaHashSet::new()
    }
}

impl FromIterator<BlockPos> for JavaHashSet {
    fn from_iter<T: IntoIterator<Item = BlockPos>>(iter: T) -> JavaHashSet {
        let mut set = JavaHashSet::new();
        for pos in iter {
            set.insert(pos);
        }
        set
    }
}
