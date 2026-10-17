use crate::world::worldgen::structure::pools::structure_template_pool::Projection;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JigsawJunction {
    pub source_x: i32,
    pub source_ground_y: i32,
    pub source_z: i32,
    pub delta_y: i32,
    pub dest_projection: Projection,
}

impl JigsawJunction {
    pub const fn new(
        source_x: i32,
        source_ground_y: i32,
        source_z: i32,
        delta_y: i32,
        dest_projection: Projection,
    ) -> Self {
        Self {
            source_x,
            source_ground_y,
            source_z,
            delta_y,
            dest_projection,
        }
    }
}
