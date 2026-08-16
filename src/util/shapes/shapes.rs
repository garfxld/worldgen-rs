use crate::util::axis::Axis;
use crate::util::geometry::BlockLocalAabb;
use crate::util::shapes::boolean_op::BooleanOp;
use crate::util::shapes::discrete_voxel_shape::BitSetDiscreteVoxelShape;
use crate::util::shapes::double_list::DoubleList;
use crate::util::shapes::index_merger::{IndexMerger, lcm};
use crate::util::shapes::voxel_shape::VoxelShape;

pub const EPSILON: f64 = 1.0E-7;

fn block_shape() -> BitSetDiscreteVoxelShape {
    let mut shape = BitSetDiscreteVoxelShape::new(1, 1, 1);
    shape.fill(0, 0, 0);
    shape
}

pub fn block() -> VoxelShape {
    VoxelShape::cube(block_shape())
}

pub fn empty() -> VoxelShape {
    VoxelShape::array(
        BitSetDiscreteVoxelShape::new(0, 0, 0),
        DoubleList::from_slice(&[0.0]),
        DoubleList::from_slice(&[0.0]),
        DoubleList::from_slice(&[0.0]),
    )
}

pub fn box_shape(
    min_x: f64,
    min_y: f64,
    min_z: f64,
    max_x: f64,
    max_y: f64,
    max_z: f64,
) -> VoxelShape {
    assert!(
        min_x <= max_x && min_y <= max_y && min_z <= max_z,
        "The min values need to be smaller or equals to the max values"
    );
    create(min_x, min_y, min_z, max_x, max_y, max_z)
}

pub fn create(
    min_x: f64,
    min_y: f64,
    min_z: f64,
    max_x: f64,
    max_y: f64,
    max_z: f64,
) -> VoxelShape {
    if max_x - min_x < EPSILON || max_y - min_y < EPSILON || max_z - min_z < EPSILON {
        return empty();
    }

    let x_bits = find_bits(min_x, max_x);
    let y_bits = find_bits(min_y, max_y);
    let z_bits = find_bits(min_z, max_z);
    if x_bits < 0 || y_bits < 0 || z_bits < 0 {
        return VoxelShape::array(
            block_shape(),
            DoubleList::from_slice(&[min_x, max_x]),
            DoubleList::from_slice(&[min_y, max_y]),
            DoubleList::from_slice(&[min_z, max_z]),
        );
    }

    if x_bits == 0 && y_bits == 0 && z_bits == 0 {
        return block();
    }

    let x_size = 1 << x_bits;
    let y_size = 1 << y_bits;
    let z_size = 1 << z_bits;
    let shape = BitSetDiscreteVoxelShape::with_filled_bounds(
        x_size,
        y_size,
        z_size,
        (min_x * x_size as f64).round() as i32,
        (min_y * y_size as f64).round() as i32,
        (min_z * z_size as f64).round() as i32,
        (max_x * x_size as f64).round() as i32,
        (max_y * y_size as f64).round() as i32,
        (max_z * z_size as f64).round() as i32,
    );
    VoxelShape::cube(shape)
}

pub fn create_aabb(aabb: BlockLocalAabb) -> VoxelShape {
    create(
        aabb.min_x(),
        aabb.min_y(),
        aabb.min_z(),
        aabb.max_x(),
        aabb.max_y(),
        aabb.max_z(),
    )
}

fn find_bits(min: f64, max: f64) -> i32 {
    if min < -EPSILON || max > 1.0000001 {
        return -1;
    }

    for bits in 0..=3 {
        let intervals = 1 << bits;
        let sh_min = min * f64::from(intervals);
        let sh_max = max * f64::from(intervals);
        let found_min = (sh_min - sh_min.round()).abs() < EPSILON * f64::from(intervals);
        let found_max = (sh_max - sh_max.round()).abs() < EPSILON * f64::from(intervals);
        if found_min && found_max {
            return bits;
        }
    }

    -1
}

pub fn join(first: &VoxelShape, second: &VoxelShape, op: BooleanOp) -> VoxelShape {
    join_unoptimized(first, second, op).optimize()
}

