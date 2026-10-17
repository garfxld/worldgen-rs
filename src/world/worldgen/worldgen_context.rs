#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldGenContext {
    min_y: i32,
    height: i32,
    sea_level: i32,
}

impl WorldGenContext {
    pub const fn new(
        generator_min_y: i32,
        generator_gen_depth: i32,
        generator_sea_level: i32,
        level_min_y: i32,
        level_height: i32,
    ) -> Self {
        Self {
            min_y: if level_min_y > generator_min_y {
                level_min_y
            } else {
                generator_min_y
            },
            height: if level_height < generator_gen_depth {
                level_height
            } else {
                generator_gen_depth
            },
            sea_level: generator_sea_level,
        }
    }

    pub const fn get_min_gen_y(&self) -> i32 {
        self.min_y
    }

    pub const fn get_gen_depth(&self) -> i32 {
        self.height
    }

    pub const fn sea_level(&self) -> i32 {
        self.sea_level
    }
}
