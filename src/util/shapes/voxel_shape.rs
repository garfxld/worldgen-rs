use crate::util::axis::Axis;
use crate::util::shapes::boolean_op::BooleanOp;
use crate::util::shapes::discrete_voxel_shape::BitSetDiscreteVoxelShape;
use crate::util::shapes::double_list::DoubleList;
use crate::util::shapes::shapes;

#[derive(Debug, Clone)]
pub enum VoxelShape {
    Array {
        shape: BitSetDiscreteVoxelShape,
        xs: DoubleList,
        ys: DoubleList,
        zs: DoubleList,
    },
    Cube {
        shape: BitSetDiscreteVoxelShape,
    },
}

impl VoxelShape {
    pub fn array(
        shape: BitSetDiscreteVoxelShape,
        xs: DoubleList,
        ys: DoubleList,
        zs: DoubleList,
    ) -> Self {
        assert!(
            shape.size(Axis::X) + 1 == xs.size()
                && shape.size(Axis::Y) + 1 == ys.size()
                && shape.size(Axis::Z) + 1 == zs.size(),
            "Lengths of point arrays must be consistent with the size of the VoxelShape."
        );
        Self::Array { shape, xs, ys, zs }
    }

    pub const fn cube(shape: BitSetDiscreteVoxelShape) -> Self {
        Self::Cube { shape }
    }

    pub const fn shape(&self) -> &BitSetDiscreteVoxelShape {
        match self {
            Self::Array { shape, .. } | Self::Cube { shape } => shape,
        }
    }

    pub fn get_coords(&self, axis: Axis) -> DoubleList {
        match self {
            Self::Array { xs, ys, zs, .. } => match axis {
                Axis::X => xs.clone(),
                Axis::Y => ys.clone(),
                Axis::Z => zs.clone(),
            },
            Self::Cube { shape } => DoubleList::cube_point_range(shape.size(axis)),
        }
    }

    pub fn get(&self, axis: Axis, index: i32) -> f64 {
        self.get_coords(axis).get(index as usize)
    }

    pub fn is_empty(&self) -> bool {
        self.shape().is_empty()
    }

    pub fn min(&self, axis: Axis) -> f64 {
        let index = self.shape().first_full(axis);
        if index >= self.shape().size(axis) as i32 {
            f64::INFINITY
        } else {
            self.get(axis, index)
        }
    }

    pub fn max(&self, axis: Axis) -> f64 {
        let index = self.shape().last_full(axis);
        if index <= 0 {
            f64::NEG_INFINITY
        } else {
            self.get(axis, index)
        }
    }

    pub fn for_all_boxes(&self, consumer: &mut dyn FnMut(f64, f64, f64, f64, f64, f64)) {
        let x_coords = self.get_coords(Axis::X);
        let y_coords = self.get_coords(Axis::Y);
        let z_coords = self.get_coords(Axis::Z);
        self.shape()
            .for_all_boxes(&mut |xi1, yi1, zi1, xi2, yi2, zi2| {
                consumer(
                    x_coords.get(xi1 as usize),
                    y_coords.get(yi1 as usize),
                    z_coords.get(zi1 as usize),
                    x_coords.get(xi2 as usize),
                    y_coords.get(yi2 as usize),
                    z_coords.get(zi2 as usize),
                );
            });
    }

    pub fn optimize(&self) -> Self {
        let mut result = shapes::empty();
        self.for_all_boxes(&mut |x1, y1, z1, x2, y2, z2| {
            result = shapes::join_unoptimized(
                &result,
                &shapes::box_shape(x1, y1, z1, x2, y2, z2),
                BooleanOp::Or,
            );
        });
        result
    }
}