pub fn join_unoptimized(first: &VoxelShape, second: &VoxelShape, op: BooleanOp) -> VoxelShape {
    assert!(!op.apply(false, false), "invalid boolean op for join");

    let first_only_matters = op.apply(true, false);
    let second_only_matters = op.apply(false, true);
    if first.is_empty() {
        return if second_only_matters {
            second.clone()
        } else {
            empty()
        };
    }

    if second.is_empty() {
        return if first_only_matters {
            first.clone()
        } else {
            empty()
        };
    }

    let x_merger = create_index_merger(
        1,
        &first.get_coords(Axis::X),
        &second.get_coords(Axis::X),
        first_only_matters,
        second_only_matters,
    );
    let y_merger = create_index_merger(
        x_merger.size() - 1,
        &first.get_coords(Axis::Y),
        &second.get_coords(Axis::Y),
        first_only_matters,
        second_only_matters,
    );
    let z_merger = create_index_merger(
        (x_merger.size() - 1) * (y_merger.size() - 1),
        &first.get_coords(Axis::Z),
        &second.get_coords(Axis::Z),
        first_only_matters,
        second_only_matters,
    );
    let shape = join_discrete(
        first.shape(),
        second.shape(),
        &x_merger,
        &y_merger,
        &z_merger,
        op,
    );

    if x_merger.is_discrete_cube() && y_merger.is_discrete_cube() && z_merger.is_discrete_cube() {
        VoxelShape::cube(shape)
    } else {
        VoxelShape::array(
            shape,
            x_merger.get_list(),
            y_merger.get_list(),
            z_merger.get_list(),
        )
    }
}

pub fn join_is_not_empty(first: &VoxelShape, second: &VoxelShape, op: BooleanOp) -> bool {
    assert!(!op.apply(false, false), "invalid boolean op for join");

    let first_empty = first.is_empty();
    let second_empty = second.is_empty();
    if first_empty || second_empty {
        return op.apply(!first_empty, !second_empty);
    }

    let first_only_matters = op.apply(true, false);
    let second_only_matters = op.apply(false, true);

    for axis in [Axis::X, Axis::Y, Axis::Z] {
        if first.max(axis) < second.min(axis) - EPSILON
            || second.max(axis) < first.min(axis) - EPSILON
        {
            return first_only_matters || second_only_matters;
        }
    }

    let x_merger = create_index_merger(
        1,
        &first.get_coords(Axis::X),
        &second.get_coords(Axis::X),
        first_only_matters,
        second_only_matters,
    );
    let y_merger = create_index_merger(
        x_merger.size() - 1,
        &first.get_coords(Axis::Y),
        &second.get_coords(Axis::Y),
        first_only_matters,
        second_only_matters,
    );
    let z_merger = create_index_merger(
        (x_merger.size() - 1) * (y_merger.size() - 1),
        &first.get_coords(Axis::Z),
        &second.get_coords(Axis::Z),
        first_only_matters,
        second_only_matters,
    );

    let first_shape = first.shape();
    let second_shape = second.shape();
    !x_merger.for_merged_indexes(&mut |x1, x2, _| {
        y_merger.for_merged_indexes(&mut |y1, y2, _| {
            z_merger.for_merged_indexes(&mut |z1, z2, _| {
                !op.apply(
                    first_shape.is_full_wide(x1, y1, z1),
                    second_shape.is_full_wide(x2, y2, z2),
                )
            })
        })
    })
}

fn join_discrete(
    first: &BitSetDiscreteVoxelShape,
    second: &BitSetDiscreteVoxelShape,
    x_merger: &IndexMerger,
    y_merger: &IndexMerger,
    z_merger: &IndexMerger,
    op: BooleanOp,
) -> BitSetDiscreteVoxelShape {
    let mut shape = BitSetDiscreteVoxelShape::new(
        x_merger.size() - 1,
        y_merger.size() - 1,
        z_merger.size() - 1,
    );
    let mut bounds = [i32::MAX, i32::MAX, i32::MAX, i32::MIN, i32::MIN, i32::MIN];

    x_merger.for_merged_indexes(&mut |x1, x2, xr| {
        let mut updated_slice = false;
        y_merger.for_merged_indexes(&mut |y1, y2, yr| {
            let mut updated_column = false;
            z_merger.for_merged_indexes(&mut |z1, z2, zr| {
                if op.apply(first.is_full_wide(x1, y1, z1), second.is_full_wide(x2, y2, z2)) {
                    shape.set(xr, yr, zr);
                    bounds[2] = bounds[2].min(zr);
                    bounds[5] = bounds[5].max(zr);
                    updated_column = true;
                }
                true
            });
            if updated_column {
                bounds[1] = bounds[1].min(yr);
                bounds[4] = bounds[4].max(yr);
                updated_slice = true;
            }
            true
        });
        if updated_slice {
            bounds[0] = bounds[0].min(xr);
            bounds[3] = bounds[3].max(xr);
        }
        true
    });

    shape.set_bounds(
        bounds[0],
        bounds[1],
        bounds[2],
        bounds[3] + 1,
        bounds[4] + 1,
        bounds[5] + 1,
    );
    shape
}

