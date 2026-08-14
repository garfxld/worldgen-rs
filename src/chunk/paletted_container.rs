use std::fmt::Debug;
use std::hash::Hash;
use std::mem;
use std::slice;

use crate::util::ident::BlockState;

type Cube<T, const DIM: usize> = [[[T; DIM]; DIM]; DIM];

pub trait PackedValue: Copy + Eq + Hash + Default {
    fn global_id(self) -> u16;
    fn from_global_id(id: u16) -> Self;
}

impl PackedValue for BlockState {
    fn global_id(self) -> u16 {
        self.0
    }

    fn from_global_id(id: u16) -> Self {
        Self(id)
    }
}

impl PackedValue for u16 {
    fn global_id(self) -> u16 {
        self
    }

    fn from_global_id(id: u16) -> Self {
        id
    }
}

#[derive(Debug, Clone)]
pub struct BitStorage {
    data: Vec<u64>,
    bits: u32,
    mask: u64,
    values_per_word: u32,
    len: usize,
}

impl BitStorage {
    fn new(bits: u32, len: usize) -> Self {
        debug_assert!((1..=32).contains(&bits));
        let values_per_word = 64 / bits;
        let word_count = len.div_ceil(values_per_word as usize);
        Self {
            data: vec![0; word_count],
            bits,
            mask: (1u64 << bits) - 1,
            values_per_word,
            len,
        }
    }

    #[inline]
    fn locate(&self, index: usize) -> (usize, u32) {
        debug_assert!(index < self.len);
        let word_index = index / self.values_per_word as usize;
        let bit_offset = (index % self.values_per_word as usize) as u32 * self.bits;
        (word_index, bit_offset)
    }

    #[inline]
    fn get(&self, index: usize) -> u32 {
        let (word_index, bit_offset) = self.locate(index);
        ((self.data[word_index] >> bit_offset) & self.mask) as u32
    }

    #[inline]
    fn set(&mut self, index: usize, value: u32) -> u32 {
        debug_assert!(u64::from(value) <= self.mask);
        let (word_index, bit_offset) = self.locate(index);
        let word = &mut self.data[word_index];
        let old = (*word >> bit_offset) & self.mask;
        *word = (*word & !(self.mask << bit_offset)) | (u64::from(value) << bit_offset);
        old as u32
    }

    fn iter(&self) -> impl Iterator<Item = u32> + '_ {
        (0..self.len).map(|index| self.get(index))
    }
}

#[derive(Debug, Clone)]
pub struct LocalPalette<V> {
    entries: Vec<(V, u16)>,
}

impl<V: PackedValue> LocalPalette<V> {
    fn id_for(&mut self, value: V) -> Option<u32> {
        self.entries
            .iter()
            .position(|&(v, _)| v == value)
            .map(|index| index as u32)
    }

    fn insert(&mut self, value: V) -> u32 {
        let id = self.entries.len() as u32;
        self.entries.push((value, 0));
        id
    }

    fn value_at(&self, id: u32) -> V {
        self.entries[id as usize].0
    }

    fn bump_count(&mut self, id: u32, delta: i32) {
        let count = &mut self.entries[id as usize].1;
        *count = (i32::from(*count) + delta) as u16;
    }
}

#[derive(Debug, Clone)]
pub enum Packed<V> {
    Local {
        ids: BitStorage,
        palette: LocalPalette<V>,
    },
    Global {
        ids: BitStorage,
    },
}

const GLOBAL_PALETTE_CUTOVER: usize = 64;

impl<V: PackedValue> Packed<V> {
    fn from_values(values: &[V], volume: usize) -> Self {
        let mut palette = LocalPalette { entries: Vec::new() };
        let mut ids = Vec::with_capacity(volume);
        for &value in values {
            let id = match palette.id_for(value) {
                Some(id) => id,
                None => palette.insert(value),
            };
            palette.bump_count(id, 1);
            ids.push(id);
        }

        if palette.entries.len() > GLOBAL_PALETTE_CUTOVER {
            let mut storage = BitStorage::new(16, volume);
            for (index, &value) in values.iter().enumerate() {
                storage.set(index, u32::from(value.global_id()));
            }
            return Self::Global { ids: storage };
        }

        let bits = bits_for_distinct_count(palette.entries.len());
        let mut storage = BitStorage::new(bits, volume);
        for (index, &id) in ids.iter().enumerate() {
            storage.set(index, id);
        }
        Self::Local {
            ids: storage,
            palette,
        }
    }

    fn get(&self, index: usize) -> V {
        match self {
            Self::Local { ids, palette } => palette.value_at(ids.get(index)),
            Self::Global { ids } => V::from_global_id(ids.get(index) as u16),
        }
    }

