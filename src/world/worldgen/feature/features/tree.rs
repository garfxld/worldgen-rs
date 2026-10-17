use std::collections::HashSet;

use serde::Deserialize;
use shulkr::util::Key;

use crate::util::hash_set::JavaHashSet;
use shulkr::util::BoundingBox;
use shulkr::world::block::Properties;

use crate::util::rng::RandomSource;
use crate::world::worldgen::feature::featuresize::FeatureSize;
use crate::world::worldgen::feature::foliageplacers::{FoliagePlacer, FoliageSetter};
use crate::world::worldgen::feature::rootplacers::RootPlacer;
use crate::world::worldgen::feature::state_provider::ProviderHolder;
use crate::world::worldgen::feature::treedecorators::{TreeDecorator, TreeDecoratorContext};
use crate::world::worldgen::feature::trunkplacers::TrunkPlacer;
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util::BlockPosition;
use shulkr::world::block::BlockState;

const LEAVES: Key = Key::const_vanilla("leaves");
const REPLACEABLE_BY_TREES: Key = Key::const_vanilla("replaceable_by_trees");
const VINE: Key = Key::const_vanilla("vine");
const PREVENTS_NEARBY_LEAF_DECAY: Key = Key::const_vanilla("prevents_nearby_leaf_decay");

#[derive(Debug, Deserialize)]
pub struct TreeConfig {
    pub trunk_provider: ProviderHolder,
    pub trunk_placer: TrunkPlacer,
    pub foliage_provider: ProviderHolder,
    pub foliage_placer: FoliagePlacer,
    pub minimum_size: FeatureSize,
    #[serde(default)]
    pub root_placer: Option<RootPlacer>,
    pub decorators: Vec<TreeDecorator>,
    #[serde(default)]
    pub ignore_vines: bool,
    pub below_trunk_provider: ProviderHolder,
}

pub fn lowest_trunk_or_root_of_tree(context: &TreeDecoratorContext<'_, '_>) -> Vec<BlockPosition> {
    let mut positions = Vec::new();
    if context.roots.is_empty() {
        positions.extend(context.logs.iter().copied());
    } else if !context.logs.is_empty() && context.roots[0].y() == context.logs[0].y() {
        positions.extend(context.logs.iter().copied());
        positions.extend(context.roots.iter().copied());
    } else {
        positions.extend(context.roots.iter().copied());
    }
    positions
}

pub fn is_vine(region: &WorldGenRegion<'_>, pos: BlockPosition) -> bool {
    region.get_block(pos).as_block().key() == &VINE
}

pub fn is_air_or_leaves(region: &WorldGenRegion<'_>, pos: BlockPosition) -> bool {
    let state = region.get_block(pos);
    state.is_air() || state.as_block().has_tag(&LEAVES)
}

pub fn valid_tree_pos(region: &WorldGenRegion<'_>, pos: BlockPosition) -> bool {
    let state = region.get_block(pos);
    state.is_air() || state.as_block().has_tag(&REPLACEABLE_BY_TREES)
}

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPosition,
    config: &TreeConfig,
) -> bool {
    let mut root_positions = JavaHashSet::new();
    let mut trunks = JavaHashSet::new();
    let mut foliage_setter = FoliageSetter::new();
    let mut decorations = JavaHashSet::new();

    let placed = do_place(
        region,
        random,
        origin,
        &mut root_positions,
        &mut trunks,
        &mut foliage_setter,
        config,
    );
    if !placed || (trunks.is_empty() && foliage_setter.positions.is_empty()) {
        return false;
    }

    if !config.decorators.is_empty() {
        let mut context = TreeDecoratorContext::new(
            region,
            random,
            &trunks,
            &foliage_setter.positions,
            &root_positions,
            &mut decorations,
        );
        for decorator in &config.decorators {
            decorator.place(&mut context);
        }
    }

    let mut all = root_positions
        .iter()
        .chain(trunks.iter())
        .chain(foliage_setter.positions.iter())
        .chain(decorations.iter());
    let Some(first) = all.next() else {
        return false;
    };
    let mut bounds = BoundingBox::from_point(first);
    for pos in all {
        bounds = bounds.encapsulate(pos);
    }

    update_leaves(region, bounds, &trunks, &decorations, &root_positions);
    true
}

