use crate::chunk::heightmap::HeightmapType;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum ChunkStatus {
    Empty,
    StructureStarts,
    StructureReferences,
    Biomes,
    Noise,
    Surface,
    Carvers,
    Features,
    InitializeLight,
    Light,
    Spawn,
    Full,
}

impl ChunkStatus {
    pub const fn get_index(self) -> usize {
        self as usize
    }

    pub const fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::Empty),
            1 => Some(Self::StructureStarts),
            2 => Some(Self::StructureReferences),
            3 => Some(Self::Biomes),
            4 => Some(Self::Noise),
            5 => Some(Self::Surface),
            6 => Some(Self::Carvers),
            7 => Some(Self::Features),
            8 => Some(Self::InitializeLight),
            9 => Some(Self::Light),
            10 => Some(Self::Spawn),
            11 => Some(Self::Full),
            _ => None,
        }
    }

    pub const fn next(self) -> Option<Self> {
        match self {
            Self::Empty => Some(Self::StructureStarts),
            Self::StructureStarts => Some(Self::StructureReferences),
            Self::StructureReferences => Some(Self::Biomes),
            Self::Biomes => Some(Self::Noise),
            Self::Noise => Some(Self::Surface),
            Self::Surface => Some(Self::Carvers),
            Self::Carvers => Some(Self::Features),
            Self::Features => Some(Self::InitializeLight),
            Self::InitializeLight => Some(Self::Light),
            Self::Light => Some(Self::Spawn),
            Self::Spawn => Some(Self::Full),
            Self::Full => None,
        }
    }

    pub const fn parent(self) -> Option<Self> {
        match self {
            Self::Empty => None,
            Self::StructureStarts => Some(Self::Empty),
            Self::StructureReferences => Some(Self::StructureStarts),
            Self::Biomes => Some(Self::StructureReferences),
            Self::Noise => Some(Self::Biomes),
            Self::Surface => Some(Self::Noise),
            Self::Carvers => Some(Self::Surface),
            Self::Features => Some(Self::Carvers),
            Self::InitializeLight => Some(Self::Features),
            Self::Light => Some(Self::InitializeLight),
            Self::Spawn => Some(Self::Light),
            Self::Full => Some(Self::Spawn),
        }
    }

    pub const fn heightmaps_after(self) -> &'static [HeightmapType] {
        match self {
            Self::Empty
            | Self::StructureStarts
            | Self::StructureReferences
            | Self::Biomes
            | Self::Noise
            | Self::Surface => HeightmapType::worldgen_types(),
            Self::Carvers
            | Self::Features
            | Self::InitializeLight
            | Self::Light
            | Self::Spawn
            | Self::Full => HeightmapType::final_types(),
        }
    }

    pub const fn requirements(self) -> &'static [(ChunkStatus, usize)] {
        match self {
            Self::Empty | Self::StructureStarts | Self::InitializeLight | Self::Full => &[],
            Self::StructureReferences | Self::Biomes | Self::Carvers => {
                &[(Self::StructureStarts, 8)]
            }
            Self::Noise | Self::Surface => &[(Self::StructureStarts, 8), (Self::Biomes, 1)],
            Self::Features => &[(Self::StructureStarts, 8), (Self::Carvers, 1)],
            Self::Light => &[(Self::InitializeLight, 1)],
            Self::Spawn => &[(Self::Biomes, 1)],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_roundtrips() {
        for index in 0..=11 {
            let status = ChunkStatus::from_index(index).expect("valid index");
            assert_eq!(status.get_index(), index);
        }
        assert_eq!(ChunkStatus::from_index(12), None);
    }

    #[test]
    fn next_and_parent_are_inverses() {
        let mut status = ChunkStatus::Empty;
        while let Some(next) = status.next() {
            assert_eq!(next.parent(), Some(status));
            status = next;
        }
        assert_eq!(status, ChunkStatus::Full);
    }

    #[test]
    fn requirements_match_vanilla_generation_pyramid() {
        assert_eq!(ChunkStatus::Empty.requirements(), &[]);
        assert_eq!(
            ChunkStatus::StructureReferences.requirements(),
            &[(ChunkStatus::StructureStarts, 8)]
        );
        assert_eq!(
            ChunkStatus::Noise.requirements(),
            &[(ChunkStatus::StructureStarts, 8), (ChunkStatus::Biomes, 1)]
        );
        assert_eq!(
            ChunkStatus::Features.requirements(),
            &[(ChunkStatus::StructureStarts, 8), (ChunkStatus::Carvers, 1)]
        );
        assert_eq!(
            ChunkStatus::Light.requirements(),
            &[(ChunkStatus::InitializeLight, 1)]
        );
        assert_eq!(
            ChunkStatus::Spawn.requirements(),
            &[(ChunkStatus::Biomes, 1)]
        );
    }
}
