use std::cmp::Ordering;
use std::simd::cmp::SimdOrd as _;
use std::simd::num::SimdInt as _;
use std::simd::{Simd, i32x8, i64x8};

use rayon::iter::{IntoParallelIterator, ParallelIterator};

use super::PARAMETER_COUNT;
use super::types::{Parameter, ParameterPoint, TargetPoint};

const CHILDREN_PER_NODE: usize = 19;

enum RTreeNode {
    Leaf {
        parameter_space: [Parameter; PARAMETER_COUNT],
        value_index: usize,
    },
    SubTree {
        parameter_space: [Parameter; PARAMETER_COUNT],
        children: Vec<RTreeNode>,
    },
}

impl RTreeNode {
    const fn parameter_space(&self) -> &[Parameter; PARAMETER_COUNT] {
        match self {
            Self::Leaf {
                parameter_space, ..
            }
            | Self::SubTree {
                parameter_space, ..
            } => parameter_space,
        }
    }
}

#[derive(Clone)]
struct BuildEntry {
    parameter_space: [Parameter; PARAMETER_COUNT],
    index: usize,
}

fn build_parameter_space(children: &[RTreeNode]) -> [Parameter; PARAMETER_COUNT] {
    let mut bounds: [Option<Parameter>; PARAMETER_COUNT] = [None; PARAMETER_COUNT];
    for child in children {
        let ps = child.parameter_space();
        for d in 0..PARAMETER_COUNT {
            bounds[d] = Some(ps[d].span_with(bounds[d].as_ref()));
        }
    }
    bounds.map(|b| b.expect("bounds should be initialized"))
}

fn cost(parameter_space: &[Parameter; PARAMETER_COUNT]) -> i64 {
    let mut result = 0i64;
    for p in parameter_space {
        result += (p.max - p.min).abs();
    }
    result
}

fn build_tree(entries: &mut [BuildEntry]) -> RTreeNode {
    assert!(!entries.is_empty());

    if entries.len() == 1 {
        return RTreeNode::Leaf {
            parameter_space: entries[0].parameter_space,
            value_index: entries[0].index,
        };
    }

    if entries.len() <= CHILDREN_PER_NODE {
        entries.sort_by_key(|e| {
            let mut total: i64 = 0;
            for d in 0..PARAMETER_COUNT {
                let p = &e.parameter_space[d];
                total += ((p.min + p.max) / 2).abs();
            }
            total
        });

        let children: Vec<RTreeNode> = entries
            .iter()
            .map(|e| RTreeNode::Leaf {
                parameter_space: e.parameter_space,
                value_index: e.index,
            })
            .collect();
        let ps = build_parameter_space(&children);
        return RTreeNode::SubTree {
            parameter_space: ps,
            children,
        };
    }

    let mut min_cost = i64::MAX;
    let mut best_dim = 0;
    let mut best_buckets: Option<Vec<Vec<BuildEntry>>> = None;

    for d in 0..PARAMETER_COUNT {
        sort_entries(entries, d);
        let (bucket_cost, buckets) = snapshot_buckets(entries);
        if min_cost > bucket_cost {
            min_cost = bucket_cost;
            best_dim = d;
            best_buckets = Some(buckets);
        }
    }

    let buckets = best_buckets.expect("should have found at least one dimension");

    let mut bucket_subtrees: Vec<([Parameter; PARAMETER_COUNT], Vec<BuildEntry>)> = buckets
        .into_iter()
        .map(|bucket_entries| {
            let mut bounds: [Option<Parameter>; PARAMETER_COUNT] = [None; PARAMETER_COUNT];
            for e in &bucket_entries {
                #[expect(clippy::needless_range_loop, reason = "dim indexes parallel arrays")]
                for dim in 0..PARAMETER_COUNT {
                    bounds[dim] = Some(e.parameter_space[dim].span_with(bounds[dim].as_ref()));
                }
            }
            let ps = bounds.map(|b| b.expect("bounds should be initialized"));
            (ps, bucket_entries)
        })
        .collect();

    sort_bucket_subtrees(&mut bucket_subtrees, best_dim);
    let final_children: Vec<RTreeNode> = bucket_subtrees
        .into_par_iter()
        .map(|(_, mut child_entries)| build_tree(&mut child_entries))
        .collect();

    let ps = build_parameter_space(&final_children);
    RTreeNode::SubTree {
        parameter_space: ps,
        children: final_children,
    }
}