fn do_place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPosition,
    root_positions: &mut JavaHashSet,
    trunks: &mut JavaHashSet,
    foliage_setter: &mut FoliageSetter,
    config: &TreeConfig,
) -> bool {
    let tree_height = config.trunk_placer.get_tree_height(random);
    let foliage_height = config.foliage_placer.foliage_height(random, tree_height);
    let trunk_height = tree_height - foliage_height;
    let leaf_radius = config.foliage_placer.foliage_radius(random, trunk_height);
    let trunk_origin = config.root_placer.as_ref().map_or(origin, |root_placer| {
        root_placer.get_trunk_origin(origin, random)
    });
    let min_y = origin.y().min(trunk_origin.y());
    let max_y = origin.y().max(trunk_origin.y()) + tree_height + 1;
    if min_y < region.min_y() + 1 || max_y > region.max_y() {
        return false;
    }

    let min_clipped_height = config.minimum_size.min_clipped_height();
    let clipped_tree_height = get_max_free_tree_height(region, tree_height, trunk_origin, config);
    if clipped_tree_height < tree_height
        && !min_clipped_height.is_some_and(|min| clipped_tree_height >= min)
    {
        return false;
    }

    if let Some(root_placer) = &config.root_placer {
        let mut root_setter = |pos: BlockPosition, state: BlockState| {
            root_positions.insert(pos);
            region.set_block(pos, state);
        };
        if !root_placer.place_roots(
            region,
            &mut root_setter,
            random,
            origin,
            trunk_origin,
            config,
        ) {
            return false;
        }
    }

    let attachments = {
        let mut trunk_setter = |pos: BlockPosition, state: BlockState| {
            trunks.insert(pos);
            region.set_block(pos, state);
        };
        config.trunk_placer.place_trunk(
            region,
            &mut trunk_setter,
            random,
            clipped_tree_height,
            trunk_origin,
            config,
        )
    };
    for attachment in &attachments {
        config.foliage_placer.create_foliage(
            region,
            foliage_setter,
            random,
            config,
            clipped_tree_height,
            attachment,
            foliage_height,
            leaf_radius,
        );
    }
    true
}

fn get_max_free_tree_height(
    region: &WorldGenRegion<'_>,
    max_tree_height: i32,
    tree_pos: BlockPosition,
    config: &TreeConfig,
) -> i32 {
    for y in 0..=max_tree_height + 1 {
        let r = config.minimum_size.get_size_at_height(max_tree_height, y);
        for x in -r..=r {
            for z in -r..=r {
                let pos = tree_pos.offset(x, y, z);
                if !config.trunk_placer.is_free(region, pos)
                    || (!config.ignore_vines && is_vine(region, pos))
                {
                    return y - 2;
                }
            }
        }
    }
    max_tree_height
}

fn update_leaves(
    region: &WorldGenRegion<'_>,
    bounds: BoundingBox,
    logs: &JavaHashSet,
    decorations: &JavaHashSet,
    root_positions: &JavaHashSet,
) {
    const MAX_DISTANCE: usize = 7;
    let mut shape: HashSet<BlockPosition> = HashSet::new();
    let mut to_check: Vec<JavaHashSet> = (0..MAX_DISTANCE).map(|_| JavaHashSet::new()).collect();

    for pos in decorations.iter().chain(root_positions.iter()) {
        if bounds.is_inside(pos) {
            shape.insert(pos);
        }
    }

    let mut smallest_distance = 0usize;
    for pos in logs.iter() {
        to_check[0].insert(pos);
    }

    loop {
        while smallest_distance >= MAX_DISTANCE || !to_check[smallest_distance].is_empty() {
            if smallest_distance >= MAX_DISTANCE {
                return;
            }
            let pos = to_check[smallest_distance]
                .remove_first()
                .expect("bucket is non-empty");
            if !bounds.is_inside(pos) {
                continue;
            }
            if smallest_distance != 0 {
                let state = region.get_block(pos);
                region.set_block(
                    pos,
                    state.with(Properties::DISTANCE, smallest_distance as u8),
                );
            }
            shape.insert(pos);

            for direction in super::DIRECTIONS {
                let neighbour = pos.relative(direction);
                if !bounds.is_inside(neighbour) || shape.contains(&neighbour) {
                    continue;
                }
                let state = region.get_block(neighbour);
                let distance = if state.as_block().has_tag(&PREVENTS_NEARBY_LEAF_DECAY) {
                    0
                } else {
                    let Some(value) = state.get(Properties::DISTANCE) else {
                        continue;
                    };
                    usize::from(value)
                };
                let new_distance = distance.min(smallest_distance + 1);
                if new_distance < MAX_DISTANCE {
                    to_check[new_distance].insert(neighbour);
                    smallest_distance = smallest_distance.min(new_distance);
                }
            }
        }
        smallest_distance += 1;
    }
}