    fn set(&mut self, index: usize, value: V) -> V {
        match self {
            Self::Local { ids, palette } => {
                let old_id = ids.get(index);
                let old_value = palette.value_at(old_id);
                if old_value == value {
                    return old_value;
                }

                let new_id = match palette.id_for(value) {
                    Some(id) => id,
                    None => {
                        let id = palette.insert(value);
                        if id >= (1u32 << ids.bits) {
                            Self::grow_local(ids, palette, id + 1);
                        }
                        id
                    }
                };

                palette.bump_count(new_id, 1);
                palette.bump_count(old_id, -1);
                ids.set(index, new_id);
                old_value
            }
            Self::Global { ids } => {
                V::from_global_id(ids.set(index, u32::from(value.global_id())) as u16)
            }
        }
    }

    fn grow_local(ids: &mut BitStorage, palette: &LocalPalette<V>, needed_ids: u32) {
        let new_bits = bits_for_distinct_count(needed_ids as usize);
        let mut grown = BitStorage::new(new_bits, ids.len);
        for (index, id) in ids.iter().enumerate() {
            grown.set(index, id);
        }
        let _ = palette;
        *ids = grown;
    }

    fn values(&self) -> Box<dyn Iterator<Item = V> + '_> {
        match self {
            Self::Local { ids, palette } => Box::new(ids.iter().map(|id| palette.value_at(id))),
            Self::Global { ids } => Box::new(ids.iter().map(|id| V::from_global_id(id as u16))),
        }
    }

    fn maybe_has(&self, mut predicate: impl FnMut(V) -> bool) -> bool {
        match self {
            Self::Local { palette, .. } => palette.entries.iter().any(|&(v, _)| predicate(v)),
            Self::Global { ids } => ids.iter().any(|id| predicate(V::from_global_id(id as u16))),
        }
    }
}

const fn bits_for_distinct_count(count: usize) -> u32 {
    if count <= 1 {
        1
    } else {
        (usize::BITS - (count - 1).leading_zeros()).max(1)
    }
}

#[derive(Debug, Clone)]
pub enum PalettedContainer<V: PackedValue + Debug, const DIM: usize> {
    Homogeneous(V),
    Heterogeneous(Box<Packed<V>>),
    Building(Box<Cube<V, DIM>>),
}

impl<V: PackedValue + Debug, const DIM: usize> PalettedContainer<V, DIM> {
    pub const SIZE: usize = DIM;
    pub const VOLUME: usize = DIM * DIM * DIM;

    const fn index_of(x: usize, y: usize, z: usize) -> usize {
        (y * DIM + z) * DIM + x
    }

    pub fn from_cube(cube: &Cube<V, DIM>) -> Self {
        let flat: Vec<V> = cube.iter().flatten().flatten().copied().collect();

        if flat.iter().all(|&v| v == flat[0]) {
            return Self::Homogeneous(flat[0]);
        }

        Self::Heterogeneous(Box::new(Packed::from_values(&flat, Self::VOLUME)))
    }

    pub fn get(&self, x: usize, y: usize, z: usize) -> V {
        debug_assert!(x < DIM && y < DIM && z < DIM);
        match self {
            Self::Homogeneous(value) => *value,
            Self::Heterogeneous(packed) => packed.get(Self::index_of(x, y, z)),
            Self::Building(cube) => cube[y][z][x],
        }
    }

    pub fn get_at_index(&self, index: usize) -> V {
        debug_assert!(index < Self::VOLUME);
        match self {
            Self::Homogeneous(value) => *value,
            Self::Heterogeneous(packed) => packed.get(index),
            Self::Building(cube) => {
                let y = index / (DIM * DIM);
                let z = (index / DIM) % DIM;
                let x = index % DIM;
                cube[y][z][x]
            }
        }
    }

    pub(crate) fn copy_column_into(&self, x: usize, z: usize, out: &mut [V]) {
        debug_assert!(x < DIM);
        debug_assert!(z < DIM);
        debug_assert!(out.len() >= DIM);

        for (y, slot) in out[..DIM].iter_mut().enumerate() {
            *slot = self.get(x, y, z);
        }
    }

    pub fn maybe_has(&self, mut predicate: impl FnMut(V) -> bool) -> bool {
        match self {
            Self::Homogeneous(value) => predicate(*value),
            Self::Heterogeneous(packed) => packed.maybe_has(predicate),
            Self::Building(cube) => cube
                .iter()
                .flatten()
                .flatten()
                .any(|value| predicate(*value)),
        }
    }