fn sort_entries(entries: &mut [BuildEntry], dimension: usize) {
    entries.sort_by(|a, b| {
        for offset in 0..PARAMETER_COUNT {
            let d = (dimension + offset) % PARAMETER_COUNT;
            let center_a = (a.parameter_space[d].min + a.parameter_space[d].max) / 2;
            let center_b = (b.parameter_space[d].min + b.parameter_space[d].max) / 2;
            let cmp = center_a.cmp(&center_b);
            if cmp != Ordering::Equal {
                return cmp;
            }
        }
        Ordering::Equal
    });
}

fn sort_bucket_subtrees(
    subtrees: &mut [([Parameter; PARAMETER_COUNT], Vec<BuildEntry>)],
    dimension: usize,
) {
    subtrees.sort_by(|a, b| {
        for offset in 0..PARAMETER_COUNT {
            let d = (dimension + offset) % PARAMETER_COUNT;
            let center_a = (a.0[d].min + a.0[d].max) / 2;
            let center_b = (b.0[d].min + b.0[d].max) / 2;
            let cmp = center_a.abs().cmp(&center_b.abs());
            if cmp != Ordering::Equal {
                return cmp;
            }
        }
        Ordering::Equal
    });
}

fn expected_children_count(total: usize) -> usize {
    let log_base = ((total as f64) - 0.01).ln() / (CHILDREN_PER_NODE as f64).ln();
    (CHILDREN_PER_NODE as f64).powf(log_base.floor()) as usize
}

#[expect(
    clippy::needless_range_loop,
    reason = "indexing into PARAMETER_COUNT parallel arrays; iterator would be less clear"
)]
fn snapshot_buckets(entries: &[BuildEntry]) -> (i64, Vec<Vec<BuildEntry>>) {
    let expected = expected_children_count(entries.len());
    let mut buckets = Vec::new();
    let mut total_cost = 0i64;
    let mut start = 0;
    while start < entries.len() {
        let end = (start + expected).min(entries.len());
        let bucket = entries[start..end].to_vec();
        let mut bounds: [Option<Parameter>; PARAMETER_COUNT] = [None; PARAMETER_COUNT];
        for e in &bucket {
            for d in 0..PARAMETER_COUNT {
                bounds[d] = Some(e.parameter_space[d].span_with(bounds[d].as_ref()));
            }
        }
        let ps = bounds.map(|b| b.expect("bounds should be initialized"));
        total_cost += cost(&ps);
        buckets.push(bucket);
        start = end;
    }
    (total_cost, buckets)
}

const LANES: usize = 8;

fn pack_target(target: &[i64; PARAMETER_COUNT]) -> i32x8 {
    let mut lanes = [0i32; LANES];
    for (lane, &value) in lanes.iter_mut().zip(target) {
        *lane = value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
    }
    Simd::from_array(lanes)
}

fn pack_bounds(space: &[Parameter; PARAMETER_COUNT]) -> (i32x8, i32x8) {
    let mut mins = [0i32; LANES];
    let mut maxs = [0i32; LANES];
    for (lane, parameter) in space.iter().enumerate() {
        mins[lane] = parameter
            .min
            .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
        maxs[lane] = parameter
            .max
            .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
    }
    (Simd::from_array(mins), Simd::from_array(maxs))
}

#[inline]
fn squared_distance(mins: i32x8, maxs: i32x8, target: i32x8) -> i64 {
    let delta = (target - maxs)
        .simd_max(mins - target)
        .simd_max(Simd::splat(0));
    let widened: i64x8 = delta.cast();
    (widened * widened).reduce_sum()
}

struct FlatNode {
    mins: i32x8,
    maxs: i32x8,
    value_index: u32,
    children_start: u32,
    children_count: u8,
}

impl FlatNode {
    #[inline]
    fn distance(&self, target: i32x8) -> i64 {
        squared_distance(self.mins, self.maxs, target)
    }

    #[inline]
    const fn is_leaf(&self) -> bool {
        self.children_count == 0
    }
}

fn flatten_tree(root: RTreeNode) -> Vec<FlatNode> {
    use std::collections::VecDeque;

    let mut nodes: Vec<FlatNode> = Vec::new();
    let mut queue: VecDeque<(Vec<RTreeNode>, Option<u32>)> = VecDeque::new();
    queue.push_back((vec![root], None));

    while let Some((batch, parent_idx)) = queue.pop_front() {
        let batch_start = nodes.len() as u32;

        if let Some(pidx) = parent_idx {
            nodes[pidx as usize].children_start = batch_start;
        }

        for node in batch {
            let flat_idx = nodes.len() as u32;
            match node {
                RTreeNode::Leaf {
                    parameter_space,
                    value_index,
                } => {
                    let (mins, maxs) = pack_bounds(&parameter_space);
                    nodes.push(FlatNode {
                        mins,
                        maxs,
                        value_index: value_index as u32,
                        children_start: 0,
                        children_count: 0,
                    });
                }
                RTreeNode::SubTree {
                    parameter_space,
                    children,
                } => {
                    let children_count = children.len() as u8;
                    let (mins, maxs) = pack_bounds(&parameter_space);
                    nodes.push(FlatNode {
                        mins,
                        maxs,
                        value_index: u32::MAX,
                        children_start: 0,
                        children_count,
                    });
                    queue.push_back((children, Some(flat_idx)));
                }
            }
        }
    }

    nodes
}

