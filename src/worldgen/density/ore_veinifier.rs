use crate::util::ident::BlockStateId;

#[derive(Default)]
pub struct OreVeinifier;

impl OreVeinifier {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    #[must_use]
    pub const fn compute_interpolated(
        &self,
        _world_x: i32,
        _world_y: i32,
        _world_z: i32,
    ) -> Option<BlockStateId> {
        None
    }
}