    pub fn collect_values(&self) -> Vec<V> {
        match self {
            Self::Homogeneous(value) => vec![*value; Self::VOLUME],
            Self::Heterogeneous(packed) => packed.values().collect(),
            Self::Building(cube) => cube.iter().flatten().flatten().copied().collect(),
        }
    }

    pub fn enter_building_mode(&mut self) {
        match self {
            Self::Building(_) => {}
            Self::Homogeneous(value) => {
                let cube: Box<Cube<V, DIM>> = Box::new([[[*value; DIM]; DIM]; DIM]);
                *self = Self::Building(cube);
            }
            Self::Heterogeneous(packed) => {
                let mut cube: Box<Cube<V, DIM>> = Box::new([[[V::default(); DIM]; DIM]; DIM]);
                for index in 0..Self::VOLUME {
                    let y = index / (DIM * DIM);
                    let z = (index / DIM) % DIM;
                    let x = index % DIM;
                    cube[y][z][x] = packed.get(index);
                }
                *self = Self::Building(cube);
            }
        }
    }

    #[inline]
    pub fn as_building_slice_mut(&mut self) -> Option<&mut [V]> {
        if let Self::Building(cube) = self {
            // SAFETY: `[[[V; DIM]; DIM]; DIM]` is a contiguous array of
            // `DIM*DIM*DIM` `V`s; the cast preserves the live `&mut Box`'s
            // borrow because the returned slice cannot outlive `self`.
            Some(unsafe {
                slice::from_raw_parts_mut(cube.as_mut_ptr().cast::<V>(), DIM * DIM * DIM)
            })
        } else {
            None
        }
    }

    pub fn finalize_building(&mut self) {
        if !matches!(self, Self::Building(_)) {
            return;
        }
        let taken = mem::replace(self, Self::Homogeneous(V::default()));
        let Self::Building(cube) = taken else {
            unreachable!()
        };
        *self = Self::from_cube(&cube);
    }

    pub fn set(&mut self, x: usize, y: usize, z: usize, value: V) -> V {
        debug_assert!(x < Self::SIZE);
        debug_assert!(y < Self::SIZE);
        debug_assert!(z < Self::SIZE);

        match self {
            Self::Homogeneous(original) => {
                let original = *original;
                if value != original {
                    let mut cube: Box<Cube<V, DIM>> = Box::new([[[original; DIM]; DIM]; DIM]);
                    cube[y][z][x] = value;
                    *self = Self::from_cube(&cube);
                }
                original
            }
            Self::Heterogeneous(packed) => {
                let index = Self::index_of(x, y, z);
                let old = packed.set(index, value);
                if let Packed::Local { palette, .. } = packed.as_ref()
                    && palette.entries.iter().filter(|(_, count)| *count > 0).count() == 1
                {
                    let (only_value, _) = palette
                        .entries
                        .iter()
                        .find(|(_, count)| *count > 0)
                        .expect("checked above");
                    *self = Self::Homogeneous(*only_value);
                }
                old
            }
            Self::Building(cube) => {
                let old = cube[y][z][x];
                cube[y][z][x] = value;
                old
            }
        }
    }
}

pub type BlockPalette = PalettedContainer<BlockState, 16>;
pub type BiomePalette = PalettedContainer<u16, 4>;

impl BlockPalette {
    pub fn non_empty_block_count(&self) -> u16 {
        match self {
            Self::Homogeneous(v) => {
                if v.0 == 0 { 0 } else { Self::VOLUME as u16 }
            }
            Self::Heterogeneous(packed) => match packed.as_ref() {
                Packed::Local { palette, .. } => palette
                    .entries
                    .iter()
                    .filter(|(v, _)| v.0 != 0)
                    .map(|&(_, count)| count)
                    .sum(),
                Packed::Global { ids } => {
                    ids.iter().filter(|&id| id != 0).count() as u16
                }
            },
            Self::Building(cube) => {
                let mut count: u16 = 0;
                for slab in cube {
                    for row in slab {
                        for v in row {
                            if v.0 != 0 {
                                count += 1;
                            }
                        }
                    }
                }
                count
            }
        }
    }

    pub fn has_only_air(&self) -> bool {
        match self {
            Self::Homogeneous(v) => v.is_air(),
            Self::Heterogeneous(_) => false,
            Self::Building(cube) => cube.iter().flatten().flatten().all(BlockState::is_air),
        }
    }

