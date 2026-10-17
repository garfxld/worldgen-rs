use crate::util::hash_set::JavaHashSet;
use crate::util::rng::RandomSource;
use crate::util::value_providers::int::IntProvider;
use crate::world::worldgen::block_predicate::HolderSet;
use crate::world::worldgen::feature::PlacedFeature;
use crate::world::worldgen::feature::state_provider::ProviderHolder;
use crate::world::worldgen::placement::CaveSurface;
use crate::world::worldgen::region::WorldGenRegion;
use shulkr::util::BlockPosition;
use shulkr::world::block::Block;
use shulkr::world::block::Properties;

#[derive(Debug)]
pub struct VegetationPatchConfig {
    pub replaceable: HolderSet,
    pub ground_state: ProviderHolder,
    pub vegetation_feature: &'static PlacedFeature,
    pub surface: CaveSurface,
    pub depth: IntProvider,
    pub extra_bottom_block_chance: f32,
    pub vertical_range: i32,
    pub vegetation_chance: f32,
    pub xz_radius: IntProvider,
    pub extra_edge_column_chance: f32,
}

pub fn place(
    region: &WorldGenRegion<'_>,
    random: &mut RandomSource,
    origin: BlockPosition,
    config: &VegetationPatchConfig,
    waterlogged: bool,
) -> bool {
    let x_radius = config.xz_radius.sample(random) + 1;
    let z_radius = config.xz_radius.sample(random) + 1;
    let surface = place_ground_patch(region, config, random, origin, x_radius, z_radius);
    let surface = if waterlogged {
        waterlog(region, &surface)
    } else {
        surface
    };
    distribute_vegetation(region, config, random, &surface, waterlogged);
    !surface.is_empty()
}

fn place_ground_patch(
    region: &WorldGenRegion<'_>,
    config: &VegetationPatchConfig,
    random: &mut RandomSource,
    origin: BlockPosition,
    x_radius: i32,
    z_radius: i32,
) -> JavaHashSet {
    let inwards = config.surface.direction();
    let outwards = inwards.opposite();
    let mut surface = JavaHashSet::new();

    for dx in -x_radius..=x_radius {
        let is_x_edge = dx == -x_radius || dx == x_radius;
        for dz in -z_radius..=z_radius {
            let is_z_edge = dz == -z_radius || dz == z_radius;
            let is_edge = is_x_edge || is_z_edge;
            let is_corner = is_x_edge && is_z_edge;
            let is_edge_but_not_corner = is_edge && !is_corner;

            let process = !is_corner
                && (!is_edge_but_not_corner
                    || (config.extra_edge_column_chance != 0.0
                        && random.next_f32() <= config.extra_edge_column_chance));
            if !process {
                continue;
            }

            let mut pos = origin.offset(dx, 0, dz);

            let mut offset = 0;
            while region.get_block(pos).is_air() && offset < config.vertical_range {
                pos = pos.relative(inwards);
                offset += 1;
            }
            let mut offset = 0;
            while !region.get_block(pos).is_air() && offset < config.vertical_range {
                pos = pos.relative(outwards);
                offset += 1;
            }

            let below_pos = pos.relative(inwards);
            let below_state = region.get_block(below_pos);
            if region.get_block(pos).is_air() && below_state.is_side_solid_at(below_pos, outwards) {
                let depth = config.depth.sample(random)
                    + i32::from(
                        config.extra_bottom_block_chance > 0.0
                            && random.next_f32() < config.extra_bottom_block_chance,
                    );
                let ground_pos = below_pos;
                if place_ground(region, config, random, below_pos, depth) {
                    surface.insert(ground_pos);
                }
            }
        }
    }

    surface
}

fn place_ground(
    region: &WorldGenRegion<'_>,
    config: &VegetationPatchConfig,
    random: &mut RandomSource,
    mut below_pos: BlockPosition,
    depth: i32,
) -> bool {
    for i in 0..depth {
        let state_to_place = config.ground_state.get_state(region, random, below_pos);
        let below_state = region.get_block(below_pos);
        if state_to_place.as_block() != below_state.as_block() {
            if !config.replaceable.contains_block(below_state) {
                return i != 0;
            }
            region.set_block(below_pos, state_to_place);
            below_pos = below_pos.relative(config.surface.direction());
        }
    }
    true
}

fn distribute_vegetation(
    region: &WorldGenRegion<'_>,
    config: &VegetationPatchConfig,
    random: &mut RandomSource,
    surface: &JavaHashSet,
    waterlogged: bool,
) {
    for surface_pos in surface.iter() {
        if config.vegetation_chance > 0.0 && random.next_f32() < config.vegetation_chance {
            place_vegetation(region, config, random, surface_pos, waterlogged);
        }
    }
}

fn place_vegetation(
    region: &WorldGenRegion<'_>,
    config: &VegetationPatchConfig,
    random: &mut RandomSource,
    placement_pos: BlockPosition,
    waterlogged: bool,
) -> bool {
    if !waterlogged {
        return base_place_vegetation(region, config, random, placement_pos);
    }

    if !base_place_vegetation(region, config, random, placement_pos.below()) {
        return false;
    }

    let placed = region.get_block(placement_pos);
    if placed.get(Properties::WATERLOGGED) == Some(false) {
        region.set_block(placement_pos, placed.with(Properties::WATERLOGGED, true));
    }
    true
}

fn base_place_vegetation(
    region: &WorldGenRegion<'_>,
    config: &VegetationPatchConfig,
    random: &mut RandomSource,
    vegetation_pos: BlockPosition,
) -> bool {
    let final_pos = vegetation_pos.relative(config.surface.direction().opposite());
    config.vegetation_feature.place(region, random, final_pos)
}

fn waterlog(region: &WorldGenRegion<'_>, surface: &JavaHashSet) -> JavaHashSet {
    let water_surface: JavaHashSet = surface
        .iter()
        .filter(|&pos| !is_exposed(region, pos))
        .collect();
    let water = Block::WATER.default_state();
    for pos in water_surface.iter() {
        region.set_block(pos, water);
    }
    water_surface
}

fn is_exposed(region: &WorldGenRegion<'_>, pos: BlockPosition) -> bool {
    use shulkr::util::Direction;

    [
        Direction::North,
        Direction::East,
        Direction::South,
        Direction::West,
        Direction::Down,
    ]
    .into_iter()
    .any(|direction| {
        let test_pos = pos.relative(direction);
        !region
            .get_block(test_pos)
            .is_side_solid_at(test_pos, direction.opposite())
    })
}
