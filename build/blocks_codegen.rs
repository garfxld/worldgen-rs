#![expect(
    clippy::float_cmp,
    clippy::items_after_statements,
    clippy::too_many_lines,
    clippy::unwrap_used,
    reason = "build script transforms extracted vanilla data and favors direct generation code over pedantic reshaping"
)]

use core::panic;
use std::{borrow::Cow, fs};

use rustc_hash::FxHashMap;

use heck::{ToShoutySnakeCase, ToUpperCamelCase};
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BlockConfig {
    pub can_occlude: bool,
    pub force_solid_off: bool,
    pub force_solid_on: bool,
    pub dynamic_shape: bool,
    pub offset_type: Cow<'static, str>,
    pub max_horizontal_offset: f32,
    pub max_vertical_offset: f32,
    pub liquid: bool,
    pub is_air: bool,
    pub replaceable: bool,
}

impl BlockConfig {
    pub const fn new() -> Self {
        Self {
            can_occlude: true,
            force_solid_off: false,
            force_solid_on: false,
            dynamic_shape: false,
            offset_type: Cow::Borrowed("NONE"),
            max_horizontal_offset: 0.25,
            max_vertical_offset: 0.2,
            liquid: false,
            is_air: false,
            replaceable: false,
        }
    }
}

#[derive(Deserialize, Clone, Debug)]
pub struct ShapeOverwrite {
    pub offset: u16,
    pub shapes: Vec<u16>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ShapeData {
    #[serde(default, rename = "usesOffset")]
    pub uses_offset: bool,
    pub default: Vec<u16>,
    pub overwrites: Vec<ShapeOverwrite>,
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct LightProperties {
    pub light_emission: u8,
    pub light_dampening: u8,
    pub use_shape_for_light_occlusion: bool,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LightPropertiesOverwrite {
    pub offset: u16,
    pub light_emission: u8,
    pub light_dampening: u8,
    pub use_shape_for_light_occlusion: bool,
}

#[derive(Deserialize, Clone, Debug)]
pub struct LightPropertiesData {
    pub default: LightProperties,
    pub overwrites: Vec<LightPropertiesOverwrite>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct BooleanOverwrite {
    pub offset: u16,
    pub value: bool,
}

#[derive(Deserialize, Clone, Debug)]
pub struct StateBooleanData {
    pub default: bool,
    pub overwrites: Vec<BooleanOverwrite>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct StateFluidValue {
    pub fluid: String,
    pub amount: u8,
    pub falling: bool,
}

#[derive(Deserialize, Clone, Debug)]
pub struct StateFluidOverwriteData {
    pub offset: u16,
    pub fluid: String,
    pub amount: u8,
    pub falling: bool,
}

#[derive(Deserialize, Clone, Debug)]
pub struct StateFluidData {
    pub default: StateFluidValue,
    pub overwrites: Vec<StateFluidOverwriteData>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Block {
    #[expect(
        dead_code,
        reason = "extracted block ids are retained for validation context"
    )]
    pub id: u16,
    pub name: String,
    pub properties: Vec<String>,
    pub default_properties: Vec<String>,
    pub behavior_properties: BlockConfig,
    pub collision_shapes: ShapeData,
    pub support_shapes: ShapeData,
    pub outline_shapes: ShapeData,
    pub occlusion_shapes: ShapeData,
    pub light_properties: LightPropertiesData,
    pub fluid_state: StateFluidData,
    pub suffocating: StateBooleanData,
    pub redstone_conductor: StateBooleanData,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Shape {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

#[derive(Deserialize, Clone, Debug)]
pub struct BlockAssets {
    pub blocks: Vec<Block>,
    pub shapes: Vec<Shape>,
}

fn offset_type_to_tokens(offset_type: &str) -> TokenStream {
    match offset_type {
        "NONE" => quote! { OffsetType::None },
        "XZ" => quote! { OffsetType::Xz },
        "XYZ" => quote! { OffsetType::Xyz },
        _ => panic!("Unknown offset type: {offset_type}"),
    }
}

fn generate_builder_calls(bp: &BlockConfig, default_props: &BlockConfig) -> Vec<TokenStream> {
    let mut builder_calls = Vec::new();

    if bp.can_occlude != default_props.can_occlude {
        let val = bp.can_occlude;
        builder_calls.push(quote! { .can_occlude(#val) });
    }
    if bp.force_solid_off != default_props.force_solid_off {
        let val = bp.force_solid_off;
        builder_calls.push(quote! { .force_solid_off(#val) });
    }
    if bp.force_solid_on != default_props.force_solid_on {
        let val = bp.force_solid_on;
        builder_calls.push(quote! { .force_solid_on(#val) });
    }
    if bp.dynamic_shape != default_props.dynamic_shape {
        let val = bp.dynamic_shape;
        builder_calls.push(quote! { .dynamic_shape(#val) });
    }
    if bp.offset_type != default_props.offset_type {
        let offset_type = offset_type_to_tokens(bp.offset_type.as_ref());
        builder_calls.push(quote! { .offset_type(#offset_type) });
    }
    if bp.max_horizontal_offset != default_props.max_horizontal_offset {
        let val = bp.max_horizontal_offset;
        builder_calls.push(quote! { .max_horizontal_offset(#val) });
    }
    if bp.max_vertical_offset != default_props.max_vertical_offset {
        let val = bp.max_vertical_offset;
        builder_calls.push(quote! { .max_vertical_offset(#val) });
    }
    if bp.liquid != default_props.liquid {
        let val = bp.liquid;
        builder_calls.push(quote! { .liquid(#val) });
    }
    if bp.is_air != default_props.is_air {
        let val = bp.is_air;
        builder_calls.push(quote! { .set_is_air(#val) });
    }
    if bp.replaceable != default_props.replaceable {
        let val = bp.replaceable;
        builder_calls.push(quote! { .replaceable(#val) });
    }

    builder_calls
}

fn generate_default_state(block: &Block) -> TokenStream {
    if block.properties.is_empty() || block.default_properties.is_empty() {
        return quote! {};
    }

    let property_values = block
        .properties
        .iter()
        .zip(block.default_properties.iter())
        .map(|(prop_name, default_val)| {
            let property_ident =
                Ident::new(&prop_name.to_shouty_snake_case(), Span::call_site());

            let value_expr = if default_val.starts_with("bool_") {
                let bool_val = default_val == "bool_true";
                quote! {
                    Properties::#property_ident.index_of(#bool_val)
                }
            } else if default_val.starts_with("int_") {
                let int_val = default_val
                    .strip_prefix("int_")
                    .unwrap()
                    .parse::<u8>()
                    .unwrap();
                quote! { Properties::#property_ident.get_internal_index_const(&#int_val) }
            } else if default_val.starts_with("enum_") {
                let enum_part = default_val.strip_prefix("enum_").unwrap();
                let parts: Vec<&str> = enum_part.split('_').collect();

                if parts.len() >= 2 {
                    let enum_type = parts[0];
                    let variant_name = parts[1..].join("_");

                    let enum_type_ident = Ident::new(enum_type, Span::call_site());
                    let variant_ident =
                        Ident::new(&variant_name.to_upper_camel_case(), Span::call_site());

                    quote! { Properties::#property_ident.get_internal_index_const(&properties::#enum_type_ident::#variant_ident) }
                } else {
                    quote! { 0 }
                }
            } else {
                quote! { 0 }
            };

            quote! {
                Properties::#property_ident => #value_expr
            }
        })
        .collect::<Vec<_>>();

    quote! {
        .with_default_state(offset!(
            #(#property_values),*
        ))
    }
}

struct VoxelShapePool {
    shapes: FxHashMap<Vec<u16>, u16>,
    shape_list: Vec<Vec<u16>>,
}

impl VoxelShapePool {
    fn new() -> Self {
        let mut pool = Self {
            shapes: FxHashMap::default(),
            shape_list: Vec::new(),
        };
        pool.get_or_insert(vec![]); // EMPTY = 0
        pool.get_or_insert(vec![u16::MAX]); // FULL_BLOCK = 1 (special marker)
        pool
    }

    fn get_or_insert(&mut self, aabb_indices: Vec<u16>) -> u16 {
        if let Some(&id) = self.shapes.get(&aabb_indices) {
            return id;
        }
        let id = self.shape_list.len() as u16;
        self.shapes.insert(aabb_indices.clone(), id);
        self.shape_list.push(aabb_indices);
        id
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct ShapeFunctionSignature {
    default_id: u16,
    arms: Vec<(Vec<u16>, u16)>,
}

struct ShapeFunctionPool {
    functions: FxHashMap<ShapeFunctionSignature, u16>,
    function_list: Vec<ShapeFunctionSignature>,
}

impl ShapeFunctionPool {
    fn new() -> Self {
        Self {
            functions: FxHashMap::default(),
            function_list: Vec::new(),
        }
    }

    fn get_or_insert(&mut self, sig: ShapeFunctionSignature) -> u16 {
        if let Some(&id) = self.functions.get(&sig) {
            return id;
        }
        let id = self.function_list.len() as u16;
        self.functions.insert(sig.clone(), id);
        self.function_list.push(sig);
        id
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct LightFunctionSignature {
    default: LightProperties,
    arms: Vec<(Vec<u16>, LightProperties)>,
}

struct LightFunctionPool {
    functions: FxHashMap<LightFunctionSignature, u16>,
    function_list: Vec<LightFunctionSignature>,
}

impl LightFunctionPool {
    fn new() -> Self {
        Self {
            functions: FxHashMap::default(),
            function_list: Vec::new(),
        }
    }

    fn get_or_insert(&mut self, sig: LightFunctionSignature) -> u16 {
        if let Some(&id) = self.functions.get(&sig) {
            return id;
        }
        let id = self.function_list.len() as u16;
        self.functions.insert(sig.clone(), id);
        self.function_list.push(sig);
        id
    }
}

fn generate_shape_match(
    shape_data: &ShapeData,
    voxel_pool: &mut VoxelShapePool,
) -> (u16, Vec<(Vec<u16>, u16)>) {
    let default_id = voxel_pool.get_or_insert(shape_data.default.clone());

    let mut shape_to_offsets: FxHashMap<Vec<u16>, Vec<u16>> = FxHashMap::default();
    for overwrite in &shape_data.overwrites {
        shape_to_offsets
            .entry(overwrite.shapes.clone())
            .or_default()
            .push(overwrite.offset);
    }

    let mut arms: Vec<(Vec<u16>, u16)> = shape_to_offsets
        .into_iter()
        .map(|(shapes, mut offsets)| {
            offsets.sort_unstable();
            let shape_id = voxel_pool.get_or_insert(shapes);
            (offsets, shape_id)
        })
        .collect();

    arms.sort_by_key(|(offsets, _)| offsets.first().copied().unwrap_or(0));

    (default_id, arms)
}

fn light_properties_to_tokens(properties: &LightProperties) -> TokenStream {
    let light_emission = properties.light_emission;
    let light_dampening = properties.light_dampening;
    let use_shape_for_light_occlusion = properties.use_shape_for_light_occlusion;
    quote! {
        BlockLightProperties {
            light_emission: #light_emission,
            light_dampening: #light_dampening,
            use_shape_for_light_occlusion: #use_shape_for_light_occlusion,
        }
    }
}

fn generate_light_match(light_data: &LightPropertiesData) -> LightFunctionSignature {
    let mut properties_to_offsets: FxHashMap<LightProperties, Vec<u16>> = FxHashMap::default();
    for overwrite in &light_data.overwrites {
        properties_to_offsets
            .entry(LightProperties {
                light_emission: overwrite.light_emission,
                light_dampening: overwrite.light_dampening,
                use_shape_for_light_occlusion: overwrite.use_shape_for_light_occlusion,
            })
            .or_default()
            .push(overwrite.offset);
    }

    let mut arms: Vec<(Vec<u16>, LightProperties)> = properties_to_offsets
        .into_iter()
        .map(|(properties, mut offsets)| {
            offsets.sort_unstable();
            (offsets, properties)
        })
        .collect();

    arms.sort_by_key(|(offsets, _)| offsets.first().copied().unwrap_or(0));

    LightFunctionSignature {
        default: light_data.default.clone(),
        arms,
    }
}

pub fn build() -> TokenStream {
    println!("cargo:rerun-if-changed=assets/blocks.json");
    let block_assets: BlockAssets =
        serde_json::from_str(&fs::read_to_string("assets/blocks.json").unwrap()).unwrap();

    let default_props = BlockConfig::new();

    let mut voxel_pool = VoxelShapePool::new();

    let mut shape_fn_pool = ShapeFunctionPool::new();

    let mut light_fn_pool = LightFunctionPool::new();

    struct BlockShapeInfo {
        collision_fn_id: u16,
        support_fn_id: u16,
        outline_fn_id: u16,
        occlusion_fn_id: u16,
        light_fn_id: u16,
        collision_uses_offset: bool,
        support_uses_offset: bool,
        outline_uses_offset: bool,
        occlusion_uses_offset: bool,
    }
    let mut block_shape_infos: Vec<BlockShapeInfo> = Vec::new();

    for block in &block_assets.blocks {
        let (collision_default, collision_arms) =
            generate_shape_match(&block.collision_shapes, &mut voxel_pool);
        let (support_default, support_arms) =
            generate_shape_match(&block.support_shapes, &mut voxel_pool);
        let (outline_default, outline_arms) =
            generate_shape_match(&block.outline_shapes, &mut voxel_pool);
        let (occlusion_default, occlusion_arms) =
            generate_shape_match(&block.occlusion_shapes, &mut voxel_pool);

        let collision_sig = ShapeFunctionSignature {
            default_id: collision_default,
            arms: collision_arms,
        };
        let support_sig = ShapeFunctionSignature {
            default_id: support_default,
            arms: support_arms,
        };
        let outline_sig = ShapeFunctionSignature {
            default_id: outline_default,
            arms: outline_arms,
        };
        let occlusion_sig = ShapeFunctionSignature {
            default_id: occlusion_default,
            arms: occlusion_arms,
        };

        let collision_fn_id = shape_fn_pool.get_or_insert(collision_sig);
        let support_fn_id = shape_fn_pool.get_or_insert(support_sig);
        let outline_fn_id = shape_fn_pool.get_or_insert(outline_sig);
        let occlusion_fn_id = shape_fn_pool.get_or_insert(occlusion_sig);
        let light_fn_id =
            light_fn_pool.get_or_insert(generate_light_match(&block.light_properties));

        block_shape_infos.push(BlockShapeInfo {
            collision_fn_id,
            support_fn_id,
            outline_fn_id,
            occlusion_fn_id,
            light_fn_id,
            collision_uses_offset: block.collision_shapes.uses_offset,
            support_uses_offset: block.support_shapes.uses_offset,
            outline_uses_offset: block.outline_shapes.uses_offset,
            occlusion_uses_offset: block.occlusion_shapes.uses_offset,
        });
    }

    let aabb_consts: Vec<TokenStream> = block_assets
        .shapes
        .iter()
        .enumerate()
        .map(|(i, shape)| {
            let name = Ident::new(&format!("BOX_{i}"), Span::call_site());
            let min_x = shape.min[0];
            let min_y = shape.min[1];
            let min_z = shape.min[2];
            let max_x = shape.max[0];
            let max_y = shape.max[1];
            let max_z = shape.max[2];
            quote! {
                const #name: BlockLocalAabb =
                    BlockLocalAabb::new(#min_x, #min_y, #min_z, #max_x, #max_y, #max_z);
            }
        })
        .collect();

    let voxel_shape_consts: Vec<TokenStream> = voxel_pool
        .shape_list
        .iter()
        .enumerate()
        .map(|(id, aabb_indices)| {
            let name = Ident::new(&format!("VSHAPE_{id}"), Span::call_site());
            if aabb_indices.is_empty() {
                quote! {
                    const #name: VoxelShape = VoxelShape::EMPTY;
                }
            } else if aabb_indices.len() == 1 && aabb_indices[0] == u16::MAX {
                quote! {
                    const #name: VoxelShape = VoxelShape::FULL_BLOCK;
                }
            } else {
                let aabb_refs: Vec<TokenStream> = aabb_indices
                    .iter()
                    .map(|&idx| {
                        let aabb_name = Ident::new(&format!("BOX_{idx}"), Span::call_site());
                        quote! { #aabb_name }
                    })
                    .collect();
                quote! {
                    const #name: VoxelShape = VoxelShape::from_boxes(&[#(#aabb_refs),*]);
                }
            }
        })
        .collect();

    let mut shape_fns = TokenStream::new();

    for (fn_id, sig) in shape_fn_pool.function_list.iter().enumerate() {
        let fn_name = Ident::new(&format!("shape_fn_{fn_id}"), Span::call_site());
        let default_shape = Ident::new(&format!("VSHAPE_{}", sig.default_id), Span::call_site());

        if sig.arms.is_empty() {
            shape_fns.extend(quote! {
                #[inline]
                const fn #fn_name(_offset: u16) -> VoxelShape {
                    #default_shape
                }
            });
        } else {
            let arms: Vec<TokenStream> = sig
                .arms
                .iter()
                .map(|(offsets, shape_id)| {
                    let shape_name = Ident::new(&format!("VSHAPE_{shape_id}"), Span::call_site());
                    let patterns: Vec<TokenStream> = offsets
                        .iter()
                        .map(|&o| {
                            quote! { #o }
                        })
                        .collect();
                    quote! {
                        #(#patterns)|* => #shape_name,
                    }
                })
                .collect();

            shape_fns.extend(quote! {
                #[inline]
                fn #fn_name(offset: u16) -> VoxelShape {
                    match offset {
                        #(#arms)*
                        _ => #default_shape,
                    }
                }
            });
        }
    }

    let mut light_fns = TokenStream::new();

    for (fn_id, sig) in light_fn_pool.function_list.iter().enumerate() {
        let fn_name = Ident::new(&format!("light_fn_{fn_id}"), Span::call_site());
        let default_properties = light_properties_to_tokens(&sig.default);

        if sig.arms.is_empty() {
            light_fns.extend(quote! {
                #[inline]
                const fn #fn_name(_offset: u16) -> BlockLightProperties {
                    #default_properties
                }
            });
        } else {
            let arms: Vec<TokenStream> = sig
                .arms
                .iter()
                .map(|(offsets, properties)| {
                    let properties = light_properties_to_tokens(properties);
                    let patterns: Vec<TokenStream> = offsets
                        .iter()
                        .map(|&o| {
                            quote! { #o }
                        })
                        .collect();
                    quote! {
                        #(#patterns)|* => #properties,
                    }
                })
                .collect();

            light_fns.extend(quote! {
                #[inline]
                fn #fn_name(offset: u16) -> BlockLightProperties {
                    match offset {
                        #(#arms)*
                        _ => #default_properties,
                    }
                }
            });
        }
    }

    let mut stream = TokenStream::new();

    for (block, info) in block_assets.blocks.iter().zip(block_shape_infos.iter()) {
        let block_name = Ident::new(&block.name.to_shouty_snake_case(), Span::call_site());
        let block_name_str = block.name.clone();
        let properties = block
            .properties
            .iter()
            .map(|p| {
                let property_name = Ident::new(&p.to_shouty_snake_case(), Span::call_site());
                quote! {
                    &Properties::#property_name
                }
            })
            .collect::<Vec<_>>();

        let builder_calls = generate_builder_calls(&block.behavior_properties, &default_props);

        let default_state = generate_default_state(block);

        let suffocating_default = block.suffocating.default;
        let suffocating_overwrites = block
            .suffocating
            .overwrites
            .iter()
            .map(|overwrite| {
                let offset = overwrite.offset;
                let value = overwrite.value;
                quote! { StateBooleanOverwrite::new(#offset, #value) }
            })
            .collect::<Vec<_>>();
        let redstone_conductor_default = block.redstone_conductor.default;
        let redstone_conductor_overwrites = block
            .redstone_conductor
            .overwrites
            .iter()
            .map(|overwrite| {
                let offset = overwrite.offset;
                let value = overwrite.value;
                quote! { StateBooleanOverwrite::new(#offset, #value) }
            })
            .collect::<Vec<_>>();
        let default_fluid = Ident::new(
            &block.fluid_state.default.fluid.to_shouty_snake_case(),
            Span::call_site(),
        );
        let default_fluid_amount = block.fluid_state.default.amount;
        let default_fluid_falling = block.fluid_state.default.falling;
        let fluid_overwrites = block
            .fluid_state
            .overwrites
            .iter()
            .map(|overwrite| {
                let offset = overwrite.offset;
                let fluid = Ident::new(&overwrite.fluid.to_shouty_snake_case(), Span::call_site());
                let amount = overwrite.amount;
                let falling = overwrite.falling;
                quote! {
                    StateFluidOverwrite::new(
                        #offset,
                        FluidState::new(&vanilla_fluids::#fluid, #amount, #falling),
                    )
                }
            })
            .collect::<Vec<_>>();

        let collision_fn = Ident::new(
            &format!("shape_fn_{}", info.collision_fn_id),
            Span::call_site(),
        );
        let support_fn = Ident::new(
            &format!("shape_fn_{}", info.support_fn_id),
            Span::call_site(),
        );
        let outline_fn = Ident::new(
            &format!("shape_fn_{}", info.outline_fn_id),
            Span::call_site(),
        );
        let occlusion_fn = Ident::new(
            &format!("shape_fn_{}", info.occlusion_fn_id),
            Span::call_site(),
        );
        let light_fn = Ident::new(&format!("light_fn_{}", info.light_fn_id), Span::call_site());
        let shape_offsets = if info.collision_uses_offset
            || info.support_uses_offset
            || info.outline_uses_offset
            || info.occlusion_uses_offset
        {
            let collision = info.collision_uses_offset;
            let support = info.support_uses_offset;
            let outline = info.outline_uses_offset;
            let occlusion = info.occlusion_uses_offset;
            quote! {
                .with_shape_offsets(ShapeOffsetFlags::new(
                    #collision,
                    #support,
                    #outline,
                    #occlusion,
                ))
            }
        } else {
            quote! {}
        };

        stream.extend(quote! {
            pub static #block_name: Block = Block::new(
                Identifier::const_vanilla(#block_name_str),
                BlockConfig::new()#(#builder_calls)*,
                &[
                    #(#properties),*
                ],
            ).with_light_properties(
                #light_fn,
            ).with_shapes(
                #collision_fn,
                #support_fn,
                #outline_fn,
                #occlusion_fn,
            ).with_suffocating(
                StateBooleanData::new(
                    #suffocating_default,
                    &[#(#suffocating_overwrites),*],
                ),
            ).with_redstone_conductor(
                StateBooleanData::new(
                    #redstone_conductor_default,
                    &[#(#redstone_conductor_overwrites),*],
                ),
            ).with_fluid_state(
                StateFluidData::new(
                    FluidState::new(
                        &vanilla_fluids::#default_fluid,
                        #default_fluid_amount,
                        #default_fluid_falling,
                    ),
                    &[#(#fluid_overwrites),*],
                ),
            ) #shape_offsets #default_state;
        });
    }

    let mut register_stream = TokenStream::new();

    for block in &block_assets.blocks {
        let block_name = Ident::new(&block.name.to_shouty_snake_case(), Span::call_site());

        register_stream.extend(quote! {
            registry.register(&#block_name);
        });
    }

    println!("cargo:rerun-if-changed=assets/block_transforms.json");
    let transform_assets: FxHashMap<String, String> =
        serde_json::from_str(&fs::read_to_string("assets/block_transforms.json").unwrap()).unwrap();

    let transform_entries: Vec<TokenStream> = block_assets
        .blocks
        .iter()
        .map(|block| {
            let variant = match transform_assets
                .get::<str>(block.name.as_ref())
                .map_or("none", String::as_str)
            {
                "none" => "None",
                "facing" => "Facing",
                "facing_rod" => "FacingRod",
                "stairs" => "Stairs",
                "door" => "Door",
                "rotation16" => "Rotation16",
                "pillar" => "Pillar",
                "connections" => "Connections",
                "facemap6" => "FaceMap",
                "rail" => "Rail",
                "orientation" => "Orientation",
                other => panic!("unknown block transform kind {other}"),
            };
            let variant = Ident::new(variant, Span::call_site());
            quote! { BlockTransform::#variant }
        })
        .collect();
    let transform_count = transform_entries.len();

    quote! {
        use crate::registry::blocks::{
            config::{BlockConfig, OffsetType},
            shapes::ShapeOffsetFlags,
            Block, BlockLightProperties, BlockRegistry, StateBooleanData,
            StateBooleanOverwrite, StateFluidData, StateFluidOverwrite, offset,
        };
        use crate::registry::blocks::transform::BlockTransform;
        use crate::registry::blocks::properties::{self, Properties};
        use crate::registry::blocks::shapes::VoxelShape;
        use crate::registry::fluid::FluidState;
        use crate::registry::fluid::vanilla_fluids;
        use crate::util::geometry::BlockLocalAabb;
        use crate::util::ident::Identifier;

        #(#aabb_consts)*

        #(#voxel_shape_consts)*

        #shape_fns

        #light_fns

        #stream

        pub static BLOCK_TRANSFORMS: [BlockTransform; #transform_count] = [#(#transform_entries),*];

        pub fn register_blocks(registry: &mut BlockRegistry) {
            #register_stream
        }
    }
}
