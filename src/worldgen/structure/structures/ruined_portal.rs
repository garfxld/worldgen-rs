use crate::util::geometry::BoundingBox;
use crate::util::ident::Identifier;
use crate::util::mirror::Mirror;
use crate::util::pos::BlockPos;
use crate::util::rng::RandomSource;
use crate::util::rotation::Rotation;
use crate::worldgen::generator::Generator;
use crate::worldgen::structure::structure::{GenerationContext, GenerationStub};
use crate::worldgen::structure::structures::ruined_portal_piece::{
    PortalProperties, RuinedPortalPiece, VerticalPlacement,
};
use crate::worldgen::structure::templatesystem::structure_template::bounding_box_of;
use crate::worldgen::structure::templatesystem::structure_template_manager::templates;

const STRUCTURE_LOCATION_PORTALS: &[&str] = &[
    "ruined_portal/portal_1",
    "ruined_portal/portal_2",
    "ruined_portal/portal_3",
    "ruined_portal/portal_4",
    "ruined_portal/portal_5",
    "ruined_portal/portal_6",
    "ruined_portal/portal_7",
    "ruined_portal/portal_8",
    "ruined_portal/portal_9",
    "ruined_portal/portal_10",
];

const STRUCTURE_LOCATION_GIANT_PORTALS: &[&str] = &[
    "ruined_portal/giant_portal_1",
    "ruined_portal/giant_portal_2",
    "ruined_portal/giant_portal_3",
];

const MIN_Y_INDEX: i32 = 15;

#[derive(Debug, Clone, Copy)]
pub struct Setup {
    pub placement: VerticalPlacement,
    pub air_pocket_probability: f32,
    pub mossiness: f32,
    pub overgrown: bool,
    pub vines: bool,
    pub can_be_cold: bool,
    pub replace_with_blackstone: bool,
    pub weight: f32,
}

fn sample(random: &mut RandomSource, limit: f32) -> bool {
    if limit == 0.0 {
        false
    } else if limit == 1.0 {
        true
    } else {
        random.next_f32() < limit
    }
}

fn random_within_interval(random: &mut RandomSource, min_preferred: i32, max: i32) -> i32 {
    if min_preferred < max {
        random.next_i32_between(min_preferred, max)
    } else {
        max
    }
}

#[expect(clippy::too_many_arguments, reason = "mirrors vanilla findSuitableY")]
fn find_suitable_y(
    random: &mut RandomSource,
    generator: &Generator,
    vertical_placement: VerticalPlacement,
    air_pocket: bool,
    surface_y_at_center: i32,
    y_span: i32,
    bounding_box: BoundingBox,
    min_y: i32,
) -> i32 {
    let min_y = min_y + MIN_Y_INDEX;
    let new_y = match vertical_placement {
        VerticalPlacement::InNether => {
            if air_pocket {
                random.next_i32_between(32, 100)
            } else if random.next_f32() < 0.5 {
                random.next_i32_between(27, 29)
            } else {
                random.next_i32_between(29, 100)
            }
        }
        VerticalPlacement::InMountain => {
            random_within_interval(random, 70, surface_y_at_center - y_span)
        }
        VerticalPlacement::Underground => {
            random_within_interval(random, min_y, surface_y_at_center - y_span)
        }
        VerticalPlacement::PartlyBuried => {
            surface_y_at_center - y_span + random.next_i32_between(2, 8)
        }
        VerticalPlacement::OnLandSurface | VerticalPlacement::OnOceanFloor => surface_y_at_center,
    };

    let corners = [
        (bounding_box.min_x(), bounding_box.min_z()),
        (bounding_box.max_x(), bounding_box.min_z()),
        (bounding_box.min_x(), bounding_box.max_z()),
        (bounding_box.max_x(), bounding_box.max_z()),
    ];
    let columns: Vec<_> = corners
        .iter()
        .map(|(x, z)| generator.get_base_column(*x, *z))
        .collect();
    let heightmap = vertical_placement.heightmap();

    let mut projected_y = new_y;
    while projected_y > min_y {
        let mut corners_on_solid_ground = 0;
        for column in &columns {
            if heightmap.is_opaque(column.get_block(projected_y)) {
                corners_on_solid_ground += 1;
                if corners_on_solid_ground == 3 {
                    return projected_y;
                }
            }
        }
        projected_y -= 1;
    }

    projected_y
}

pub fn find_generation_point<'a>(
    context: &'a GenerationContext<'a>,
    setups: &'a [Setup],
) -> Option<GenerationStub<'a>> {
    let chosen_setup = context.with_random(|random| {
        if setups.len() > 1 {
            let total: f32 = setups.iter().map(|setup| setup.weight).sum();
            let mut pick = random.next_f32();
            for setup in setups {
                pick -= setup.weight / total;
                if pick < 0.0 {
                    return *setup;
                }
            }
            setups[setups.len() - 1]
        } else {
            setups[0]
        }
    });

    let air_pocket =
        context.with_random(|random| sample(random, chosen_setup.air_pocket_probability));
    let template_location: Identifier = context.with_random(|random| {
        let pool = if random.next_f32() < 0.05 {
            STRUCTURE_LOCATION_GIANT_PORTALS
        } else {
            STRUCTURE_LOCATION_PORTALS
        };
        let index = random.next_i32_bounded(pool.len() as i32) as usize;
        format!("minecraft:{}", pool[index])
            .parse()
            .expect("ruined portal template path should be a valid identifier")
    });

    let template = templates().get_or_create(&template_location);
    let rotation = context.with_random(Rotation::get_random);
    let mirror = context.with_random(|random| {
        if random.next_f32() < 0.5 {
            Mirror::None
        } else {
            Mirror::FrontBack
        }
    });
    let size = template.size();
    let pivot = BlockPos::new(size.x / 2, 0, size.z / 2);
    let chunk_pos = context.chunk_pos;
    let base_position = BlockPos::new(chunk_pos.0.x * 16, 0, chunk_pos.0.y * 16);
    let bounding_box = bounding_box_of(base_position, rotation, pivot, mirror, size);
    let center = bounding_box.center();
    let surface_y = context.generator.get_base_height(
        center.x(),
        center.z(),
        chosen_setup.placement.heightmap(),
    ) - 1;
    let projected_y = context.with_random(|random| {
        find_suitable_y(
            random,
            context.generator,
            chosen_setup.placement,
            air_pocket,
            surface_y,
            bounding_box.y_span(),
            bounding_box,
            context.min_y,
        )
    });
    let origin = BlockPos::new(base_position.x(), projected_y, base_position.z());

    Some(GenerationStub {
        position: origin,
        build: Box::new(move |builder| {
            let cold = chosen_setup.can_be_cold && is_cold(context, origin);
            let properties = PortalProperties {
                cold,
                mossiness: chosen_setup.mossiness,
                air_pocket,
                overgrown: chosen_setup.overgrown,
                vines: chosen_setup.vines,
                replace_with_blackstone: chosen_setup.replace_with_blackstone,
            };
            builder.add_piece(RuinedPortalPiece::new(
                origin,
                chosen_setup.placement,
                properties,
                &template_location,
                rotation,
                mirror,
                pivot,
            ));
        }),
    })
}

fn is_cold(context: &GenerationContext<'_>, origin: BlockPos) -> bool {
    let mut sampler = context.generator.biome_source().chunk_sampler();
    sampler.init_grid(origin.x() & !15, origin.z() & !15);
    let biome = sampler.sample(origin.x() >> 2, origin.y() >> 2, origin.z() >> 2);
    biome.cold_enough_to_snow(origin, context.generator.sea_level())
}