    pub fn palette_counts(&self) -> Vec<(BlockState, u16)> {
        match self {
            Self::Homogeneous(v) => vec![(*v, Self::VOLUME as u16)],
            Self::Heterogeneous(packed) => match packed.as_ref() {
                Packed::Local { palette, .. } => palette
                    .entries
                    .iter()
                    .filter(|(_, count)| *count > 0)
                    .copied()
                    .collect(),
                Packed::Global { ids } => {
                    let mut counts: Vec<(BlockState, u16)> = Vec::new();
                    for id in ids.iter() {
                        let value = BlockState::from_global_id(id as u16);
                        if let Some(entry) = counts.iter_mut().find(|(v, _)| *v == value) {
                            entry.1 += 1;
                        } else {
                            counts.push((value, 1));
                        }
                    }
                    counts
                }
            },
            Self::Building(_) => unreachable!("finalize_building must be called first"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BlockPalette;
    use crate::util::ident::BlockState;

    fn assert_column_matches_get(container: &BlockPalette, x: usize, z: usize) {
        let mut column = [BlockState::default(); 16];
        container.copy_column_into(x, z, &mut column);
        for (y, state) in column.into_iter().enumerate() {
            assert_eq!(state, container.get(x, y, z));
        }
    }

    #[test]
    fn get_at_index_reads_homogeneous_palette_values() {
        let container = BlockPalette::Homogeneous(BlockState(42));

        assert_eq!(container.get_at_index(0), BlockState(42));
        assert_eq!(
            container.get_at_index(BlockPalette::VOLUME - 1),
            BlockState(42)
        );
    }

    #[test]
    fn get_at_index_uses_y_z_x_linear_order_for_blocks() {
        let mut container = BlockPalette::Homogeneous(BlockState(0));

        container.set(3, 4, 5, BlockState(99));

        assert_eq!(
            container.get_at_index(3 + 5 * 16 + 4 * 16 * 16),
            BlockState(99)
        );
    }

    #[test]
    fn maybe_has_checks_palette_values_without_scanning_cells() {
        let mut container = BlockPalette::Homogeneous(BlockState(0));
        assert!(!container.maybe_has(|state| state == BlockState(7)));

        container.set(1, 2, 3, BlockState(7));
        assert!(container.maybe_has(|state| state == BlockState(7)));
        assert!(!container.maybe_has(|state| state == BlockState(9)));
    }

    #[test]
    fn copy_column_into_matches_get_for_homogeneous_container() {
        let container = BlockPalette::Homogeneous(BlockState(7));
        assert_column_matches_get(&container, 3, 12);
    }

    #[test]
    fn copy_column_into_matches_get_for_heterogeneous_container() {
        let x = 5;
        let z = 9;
        let mut cube = Box::new([[[BlockState::default(); 16]; 16]; 16]);
        for y in 0..16 {
            cube[y][z][x] = BlockState((y + 1) as u16);
        }

        let container = BlockPalette::from_cube(&cube);
        assert_column_matches_get(&container, x, z);
    }

    #[test]
    fn copy_column_into_matches_get_for_building_container() {
        let x = 11;
        let z = 2;
        let mut container = BlockPalette::Homogeneous(BlockState(3));
        container.enter_building_mode();
        for y in 0..16 {
            container.set(x, y, z, BlockState((31 + y) as u16));
        }

        assert_column_matches_get(&container, x, z);
    }

    #[test]
    fn set_reverts_to_homogeneous_when_only_one_distinct_value_remains() {
        let mut container = BlockPalette::Homogeneous(BlockState(1));
        container.set(0, 0, 0, BlockState(2));
        assert!(matches!(container, BlockPalette::Heterogeneous(_)));
        container.set(0, 0, 0, BlockState(1));
        assert!(matches!(container, BlockPalette::Homogeneous(BlockState(1))));
    }

    #[test]
    fn local_palette_grows_past_its_initial_bit_width() {
        let mut container = BlockPalette::Homogeneous(BlockState(0));
        for i in 1..100u16 {
            container.set((i % 16) as usize, (i / 16 % 16) as usize, 0, BlockState(i));
        }
        for i in 1..100u16 {
            assert_eq!(
                container.get((i % 16) as usize, (i / 16 % 16) as usize, 0),
                BlockState(i)
            );
        }
    }

    #[test]
    fn global_palette_fallback_past_cutover_round_trips_values() {
        let mut cube = Box::new([[[BlockState::default(); 16]; 16]; 16]);
        let mut n = 0u16;
        for slab in cube.iter_mut() {
            for row in slab.iter_mut() {
                for cell in row.iter_mut() {
                    *cell = BlockState(n % 300);
                    n += 1;
                }
            }
        }
        let container = BlockPalette::from_cube(&cube);
        assert!(matches!(
            container,
            BlockPalette::Heterogeneous(_)
        ));
        for y in 0..16 {
            for z in 0..16 {
                for x in 0..16 {
                    assert_eq!(container.get(x, y, z), cube[y][z][x]);
                }
            }
        }
    }
}
