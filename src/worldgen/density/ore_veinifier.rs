use crate::util::ident::BlockState;

#[derive(Default)]
pub struct OreVeinifier;

impl OreVeinifier {
    pub const fn new() -> Self {
        Self
    }

    pub const fn compute_interpolated(
        &self,
        _world_x: i32,
        _world_y: i32,
        _world_z: i32,
    ) -> Option<BlockState> {
        None
    }
}
