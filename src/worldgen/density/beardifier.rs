use std::sync::LazyLock;

use crate::util::geometry::BoundingBox;
use crate::util::math::map_clamped;
use crate::util::pos::{BlockPos, ChunkPos};
use crate::worldgen::structure::pools::jigsaw_junction::JigsawJunction;
use crate::worldgen::structure::pools::structure_template_pool::Projection;
use crate::worldgen::structure::structure::TerrainAdjustment;
use crate::worldgen::structure::structure_piece::PieceKind;
use crate::worldgen::structure::structure_start::StructureStart;

pub const BEARD_KERNEL_RADIUS: i32 = 12;
const BEARD_KERNEL_SIZE: usize = 24;

static BEARD_KERNEL: LazyLock<[f32; BEARD_KERNEL_SIZE * BEARD_KERNEL_SIZE * BEARD_KERNEL_SIZE]> =
    LazyLock::new(|| {
        let mut kernel = [0.0f32; BEARD_KERNEL_SIZE * BEARD_KERNEL_SIZE * BEARD_KERNEL_SIZE];
        for zi in 0..BEARD_KERNEL_SIZE {
            for xi in 0..BEARD_KERNEL_SIZE {
                for yi in 0..BEARD_KERNEL_SIZE {
                    kernel[zi * BEARD_KERNEL_SIZE * BEARD_KERNEL_SIZE
                        + xi * BEARD_KERNEL_SIZE
                        + yi] = compute_beard_contribution(
                        xi as i32 - BEARD_KERNEL_RADIUS,
                        f64::from(yi as i32 - BEARD_KERNEL_RADIUS) + 0.5,
                        zi as i32 - BEARD_KERNEL_RADIUS,
                    ) as f32;
                }
            }
        }
        kernel
    });

#[derive(Debug, Clone, Copy)]
pub struct Rigid {
    pub box_: BoundingBox,
    pub terrain_adjustment: TerrainAdjustment,
    pub ground_level_delta: i32,
}

#[derive(Debug, Default)]
pub struct Beardifier {
    pieces: Vec<Rigid>,
    junctions: Vec<JigsawJunction>,
    affected_box: Option<BoundingBox>,
}

impl Beardifier {
    pub const fn empty() -> Self {
        Self {
            pieces: Vec::new(),
            junctions: Vec::new(),
            affected_box: None,
        }
    }

    pub fn for_structures_in_chunk(
        chunk_pos: ChunkPos,
        collect_starts: impl FnOnce(&mut dyn FnMut(&StructureStart)),
    ) -> Self {
        let chunk_start_block_x = chunk_pos.0.x * 16;
        let chunk_start_block_z = chunk_pos.0.y * 16;
        let mut pieces = Vec::new();
        let mut junctions = Vec::new();
        let mut any_piece_bounding_box: Option<BoundingBox> = None;

        collect_starts(&mut |start| {
            let Some(structure) = start.structure() else {
                return;
            };
            let terrain_adjustment = structure.terrain_adaptation();
            if terrain_adjustment == TerrainAdjustment::None {
                return;
            }

            for piece in start.pieces() {
                if !is_close_to_chunk(piece.bounding_box(), chunk_pos, BEARD_KERNEL_RADIUS) {
                    continue;
                }

                let PieceKind::PoolElement(pool_piece) = &piece.kind else {
                    pieces.push(Rigid {
                        box_: piece.bounding_box(),
                        terrain_adjustment,
                        ground_level_delta: 0,
                    });
                    any_piece_bounding_box = Some(include_bounding_box(
                        any_piece_bounding_box,
                        piece.bounding_box(),
                    ));
                    continue;
                };
                if pool_piece.element.projection() == Projection::Rigid {
                    pieces.push(Rigid {
                        box_: piece.bounding_box(),
                        terrain_adjustment,
                        ground_level_delta: pool_piece.ground_level_delta,
                    });
                    any_piece_bounding_box = Some(include_bounding_box(
                        any_piece_bounding_box,
                        piece.bounding_box(),
                    ));
                }

                for junction in &pool_piece.junctions {
                    let junction_x = junction.source_x;
                    let junction_z = junction.source_z;
                    if junction_x > chunk_start_block_x - BEARD_KERNEL_RADIUS
                        && junction_z > chunk_start_block_z - BEARD_KERNEL_RADIUS
                        && junction_x < chunk_start_block_x + 15 + BEARD_KERNEL_RADIUS
                        && junction_z < chunk_start_block_z + 15 + BEARD_KERNEL_RADIUS
                    {
                        junctions.push(*junction);
                        let junction_box = BoundingBox::from_point(BlockPos::new(
                            junction_x,
                            junction.source_ground_y,
                            junction_z,
                        ));
                        any_piece_bounding_box =
                            Some(include_bounding_box(any_piece_bounding_box, junction_box));
                    }
                }
            }
        });

        let Some(any_piece_bounding_box) = any_piece_bounding_box else {
            return Self::empty();
        };

        Self {
            pieces,
            junctions,
            affected_box: Some(any_piece_bounding_box.inflated_by(BEARD_KERNEL_SIZE as i32)),
        }
    }