fn search_nearest(
    nodes: &[FlatNode],
    node: &FlatNode,
    target: i32x8,
    best_dist: &mut i64,
    best_idx: &mut Option<u32>,
) {
    let start = node.children_start as usize;
    let end = start + node.children_count as usize;
    let children = &nodes[start..end];

    for child in children {
        let child_dist = child.distance(target);
        if *best_dist > child_dist {
            if child.is_leaf() {
                *best_dist = child_dist;
                *best_idx = Some(child.value_index);
            } else {
                search_nearest(nodes, child, target, best_dist, best_idx);
            }
        }
    }
}

pub struct ParameterList<T> {
    values: Vec<(ParameterPoint, T)>,
    param_spaces: Vec<(i32x8, i32x8)>,
    nodes: Vec<FlatNode>,
}

impl<T> ParameterList<T> {
    pub fn new(values: Vec<(ParameterPoint, T)>) -> Self {
        assert!(!values.is_empty(), "Need at least one value");

        let param_spaces: Vec<(i32x8, i32x8)> = values
            .iter()
            .map(|(pp, _)| pack_bounds(&pp.parameter_space()))
            .collect();

        let mut entries: Vec<BuildEntry> = values
            .iter()
            .enumerate()
            .map(|(i, (pp, _))| BuildEntry {
                parameter_space: pp.parameter_space(),
                index: i,
            })
            .collect();

        let root = build_tree(&mut entries);
        let nodes = flatten_tree(root);

        Self {
            values,
            param_spaces,
            nodes,
        }
    }

    pub fn values(&self) -> &[(ParameterPoint, T)] {
        &self.values
    }

    pub fn find_value(&self, target: &TargetPoint) -> &T {
        let target_array = pack_target(&target.to_parameter_array());
        let root = &self.nodes[0];
        if root.is_leaf() {
            return &self.values[root.value_index as usize].1;
        }
        let mut best_dist = i64::MAX;
        let mut best_idx = None;
        search_nearest(
            &self.nodes,
            root,
            target_array,
            &mut best_dist,
            &mut best_idx,
        );
        &self.values[best_idx.expect("R-Tree search should always find a value") as usize].1
    }

    pub fn find_value_cached(&self, target: &TargetPoint, cache: &mut Option<usize>) -> &T {
        let target_array = pack_target(&target.to_parameter_array());

        let root = &self.nodes[0];
        if root.is_leaf() {
            let idx = root.value_index as usize;
            *cache = Some(idx);
            return &self.values[idx].1;
        }

        let (mut best_dist, init_idx) = match *cache {
            Some(idx) => {
                let (mins, maxs) = self.param_spaces[idx];
                (squared_distance(mins, maxs, target_array), Some(idx as u32))
            }
            None => (i64::MAX, None),
        };

        let mut best_idx = init_idx;
        search_nearest(
            &self.nodes,
            root,
            target_array,
            &mut best_dist,
            &mut best_idx,
        );
        let result_idx = best_idx.expect("R-Tree search should always find a value") as usize;

        *cache = Some(result_idx);
        &self.values[result_idx].1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parameter_list_find_value() {
        let values = vec![
            (
                ParameterPoint::new(
                    Parameter::new(-10000, 0),
                    Parameter::new(0, 0),
                    Parameter::new(0, 0),
                    Parameter::new(0, 0),
                    Parameter::new(0, 0),
                    Parameter::new(0, 0),
                    0,
                ),
                "cold",
            ),
            (
                ParameterPoint::new(
                    Parameter::new(0, 10000),
                    Parameter::new(0, 0),
                    Parameter::new(0, 0),
                    Parameter::new(0, 0),
                    Parameter::new(0, 0),
                    Parameter::new(0, 0),
                    0,
                ),
                "hot",
            ),
        ];

        let list = ParameterList::new(values);

        let cold_target = TargetPoint::new(-5000, 0, 0, 0, 0, 0);
        assert_eq!(*list.find_value(&cold_target), "cold");

        let hot_target = TargetPoint::new(5000, 0, 0, 0, 0, 0);
        assert_eq!(*list.find_value(&hot_target), "hot");
    }
}
