use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;

use crate::chunk::heightmap::HeightmapType;
use crate::util::geometry::{BlockLocalAabb, BoundingBox};
use crate::util::ident::Identifier;
use crate::util::pos::BlockPos;
use crate::util::rotation::Rotation;
use crate::util::shapes::boolean_op::BooleanOp;
use crate::util::shapes::shapes;
use crate::util::shapes::voxel_shape::VoxelShape;
use crate::worldgen::generator::Generator;
use crate::worldgen::structure::pools::alias::pool_alias_lookup::PoolAliasLookup;
use crate::worldgen::structure::pools::dimension_padding::DimensionPadding;
use crate::worldgen::structure::pools::jigsaw_junction::JigsawJunction;
use crate::worldgen::structure::pools::structure_pool_element::StructurePoolElement;
use crate::worldgen::structure::pools::structure_template_pool::{
    Projection, StructureTemplatePool,
};
use crate::worldgen::structure::pools::template_pools;
use crate::worldgen::structure::structure::{GenerationContext, GenerationStub};
use crate::worldgen::structure::structure_piece::{PoolElementStructurePiece, StructurePiece};
use crate::worldgen::structure::structures::jigsaw::MaxDistance;
use crate::worldgen::structure::templatesystem::liquid_settings::LiquidSettings;
use crate::worldgen::structure::templatesystem::structure_template::{can_attach, front_facing};

const UNSET_HEIGHT: i32 = i32::MIN;

#[expect(
    clippy::too_many_arguments,
    reason = "mirrors vanilla JigsawPlacement::addPieces"
)]
pub fn add_pieces<'a>(
    context: &'a GenerationContext<'a>,
    start_pool: &Identifier,
    start_jigsaw: Option<&Identifier>,
    max_depth: i32,
    position: BlockPos,
    do_expansion_hack: bool,
    project_start_to_heightmap: Option<HeightmapType>,
    max_distance_from_center: MaxDistance,
    pool_alias_lookup: PoolAliasLookup,
    dimension_padding: DimensionPadding,
    liquid_settings: LiquidSettings,
) -> Option<GenerationStub<'a>> {
    let center_rotation = context.with_random(Rotation::get_random);
    let center_pool_key = pool_alias_lookup.lookup(start_pool);
    let center_pool = template_pools()
        .get(&center_pool_key)
        .or_else(|| template_pools().get(start_pool))?;
    let center_element = context.with_random(|random| center_pool.get_random_template(random));
    if matches!(center_element, StructurePoolElement::Empty) {
        return None;
    }

    let anchored_position = match start_jigsaw {
        Some(target_jigsaw_id) => context.with_random(|random| {
            center_element
                .get_shuffled_jigsaw_blocks(position, center_rotation, random)
                .into_iter()
                .find(|jigsaw| jigsaw.name == *target_jigsaw_id)
                .map(|jigsaw| jigsaw.info.pos)
        })?,
        None => position,
    };

    let local_anchor_position = anchored_position.0 - position.0;
    let adjusted_position = BlockPos(position.0 - local_anchor_position);
    let mut center_piece = PoolElementStructurePiece::new(
        center_element,
        adjusted_position,
        center_element.ground_level_delta(),
        center_rotation,
        center_element.get_bounding_box(adjusted_position, center_rotation),
        liquid_settings,
    );

    let box_ = center_piece.bounding_box;
    let center_x = (box_.max_x() + box_.min_x()) / 2;
    let center_z = (box_.max_z() + box_.min_z()) / 2;
    let bottom_y = match project_start_to_heightmap {
        None => adjusted_position.y(),
        Some(heightmap) => {
            position.y() + context.generator.get_first_free_height(center_x, center_z, heightmap)
        }
    };
    let old_absolute_ground_y = box_.min_y() + center_piece.ground_level_delta;
    let dy = bottom_y - old_absolute_ground_y;
    center_piece.bounding_box = center_piece.bounding_box.moved(0, dy, 0);
    center_piece.position = center_piece.position.offset(0, dy, 0);

    if is_start_too_close_to_world_height_limits(context, dimension_padding, center_piece.bounding_box)
    {
        return None;
    }

    let center_y = bottom_y + local_anchor_position.y;
    let outer_box = center_piece.bounding_box;

    Some(GenerationStub {
        position: BlockPos::new(center_x, center_y, center_z),
        build: Box::new(move |builder| {
            let mut pieces = vec![center_piece];
            if max_depth > 0 {
                let aabb = BlockLocalAabb::new(
                    f64::from(center_x - max_distance_from_center.horizontal),
                    f64::from(
                        (center_y - max_distance_from_center.vertical)
                            .max(context.min_y + dimension_padding.bottom),
                    ),
                    f64::from(center_z - max_distance_from_center.horizontal),
                    f64::from(center_x + max_distance_from_center.horizontal + 1),
                    f64::from(
                        (center_y + max_distance_from_center.vertical + 1)
                            .min(context.max_y() + 1 - dimension_padding.top),
                    ),
                    f64::from(center_z + max_distance_from_center.horizontal + 1),
                );
                let shape = shapes::join(
                    &shapes::create_aabb(aabb),
                    &shapes::create_aabb(BlockLocalAabb::of(outer_box)),
                    BooleanOp::OnlyFirst,
                );

                place_children(
                    context,
                    max_depth,
                    do_expansion_hack,
                    &mut pieces,
                    shape,
                    &pool_alias_lookup,
                    liquid_settings,
                );
            }

            for piece in pieces {
                builder.add_piece(StructurePiece::PoolElement(piece));
            }
        }),
    })
}

