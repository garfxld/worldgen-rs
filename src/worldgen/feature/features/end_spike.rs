use std::f64::consts::PI;

use serde::Deserialize;

use crate::registry::blocks::properties::Properties;
use crate::registry::blocks::vanilla_blocks;
use crate::util::math::floor_f64;
use crate::util::pos::{BlockPos, SectionPos};
use crate::util::rng::RandomSource;
use crate::util::rng::legacy_random::LegacyRandomSource;
use crate::util::shuffle;
use crate::worldgen::feature::features::blockpos_iter::between_closed;
use crate::worldgen::region::WorldGenRegion;

#[derive(Debug, Deserialize)]
pub struct EndSpikeConfig {
    #[serde(default)]
    pub crystal_invulnerable: bool,
    #[serde(default)]
    pub spikes: Vec<EndSpike>,
    #[serde(default)]
    pub crystal_beam_target: Option<[i32; 3]>,
}

#[derive(Debug, Deserialize)]
pub struct EndSpike {
    #[serde(default, rename = "centerX")]
    pub center_x: i32,
    #[serde(default, rename = "centerZ")]
    pub center_z: i32,
    #[serde(default)]
    pub radius: i32,
    #[serde(default)]
    pub height: i32,
    #[serde(default)]
    pub guarded: bool,
}

fn spikes_for_level(region: &WorldGenRegion<'_>) -> Vec<EndSpike> {
    let mut random = RandomSource::Legacy(LegacyRandomSource::from_seed(region.seed() as u64));
    let key = random.next_i64() & 65535;

    let mut sizes: Vec<i32> = (0..10).collect();
    let mut size_random = RandomSource::Legacy(LegacyRandomSource::from_seed(key as u64));
    shuffle(&mut sizes, &mut size_random);

    (0..10)
        .map(|i| {
            let angle = 2.0 * (-PI + (PI / 10.0) * f64::from(i));
            let x = floor_f64(42.0 * angle.cos());
            let z = floor_f64(42.0 * angle.sin());
            let size = sizes[i as usize];
            EndSpike {
                center_x: x,
                center_z: z,
                radius: 2 + size / 3,
                height: 76 + size * 3,
                guarded: size == 1 || size == 2,
            }
        })
        .collect()
}

const fn is_center_within_chunk(spike: &EndSpike, chunk_origin: BlockPos) -> bool {
    SectionPos::block_to_section_coord(chunk_origin.x())
        == SectionPos::block_to_section_coord(spike.center_x)
        && SectionPos::block_to_section_coord(chunk_origin.z())
            == SectionPos::block_to_section_coord(spike.center_z)
}

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPos,
    config: &EndSpikeConfig,
) -> bool {
    let generated;
    let spikes = if config.spikes.is_empty() {
        generated = spikes_for_level(region);
        &generated
    } else {
        &config.spikes
    };

    for spike in spikes {
        if is_center_within_chunk(spike, origin) {
            place_spike(region, random, spike);
        }
    }

    true
}

fn place_spike(region: &WorldGenRegion<'_>, random: &mut RandomSource, spike: &EndSpike) {
    let radius = spike.radius;
    let obsidian = vanilla_blocks::OBSIDIAN.default_state();
    let air = vanilla_blocks::AIR.default_state();

    for pos in between_closed(
        BlockPos::new(
            spike.center_x - radius,
            region.min_y(),
            spike.center_z - radius,
        ),
        BlockPos::new(
            spike.center_x + radius,
            spike.height + 10,
            spike.center_z + radius,
        ),
    ) {
        let dx = f64::from(pos.x() - spike.center_x);
        let dz = f64::from(pos.z() - spike.center_z);
        if dx * dx + dz * dz <= f64::from(radius * radius + 1) && pos.y() < spike.height {
            region.set_block(pos, obsidian);
        } else if pos.y() > 65 {
            region.set_block(pos, air);
        }
    }

    if spike.guarded {
        for dx in -2i32..=2 {
            for dz in -2i32..=2 {
                for dy in 0..=3 {
                    let is_x_side = dx.abs() == 2;
                    let is_z_side = dz.abs() == 2;
                    let top = dy == 3;
                    if is_x_side || is_z_side || top {
                        let x_edge = dx == -2 || dx == 2 || top;
                        let z_edge = dz == -2 || dz == 2 || top;
                        let state = vanilla_blocks::IRON_BARS
                            .default_state()
                            .set_value(&Properties::NORTH, x_edge && dz != -2)
                            .set_value(&Properties::SOUTH, x_edge && dz != 2)
                            .set_value(&Properties::WEST, z_edge && dx != -2)
                            .set_value(&Properties::EAST, z_edge && dx != 2);
                        region.set_block(
                            BlockPos::new(
                                spike.center_x + dx,
                                spike.height + dy,
                                spike.center_z + dz,
                            ),
                            state,
                        );
                    }
                }
            }
        }
    }

    random.next_f32();
    let crystal_pos = BlockPos::new(spike.center_x, spike.height + 1, spike.center_z);
    region.set_block(crystal_pos.below(), vanilla_blocks::BEDROCK.default_state());
    region.set_block(crystal_pos, vanilla_blocks::FIRE.default_state());
}