    pub const fn is_empty(&self) -> bool {
        self.affected_box.is_none()
    }

    pub fn compute(&self, block_x: i32, block_y: i32, block_z: i32) -> f64 {
        let Some(affected_box) = self.affected_box else {
            return 0.0;
        };
        if !affected_box.is_inside(BlockPos::new(block_x, block_y, block_z)) {
            return 0.0;
        }

        let mut noise_value = 0.0;
        for rigid in &self.pieces {
            let box_ = rigid.box_;
            let ground_level_delta = rigid.ground_level_delta;
            let dx = 0.max((box_.min_x() - block_x).max(block_x - box_.max_x()));
            let dz = 0.max((box_.min_z() - block_z).max(block_z - box_.max_z()));
            let ground_y = box_.min_y() + ground_level_delta;
            let dy_to_ground = block_y - ground_y;

            let dy = match rigid.terrain_adjustment {
                TerrainAdjustment::None => 0,
                TerrainAdjustment::Bury | TerrainAdjustment::BeardThin => dy_to_ground,
                TerrainAdjustment::BeardBox => {
                    0.max((ground_y - block_y).max(block_y - box_.max_y()))
                }
                TerrainAdjustment::Encapsulate => {
                    0.max((box_.min_y() - block_y).max(block_y - box_.max_y()))
                }
            };

            noise_value += match rigid.terrain_adjustment {
                TerrainAdjustment::None => 0.0,
                TerrainAdjustment::Bury => {
                    get_bury_contribution(f64::from(dx), f64::from(dy) / 2.0, f64::from(dz))
                }
                TerrainAdjustment::BeardThin | TerrainAdjustment::BeardBox => {
                    get_beard_contribution(dx, dy, dz, dy_to_ground) * 0.8
                }
                TerrainAdjustment::Encapsulate => {
                    get_bury_contribution(
                        f64::from(dx) / 2.0,
                        f64::from(dy) / 2.0,
                        f64::from(dz) / 2.0,
                    ) * 0.8
                }
            };
        }

        for junction in &self.junctions {
            let dx = block_x - junction.source_x;
            let dy = block_y - junction.source_ground_y;
            let dz = block_z - junction.source_z;
            noise_value += get_beard_contribution(dx, dy, dz, dy) * 0.4;
        }

        noise_value
    }
}

const fn is_close_to_chunk(box_: BoundingBox, chunk_pos: ChunkPos, distance: i32) -> bool {
    let min_x = chunk_pos.0.x * 16;
    let min_z = chunk_pos.0.y * 16;
    box_.intersects_xz(
        min_x - distance,
        min_z - distance,
        min_x + 15 + distance,
        min_z + 15 + distance,
    )
}

fn include_bounding_box(encompassing: Option<BoundingBox>, new_box: BoundingBox) -> BoundingBox {
    encompassing.map_or(new_box, |existing| existing.encapsulate_box(new_box))
}

fn get_bury_contribution(dx: f64, dy: f64, dz: f64) -> f64 {
    let distance = (dx * dx + dy * dy + dz * dz).sqrt();
    map_clamped(distance, 0.0, 6.0, 1.0, 0.0)
}

fn get_beard_contribution(dx: i32, dy: i32, dz: i32, y_to_ground: i32) -> f64 {
    let xi = dx + BEARD_KERNEL_RADIUS;
    let yi = dy + BEARD_KERNEL_RADIUS;
    let zi = dz + BEARD_KERNEL_RADIUS;
    if !is_in_kernel_range(xi) || !is_in_kernel_range(yi) || !is_in_kernel_range(zi) {
        return 0.0;
    }

    let dy_with_offset = f64::from(y_to_ground) + 0.5;
    let distance_sqr = f64::from(dx) * f64::from(dx)
        + dy_with_offset * dy_with_offset
        + f64::from(dz) * f64::from(dz);
    let value = -dy_with_offset * fast_inv_sqrt(distance_sqr / 2.0) / 2.0;
    value
        * f64::from(
            BEARD_KERNEL[zi as usize * BEARD_KERNEL_SIZE * BEARD_KERNEL_SIZE
                + xi as usize * BEARD_KERNEL_SIZE
                + yi as usize],
        )
}

const fn is_in_kernel_range(value: i32) -> bool {
    value >= 0 && value < BEARD_KERNEL_SIZE as i32
}

fn compute_beard_contribution(dx: i32, dy: f64, dz: i32) -> f64 {
    let distance_sqr = f64::from(dx) * f64::from(dx) + dy * dy + f64::from(dz) * f64::from(dz);
    (-distance_sqr / 16.0).exp()
}

fn fast_inv_sqrt(x: f64) -> f64 {
    let x_half = 0.5 * x;
    let bits = 6_910_469_410_427_058_090_i64 - (x.to_bits() as i64 >> 1);
    let approx = f64::from_bits(bits as u64);
    approx * (1.5 - x_half * approx * approx)
}
