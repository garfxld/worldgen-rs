use std::f32::consts::PI;

use glam::DVec3;
use serde::Deserialize;

use crate::chunk::heightmap::HeightmapType;
use crate::registry::blocks::vanilla_blocks;
use crate::util::ident::Identifier;
use crate::util::math;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::util::value_providers::FloatProvider;
use crate::util::value_providers::int::IntProvider;
use crate::worldgen::block_predicate::HolderSet;
use crate::worldgen::column::Column;
use crate::worldgen::feature::features::speleothem::{
    get_speleothem_height, is_base_or_lava, is_circle_mostly_embedded_in_stone, is_empty_or_water,
    is_empty_or_water_or_lava,
};
use crate::worldgen::region::WorldGenRegion;

const BASE_STONE_OVERWORLD: Identifier = Identifier::const_vanilla("base_stone_overworld");

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LargeDripstoneConfig {
    #[serde(default = "default_floor_to_ceiling_search_range")]
    pub floor_to_ceiling_search_range: i32,
    pub column_radius: IntProvider,
    pub height_scale: FloatProvider,
    pub max_column_radius_to_cave_height_ratio: f32,
    pub stalactite_bluntness: FloatProvider,
    pub stalagmite_bluntness: FloatProvider,
    pub wind_speed: FloatProvider,
    pub min_radius_for_wind: i32,
    pub min_bluntness_for_wind: f32,
    pub replaceable_blocks: HolderSet,
}

const fn default_floor_to_ceiling_search_range() -> i32 {
    30
}

struct LargeDripstone {
    root: BlockPos,
    pointing_up: bool,
    radius: i32,
    bluntness: f64,
    scale: f64,
}

struct WindOffsetter {
    origin_y: i32,
    wind_speed: Option<DVec3>,
    max_offset: i32,
}

impl WindOffsetter {
    fn new(
        origin_y: i32,
        random: &mut RandomSource,
        wind_speed_range: &FloatProvider,
        max_offset: i32,
    ) -> Self {
        let speed = wind_speed_range.sample(random);
        let direction = random.next_f32() * PI;
        Self {
            origin_y,
            wind_speed: Some(DVec3::new(
                f64::from(math::cos(direction) * speed),
                0.0,
                f64::from(math::sin(direction) * speed),
            )),
            max_offset,
        }
    }

    const fn no_wind() -> Self {
        Self {
            origin_y: 0,
            wind_speed: None,
            max_offset: 0,
        }
    }

    fn offset(&self, pos: BlockPos) -> BlockPos {
        let Some(wind_speed) = self.wind_speed else {
            return pos;
        };

        let dy = self.origin_y - pos.y();
        let total_wind_adjust = wind_speed * f64::from(dy);
        let dx = math::floor_f64(total_wind_adjust.x).clamp(-self.max_offset, self.max_offset);
        let dz = math::floor_f64(total_wind_adjust.z).clamp(-self.max_offset, self.max_offset);
        pos.offset(dx, 0, dz)
    }
}

impl LargeDripstone {
    fn get_height(&self) -> i32 {
        self.get_height_at_radius(0.0)
    }

    fn get_height_at_radius(&self, check_radius: f32) -> i32 {
        get_speleothem_height(
            check_radius as f64,
            self.radius as f64,
            self.scale,
            self.bluntness,
        ) as i32
    }

    fn move_back_until_base_is_inside_stone_and_shrink_radius_if_necessary(
        &mut self,
        region: &WorldGenRegion<'_>,
        wind: &WindOffsetter,
    ) -> bool {
        while self.radius > 1 {
            let mut new_root = self.root;
            let max_tries = 10.min(self.get_height());

            for _ in 0..max_tries {
                if region.get_block(new_root).get_block() == &vanilla_blocks::LAVA {
                    return false;
                }

                if is_circle_mostly_embedded_in_stone(region, wind.offset(new_root), self.radius) {
                    self.root = new_root;
                    return true;
                }

                new_root = if self.pointing_up {
                    new_root.below()
                } else {
                    new_root.above()
                };
            }

            self.radius /= 2;
        }

        false
    }