fn create_index_merger(
    cost: usize,
    first: &DoubleList,
    second: &DoubleList,
    first_only_matters: bool,
    second_only_matters: bool,
) -> IndexMerger {
    let first_size = first.size() - 1;
    let second_size = second.size() - 1;
    if first.is_cube_point_range() && second.is_cube_point_range() {
        let size = lcm(first_size, second_size);
        if cost as u64 * size <= 256 {
            return IndexMerger::discrete_cube(first_size, second_size);
        }
    }

    if first.get(first_size) < second.get(0) - EPSILON {
        return IndexMerger::non_overlapping(first.clone(), second.clone(), false);
    }

    if second.get(second_size) < first.get(0) - EPSILON {
        return IndexMerger::non_overlapping(second.clone(), first.clone(), true);
    }

    if first_size == second_size && first == second {
        IndexMerger::Identical {
            coords: first.clone(),
        }
    } else {
        IndexMerger::indirect(first, second, first_only_matters, second_only_matters)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn free_region(min: f64, max: f64) -> VoxelShape {
        create(min, min, min, max, max, max)
    }

    #[test]
    fn create_uses_a_single_cell_for_world_scale_boxes() {
        let shape = create(-64.0, 0.0, -64.0, 64.0, 32.0, 64.0);
        assert_eq!(shape.min(Axis::X), -64.0);
        assert_eq!(shape.max(Axis::Y), 32.0);
        assert!(!shape.is_empty());
    }

    #[test]
    fn subtracting_a_box_leaves_the_remaining_space_free() {
        let free = join(
            &free_region(0.0, 32.0),
            &create(0.0, 0.0, 0.0, 16.0, 32.0, 32.0),
            BooleanOp::OnlyFirst,
        );

        let inside_removed = create(4.0, 4.0, 4.0, 8.0, 8.0, 8.0);
        assert!(join_is_not_empty(
            &free,
            &inside_removed,
            BooleanOp::OnlySecond
        ));

        let inside_free = create(20.0, 4.0, 4.0, 24.0, 8.0, 8.0);
        assert!(!join_is_not_empty(
            &free,
            &inside_free,
            BooleanOp::OnlySecond
        ));
    }

    #[test]
    fn occupying_boxes_shrinks_the_free_region() {
        let mut free = free_region(0.0, 32.0);
        let occupied = create(8.0, 8.0, 8.0, 16.0, 16.0, 16.0);
        free = join_unoptimized(&free, &occupied, BooleanOp::OnlyFirst);

        let overlapping = create(15.0, 15.0, 15.0, 20.0, 20.0, 20.0);
        assert!(join_is_not_empty(
            &free,
            &overlapping,
            BooleanOp::OnlySecond
        ));

        let outside = create(0.0, 0.0, 0.0, 8.0, 8.0, 8.0);
        assert!(!join_is_not_empty(&free, &outside, BooleanOp::OnlySecond));
    }

    #[test]
    fn optimize_preserves_the_occupied_volume() {
        let free = join(
            &free_region(0.0, 4.0),
            &create(0.0, 0.0, 0.0, 2.0, 4.0, 4.0),
            BooleanOp::OnlyFirst,
        );

        let mut boxes = Vec::new();
        free.for_all_boxes(&mut |x1, y1, z1, x2, y2, z2| {
            boxes.push((x1, y1, z1, x2, y2, z2));
        });
        assert_eq!(boxes, vec![(2.0, 0.0, 0.0, 4.0, 4.0, 4.0)]);
    }
}