fn is_start_too_close_to_world_height_limits(
    context: &GenerationContext<'_>,
    dimension_padding: DimensionPadding,
    center_piece_bb: BoundingBox,
) -> bool {
    if dimension_padding == DimensionPadding::ZERO {
        return false;
    }
    let min_y_with_padding = context.min_y + dimension_padding.bottom;
    let max_y_with_padding = context.max_y() - dimension_padding.top;
    center_piece_bb.min_y() < min_y_with_padding || center_piece_bb.max_y() > max_y_with_padding
}

struct PieceState {
    piece_index: usize,
    free: Rc<RefCell<VoxelShape>>,
    depth: i32,
}

#[derive(Default)]
struct SequencedPriorityQueue {
    queues: BTreeMap<i32, VecDeque<PieceState>>,
}

impl SequencedPriorityQueue {
    fn add(&mut self, state: PieceState, priority: i32) {
        self.queues.entry(priority).or_default().push_back(state);
    }

    fn next(&mut self) -> Option<PieceState> {
        while let Some((&priority, queue)) = self.queues.iter_mut().next_back() {
            if let Some(state) = queue.pop_front() {
                return Some(state);
            }
            self.queues.remove(&priority);
        }
        None
    }
}

fn place_children(
    context: &GenerationContext<'_>,
    max_depth: i32,
    do_expansion_hack: bool,
    pieces: &mut Vec<PoolElementStructurePiece>,
    shape: VoxelShape,
    pool_alias_lookup: &PoolAliasLookup,
    liquid_settings: LiquidSettings,
) {
    let mut placing = SequencedPriorityQueue::default();
    let free = Rc::new(RefCell::new(shape));
    try_placing_children(
        context,
        max_depth,
        do_expansion_hack,
        pieces,
        0,
        &free,
        0,
        &mut placing,
        pool_alias_lookup,
        liquid_settings,
    );

    while let Some(state) = placing.next() {
        try_placing_children(
            context,
            max_depth,
            do_expansion_hack,
            pieces,
            state.piece_index,
            &state.free,
            state.depth,
            &mut placing,
            pool_alias_lookup,
            liquid_settings,
        );
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "mirrors vanilla JigsawPlacement.Placer::tryPlacingChildren"
)]
fn try_placing_children(
    context: &GenerationContext<'_>,
    max_depth: i32,
    do_expansion_hack: bool,
    pieces: &mut Vec<PoolElementStructurePiece>,
    source_index: usize,
    context_free: &Rc<RefCell<VoxelShape>>,
    depth: i32,
    placing: &mut SequencedPriorityQueue,
    pool_alias_lookup: &PoolAliasLookup,
    liquid_settings: LiquidSettings,
) {
    let source_element = pieces[source_index].element;
    let source_box_position = pieces[source_index].position;
    let source_rotation = pieces[source_index].rotation;
    let source_projection = source_element.projection();
    let source_rigid = source_projection == Projection::Rigid;
    let source_bb = pieces[source_index].bounding_box;
    let source_box_y = source_bb.min_y();
    let source_ground_level_delta = pieces[source_index].ground_level_delta;

    let mut source_free: Option<Rc<RefCell<VoxelShape>>> = None;

    let source_jigsaws = context.with_random(|random| {
        source_element.get_shuffled_jigsaw_blocks(source_box_position, source_rotation, random)
    });

    'jigsaws: for source_jigsaw in source_jigsaws {
        let source_jigsaw_info = &source_jigsaw.info;
        let source_direction = front_facing(source_jigsaw_info.state);
        let source_jigsaw_pos = source_jigsaw_info.pos;
        let target_jigsaw_pos = source_jigsaw_pos.relative(source_direction);
        let source_jigsaw_local_y = source_jigsaw_pos.y() - source_box_y;
        let mut source_jigsaw_base_height = UNSET_HEIGHT;

        let pool_name = pool_alias_lookup.lookup(&source_jigsaw.pool);
        let Some(target_pool) = template_pools().get(&pool_name) else {
            continue;
        };
        if target_pool.size() == 0 && pool_name.path.as_ref() != "empty" {
            continue;
        }
        let fallback_name = target_pool.fallback().clone();
        let Some(fallback) = template_pools().get(&fallback_name) else {
            continue;
        };
        if fallback.size() == 0 && fallback_name.path.as_ref() != "empty" {
            continue;
        }

        let attach_inside_source = source_bb.is_inside(target_jigsaw_pos);
        let children_free = if attach_inside_source {
            let free = source_free.get_or_insert_with(|| {
                Rc::new(RefCell::new(shapes::create_aabb(BlockLocalAabb::of(
                    source_bb,
                ))))
            });
            free.clone()
        } else {
            context_free.clone()
        };

        let mut target_pieces: Vec<&'static StructurePoolElement> = Vec::new();
        if depth != max_depth {
            target_pieces
                .extend(context.with_random(|random| target_pool.get_shuffled_templates(random)));
        }
        target_pieces.extend(context.with_random(|random| fallback.get_shuffled_templates(random)));

        let placement_priority = source_jigsaw.placement_priority;

        for target_element in target_pieces {
            if matches!(target_element, StructurePoolElement::Empty) {
                break;
            }

            for target_rotation in context.with_random(Rotation::get_shuffled) {
                let target_jigsaws = context.with_random(|random| {
                    target_element.get_shuffled_jigsaw_blocks(
                        BlockPos::new(0, 0, 0),
                        target_rotation,
                        random,
                    )
                });
                let hack_box =
                    target_element.get_bounding_box(BlockPos::new(0, 0, 0), target_rotation);
                let expand_to = if do_expansion_hack && hack_box.y_span() <= 16 {
                    target_jigsaws
                        .iter()
                        .map(|target_jigsaw| {
                            let target_jigsaw_info = &target_jigsaw.info;
                            if !hack_box.is_inside(
                                target_jigsaw_info
                                    .pos
                                    .relative(front_facing(target_jigsaw_info.state)),
                            ) {
                                return 0;
                            }
                            let child_pool_name = pool_alias_lookup.lookup(&target_jigsaw.pool);
                            let child_pool = template_pools().get(&child_pool_name);
                            let child_fallback = child_pool.and_then(|pool| {
                                template_pools().get(pool.fallback())
                            });
                            let child_pool_size =
                                child_pool.map_or(0, StructureTemplatePool::get_max_size);
                            let child_fallback_size =
                                child_fallback.map_or(0, StructureTemplatePool::get_max_size);
                            child_pool_size.max(child_fallback_size)
                        })
                        .max()
                        .unwrap_or(0)
                } else {
                    0
                };

                for target_jigsaw in &target_jigsaws {
                    if !can_attach(&source_jigsaw, target_jigsaw) {
                        continue;
                    }

                    let target_jigsaw_local_pos = target_jigsaw.info.pos;
                    let raw_target_box_pos = BlockPos(target_jigsaw_pos.0 - target_jigsaw_local_pos.0);
                    let raw_target_bb =
                        target_element.get_bounding_box(raw_target_box_pos, target_rotation);
                    let raw_target_y = raw_target_bb.min_y();
                    let target_projection = target_element.projection();
                    let target_rigid = target_projection == Projection::Rigid;
                    let target_jigsaw_local_y = target_jigsaw_local_pos.y();
                    let delta_y = source_jigsaw_local_y - target_jigsaw_local_y
                        + source_direction.offset_vec().y;

                    let target_box_y = if source_rigid && target_rigid {
                        source_box_y + delta_y
                    } else {
                        if source_jigsaw_base_height == UNSET_HEIGHT {
                            source_jigsaw_base_height = first_free_height(
                                context.generator,
                                source_jigsaw_pos.x(),
                                source_jigsaw_pos.z(),
                            );
                        }
                        source_jigsaw_base_height - target_jigsaw_local_y
                    };

                    let y_offset = target_box_y - raw_target_y;
                    let mut target_bb = raw_target_bb.moved(0, y_offset, 0);
                    let target_box_position = raw_target_box_pos.offset(0, y_offset, 0);
                    if expand_to > 0 {
                        let new_size = (expand_to + 1).max(target_bb.max_y() - target_bb.min_y());
                        target_bb = target_bb.encapsulate(BlockPos::new(
                            target_bb.min_x(),
                            target_bb.min_y() + new_size,
                            target_bb.min_z(),
                        ));
                    }

                    let fits = {
                        let free = children_free.borrow();
                        !shapes::join_is_not_empty(
                            &free,
                            &shapes::create_aabb(BlockLocalAabb::of(target_bb).deflate(0.25)),
                            BooleanOp::OnlySecond,
                        )
                    };
                    if !fits {
                        continue;
                    }

                    {
                        let mut free = children_free.borrow_mut();
                        *free = shapes::join_unoptimized(
                            &free,
                            &shapes::create_aabb(BlockLocalAabb::of(target_bb)),
                            BooleanOp::OnlyFirst,
                        );
                    }

                    let target_ground_level_delta = if target_rigid {
                        source_ground_level_delta - delta_y
                    } else {
                        target_element.ground_level_delta()
                    };

                    let mut target_piece = PoolElementStructurePiece::new(
                        target_element,
                        target_box_position,
                        target_ground_level_delta,
                        target_rotation,
                        target_bb,
                        liquid_settings,
                    );

                    let junction_y = if source_rigid {
                        source_box_y + source_jigsaw_local_y
                    } else if target_rigid {
                        target_box_y + target_jigsaw_local_y
                    } else {
                        if source_jigsaw_base_height == UNSET_HEIGHT {
                            source_jigsaw_base_height = first_free_height(
                                context.generator,
                                source_jigsaw_pos.x(),
                                source_jigsaw_pos.z(),
                            );
                        }
                        source_jigsaw_base_height + delta_y / 2
                    };

                    pieces[source_index].add_junction(JigsawJunction::new(
                        target_jigsaw_pos.x(),
                        junction_y - source_jigsaw_local_y + source_ground_level_delta,
                        target_jigsaw_pos.z(),
                        delta_y,
                        target_projection,
                    ));
                    target_piece.add_junction(JigsawJunction::new(
                        source_jigsaw_pos.x(),
                        junction_y - target_jigsaw_local_y + target_ground_level_delta,
                        source_jigsaw_pos.z(),
                        -delta_y,
                        source_projection,
                    ));

                    pieces.push(target_piece);
                    if depth < max_depth {
                        placing.add(
                            PieceState {
                                piece_index: pieces.len() - 1,
                                free: children_free.clone(),
                                depth: depth + 1,
                            },
                            placement_priority,
                        );
                    }
                    continue 'jigsaws;
                }
            }
        }
    }
}

fn first_free_height(generator: &Generator, x: i32, z: i32) -> i32 {
    generator.get_first_free_height(x, z, HeightmapType::WorldSurfaceWg)
}