    fn place_blocks(
        &self,
        region: &WorldGenRegion<'_>,
        random: &mut RandomSource,
        wind: &WindOffsetter,
    ) {
        let dripstone = vanilla_blocks::DRIPSTONE_BLOCK.default_state();

        for dx in -self.radius..=self.radius {
            for dz in -self.radius..=self.radius {
                let current_radius = ((dx * dx + dz * dz) as f32).sqrt();
                if current_radius > self.radius as f32 {
                    continue;
                }

                let mut height = self.get_height_at_radius(current_radius);
                if height <= 0 {
                    continue;
                }

                if random.next_f32() < 0.2 {
                    height = (height as f32 * random_between(random, 0.8, 1.0)) as i32;
                }

                let mut pos = self.root.offset(dx, 0, dz);
                let mut has_been_out_of_stone = false;
                let max_y = if self.pointing_up {
                    region.get_height(HeightmapType::WorldSurfaceWg, pos.x(), pos.z())
                } else {
                    i32::MAX
                };

                let mut i = 0;
                while i < height && pos.y() < max_y {
                    let wind_adjusted_pos = wind.offset(pos);
                    if is_empty_or_water_or_lava(region.get_block(wind_adjusted_pos)) {
                        has_been_out_of_stone = true;
                        region.set_block(wind_adjusted_pos, dripstone);
                    } else if has_been_out_of_stone
                        && region
                            .get_block(wind_adjusted_pos)
                            .get_block()
                            .has_tag(&BASE_STONE_OVERWORLD)
                    {
                        break;
                    }

                    pos = if self.pointing_up {
                        pos.above()
                    } else {
                        pos.below()
                    };
                    i += 1;
                }
            }
        }
    }

    fn is_suitable_for_wind(&self, config: &LargeDripstoneConfig) -> bool {
        self.radius >= config.min_radius_for_wind
            && self.bluntness >= f64::from(config.min_bluntness_for_wind)
    }
}

fn random_between(random: &mut RandomSource, min: f32, max: f32) -> f32 {
    random.next_f32() * (max - min) + min
}

fn make_dripstone(
    root: BlockPos,
    pointing_up: bool,
    random: &mut RandomSource,
    radius: i32,
    bluntness: &FloatProvider,
    height_scale: &FloatProvider,
) -> LargeDripstone {
    LargeDripstone {
        root,
        pointing_up,
        radius,
        bluntness: f64::from(bluntness.sample(random)),
        scale: f64::from(height_scale.sample(random)),
    }
}

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPos,
    config: &LargeDripstoneConfig,
) -> bool {
    if !is_empty_or_water(region.get_block(origin)) {
        return false;
    }

    let Some(column) = Column::scan(
        region,
        origin,
        config.floor_to_ceiling_search_range,
        is_empty_or_water,
        |state| {
            is_base_or_lava(
                state,
                &vanilla_blocks::DRIPSTONE_BLOCK,
                &config.replaceable_blocks,
            )
        },
    ) else {
        return false;
    };

    let (Some(floor), Some(ceiling)) = (column.floor(), column.ceiling()) else {
        return false;
    };

    let Some(column_height) = column.height() else {
        return false;
    };
    if column_height < 4 {
        return false;
    }

    let max_column_radius_based_on_column_height =
        (column_height as f32 * config.max_column_radius_to_cave_height_ratio) as i32;
    let max_column_radius = max_column_radius_based_on_column_height
        .clamp(config.column_radius.min(), config.column_radius.max());
    let radius = random.next_i32_between(config.column_radius.min(), max_column_radius);

    let mut stalactite = make_dripstone(
        origin.at_y(ceiling - 1),
        false,
        random,
        radius,
        &config.stalactite_bluntness,
        &config.height_scale,
    );
    let mut stalagmite = make_dripstone(
        origin.at_y(floor + 1),
        true,
        random,
        radius,
        &config.stalagmite_bluntness,
        &config.height_scale,
    );

    let wind = if stalactite.is_suitable_for_wind(config) && stalagmite.is_suitable_for_wind(config)
    {
        WindOffsetter::new(origin.y(), random, &config.wind_speed, 16 - radius)
    } else {
        WindOffsetter::no_wind()
    };

    let stalactite_base_embedded_in_stone =
        stalactite.move_back_until_base_is_inside_stone_and_shrink_radius_if_necessary(region, &wind);
    let stalagmite_base_embedded_in_stone =
        stalagmite.move_back_until_base_is_inside_stone_and_shrink_radius_if_necessary(region, &wind);

    if stalactite_base_embedded_in_stone {
        stalactite.place_blocks(region, random, &wind);
    }

    if stalagmite_base_embedded_in_stone {
        stalagmite.place_blocks(region, random, &wind);
    }

    true
}
