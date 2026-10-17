use crate::world::chunk::heightmap::HeightmapType;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum ChunkStatus {
    Empty,
    StructureStarts,
    StructureReferences,
    Biomes,
    Terrain,
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

    pub const fn next(self) -> Option<Self> {
        match self {
            Self::Empty => Some(Self::StructureStarts),
            Self::StructureStarts => Some(Self::StructureReferences),
            Self::StructureReferences => Some(Self::Biomes),
            Self::Biomes => Some(Self::Terrain),
            Self::Terrain => Some(Self::Features),
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
            Self::Terrain => Some(Self::Biomes),
            Self::Features => Some(Self::Terrain),
            Self::InitializeLight => Some(Self::Features),
            Self::Light => Some(Self::InitializeLight),
            Self::Spawn => Some(Self::Light),
            Self::Full => Some(Self::Spawn),
        }
    }

    pub const fn heightmaps_after(self) -> &'static [HeightmapType] {
        match self {
            Self::Empty | Self::StructureStarts | Self::StructureReferences | Self::Biomes => {
                HeightmapType::worldgen_types()
            }
            Self::Terrain
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
            Self::StructureReferences | Self::Biomes => &[(Self::StructureStarts, 8)],
            Self::Terrain => &[(Self::StructureStarts, 8), (Self::Biomes, 1)],
            Self::Features => &[(Self::StructureStarts, 8), (Self::Terrain, 1)],
            Self::Light => &[(Self::InitializeLight, 1)],
            Self::Spawn => &[(Self::Biomes, 1)],
        }
    }
}

impl TryFrom<usize> for ChunkStatus {
    type Error = ();

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => Self::Empty,
            1 => Self::StructureStarts,
            2 => Self::StructureReferences,
            3 => Self::Biomes,
            4 => Self::Terrain,
            5 => Self::Features,
            6 => Self::InitializeLight,
            7 => Self::Light,
            8 => Self::Spawn,
            9 => Self::Full,
            _ => return Err(()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_roundtrips() {
        for index in 0..=9 {
            let status = ChunkStatus::try_from(index).expect("valid index");
            assert_eq!(status.get_index(), index);
        }
        assert_eq!(ChunkStatus::try_from(10), Err(()));
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
            ChunkStatus::Terrain.requirements(),
            &[(ChunkStatus::StructureStarts, 8), (ChunkStatus::Biomes, 1)]
        );
        assert_eq!(
            ChunkStatus::Features.requirements(),
            &[(ChunkStatus::StructureStarts, 8), (ChunkStatus::Terrain, 1)]
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
