use glam::DVec3;

use crate::util::pos::BlockPos;
use crate::util::rng::get_seed;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OffsetType {
    None,
    Xz,
    Xyz,
}

#[derive(Debug)]
pub struct BlockConfig {
    pub can_occlude: bool,
    pub dynamic_shape: bool,
    pub offset_type: OffsetType,
    pub max_horizontal_offset: f32,
    pub max_vertical_offset: f32,
    pub liquid: bool,
    pub is_air: bool,
    pub replaceable: bool,
}

impl BlockConfig {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            can_occlude: true,
            dynamic_shape: false,
            offset_type: OffsetType::None,
            max_horizontal_offset: 0.25,
            max_vertical_offset: 0.2,
            liquid: false,
            is_air: false,
            replaceable: false,
        }
    }

    #[must_use]
    pub const fn can_occlude(mut self, can_occlude: bool) -> Self {
        self.can_occlude = can_occlude;
        self
    }

    #[must_use]
    pub const fn dynamic_shape(mut self, dynamic: bool) -> Self {
        self.dynamic_shape = dynamic;
        self
    }

    #[must_use]
    pub const fn offset_type(mut self, offset_type: OffsetType) -> Self {
        self.offset_type = offset_type;
        self
    }

    #[must_use]
    pub const fn max_horizontal_offset(mut self, offset: f32) -> Self {
        self.max_horizontal_offset = offset;
        self
    }

    #[must_use]
    pub const fn max_vertical_offset(mut self, offset: f32) -> Self {
        self.max_vertical_offset = offset;
        self
    }

    #[must_use]
    pub const fn liquid(mut self, liquid: bool) -> Self {
        self.liquid = liquid;
        self
    }

    #[must_use]
    pub const fn set_is_air(mut self, is_air: bool) -> Self {
        self.is_air = is_air;
        self
    }

    #[must_use]
    pub const fn replaceable(mut self, replaceable: bool) -> Self {
        self.replaceable = replaceable;
        self
    }

    #[must_use]
    pub fn offset_at(&self, pos: BlockPos) -> DVec3 {
        let seed = get_seed(pos.x(), 0, pos.z());
        let x = horizontal_offset_component(seed & 15, self.max_horizontal_offset);
        let z = horizontal_offset_component((seed >> 8) & 15, self.max_horizontal_offset);

        match self.offset_type {
            OffsetType::None => DVec3::ZERO,
            OffsetType::Xz => DVec3::new(x, 0.0, z),
            OffsetType::Xyz => {
                let y = (f64::from(((seed >> 4) & 15) as f32 / 15.0) - 1.0)
                    * f64::from(self.max_vertical_offset);
                DVec3::new(x, y, z)
            }
        }
    }
}

impl Default for BlockConfig {
    fn default() -> Self {
        Self::new()
    }
}

fn horizontal_offset_component(seed_bits: i64, max_horizontal_offset: f32) -> f64 {
    let raw_offset = (f64::from(seed_bits as f32 / 15.0) - 0.5) * 0.5;
    raw_offset.clamp(
        -f64::from(max_horizontal_offset),
        f64::from(max_horizontal_offset),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_offset(config: &BlockConfig, pos: BlockPos, expected: DVec3) {
        let offset = config.offset_at(pos);
        assert!((offset - expected).length() < 1.0e-7);
    }

    #[test]
    fn xz_offset_matches_vanilla_clamping() {
        let config = BlockConfig::new()
            .offset_type(OffsetType::Xz)
            .max_horizontal_offset(0.125);

        assert_offset(&config, BlockPos::ZERO, DVec3::new(-0.125, 0.0, -0.125));
        assert_offset(
            &config,
            BlockPos::new(12, 64, 34),
            DVec3::new(0.125, 0.0, 0.125),
        );
    }

    #[test]
    fn xz_offset_uses_block_position_seed_without_y() {
        let config = BlockConfig::new()
            .offset_type(OffsetType::Xz)
            .max_horizontal_offset(0.125);

        assert_offset(
            &config,
            BlockPos::new(1, 64, 0),
            DVec3::new(0.116_666_666_666_666_64, 0.0, -0.016_666_666_666_666_663),
        );
        assert_offset(
            &config,
            BlockPos::new(-5, 12, 7),
            DVec3::new(-0.049_999_999_999_999_99, 0.0, 0.116_666_666_666_666_64),
        );
    }

    #[test]
    fn xyz_offset_applies_vanilla_vertical_component() {
        let config = BlockConfig::new()
            .offset_type(OffsetType::Xyz)
            .max_horizontal_offset(0.25)
            .max_vertical_offset(0.1);

        assert_offset(&config, BlockPos::ZERO, DVec3::new(-0.25, -0.1, -0.25));
        assert_offset(
            &config,
            BlockPos::new(3, 40, 9),
            DVec3::new(
                -0.049_999_999_999_999_99,
                -0.066_666_666_666_666_68,
                -0.049_999_999_999_999_99,
            ),
        );
    }
}
