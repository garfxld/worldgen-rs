use std::mem::transmute;

use cranelift_codegen::Context;
use cranelift_codegen::ir::{
    AbiParam, Function, InstBuilder, MemFlagsData, Signature, UserFuncName, types,
};
use cranelift_codegen::isa::CallConv;
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_module::{FuncId, Linkage, Module};
use rustc_hash::FxHashMap;

use crate::util::noise::{BlendedNoise, EndIslands, NormalNoise};
use crate::worldgen::backend::ir::analysis::graph::{
    collect_interpolated_inners, is_flat_cached, unwrap_markers,
};
use crate::worldgen::backend::ir::{AnalysisContext, DensityFunction, DensityProgram, Noise};

use super::emit::{
    CompileError, EmitCtx, EndIslandsCall, Position, emit_expr, plan_blended, plan_noises,
};
use super::inlining::InlinePolicy;
use super::jit_module::{END_ISLANDS_SYMBOL, JitError, JitModule};
use super::layout::{CacheLayout, FlatSlot};

pub type DensityFn = unsafe extern "C" fn(*const f64, f64, f64, f64) -> f64;
pub type FillFn = unsafe extern "C" fn(*const f64, f64, f64, f64, f64, *mut f64);
pub type Fill4xFn = unsafe extern "C" fn(*const f64, f64, *const f64, f64, *const f64, *mut f64);
pub type CombineFn = unsafe extern "C" fn(*const f64, *const f64, f64, f64, f64) -> f64;

const LANES: usize = 4;

const INTERPOLATED_ENTRIES: [&str; 3] = ["final_density", "vein_toggle", "vein_ridged"];

pub(crate) struct DimensionCompiler<'a> {
    module: JitModule,
    program: &'a DensityProgram,
    analysis: &'a AnalysisContext,
    layout: &'a CacheLayout,
    noises: &'a FxHashMap<String, NormalNoise>,
    blended: Option<&'a BlendedNoise>,
    end_islands: Option<*const EndIslands>,
    named_ids: FxHashMap<String, FuncId>,
    end_islands_id: Option<FuncId>,
    fill_id: Option<FuncId>,
    inlining: InlinePolicy,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Shape {
    Full,
    Fill,
    Fill4x,
    Combine,
}

impl Shape {
    fn signature(self, ptr: types::Type) -> Signature {
        let mut sig = Signature::new(CallConv::SystemV);
        let f64_param = AbiParam::new(types::F64);
        match self {
            Self::Full => {
                sig.params
                    .extend([AbiParam::new(ptr), f64_param, f64_param, f64_param]);
                sig.returns.push(f64_param);
            }
            Self::Fill => {
                sig.params.extend([
                    AbiParam::new(ptr),
                    f64_param,
                    f64_param,
                    f64_param,
                    f64_param,
                    AbiParam::new(ptr),
                ]);
            }
            Self::Fill4x => {
                sig.params.extend([
                    AbiParam::new(ptr),
                    f64_param,
                    AbiParam::new(ptr),
                    f64_param,
                    AbiParam::new(ptr),
                    AbiParam::new(ptr),
                ]);
            }
            Self::Combine => {
                sig.params.extend([
                    AbiParam::new(ptr),
                    AbiParam::new(ptr),
                    f64_param,
                    f64_param,
                    f64_param,
                ]);
                sig.returns.push(f64_param);
            }
        }
        sig
    }
}

impl<'a> DimensionCompiler<'a> {
    pub(crate) fn new(
        module: JitModule,
        program: &'a DensityProgram,
        analysis: &'a AnalysisContext,
        layout: &'a CacheLayout,
        noises: &'a FxHashMap<String, NormalNoise>,
        blended: Option<&'a BlendedNoise>,
        end_islands: Option<*const EndIslands>,
    ) -> Self {
        Self {
            module,
            program,
            analysis,
            layout,
            noises,
            blended,
            end_islands,
            named_ids: FxHashMap::default(),
            end_islands_id: None,
            fill_id: None,
            inlining: InlinePolicy::new(program, analysis),
        }
    }

    fn declare_end_islands(&mut self) -> Result<(), CompileError> {
        if self.end_islands.is_none() {
            return Ok(());
        }
        let ptr = self.ptr_type();
        let f64_param = AbiParam::new(types::F64);
        let mut sig = Signature::new(CallConv::SystemV);
        sig.params
            .extend([AbiParam::new(ptr), f64_param, f64_param, f64_param]);
        sig.returns.push(f64_param);

        let id = self
            .module
            .inner_mut()
            .declare_function(END_ISLANDS_SYMBOL, Linkage::Import, &sig)
            .map_err(JitError::from)?;
        self.end_islands_id = Some(id);
        Ok(())
    }

    fn ptr_type(&self) -> types::Type {
        self.module.inner().target_config().pointer_type()
    }

    fn declare_named(&mut self) -> Result<(), CompileError> {
        let ptr = self.ptr_type();
        for name in &self.analysis.topo_order {
            if !self.program.registry.contains_key(name) {
                continue;
            }
            let sig = Shape::Full.signature(ptr);
            let id = self
                .module
                .inner_mut()
                .declare_function(&format!("compute_{name}"), Linkage::Local, &sig)
                .map_err(JitError::from)?;
            self.named_ids.insert(name.clone(), id);
        }
        Ok(())
    }

    fn define(
        &mut self,
        id: FuncId,
        shape: Shape,
        df: &DensityFunction,
        is_flat: bool,
        channels: Option<&[DensityFunction]>,
        interp_start: usize,
    ) -> Result<(), CompileError> {
        let ptr = self.ptr_type();
        let sig = shape.signature(ptr);

        let mut func = Function::with_name_signature(UserFuncName::default(), sig);
        let mut builder_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut func, &mut builder_ctx);

        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);

        let params: Vec<_> = builder.block_params(entry).to_vec();
        let flags = MemFlagsData::trusted().with_readonly();

        let gradient_id = self.module.gradient();
        let gradient_gv = self
            .module
            .inner_mut()
            .declare_data_in_func(gradient_id, builder.func);
        let gradient = builder.ins().symbol_value(ptr, gradient_gv);

        let mut intern = |table: &[u8; 256]| {
            let data_id = self.module.permutation_table(table)?;
            let gv = self
                .module
                .inner_mut()
                .declare_data_in_func(data_id, builder.func);
            Ok::<_, JitError>(builder.ins().symbol_value(ptr, gv))
        };
        let plans = plan_noises(self.noises, &mut intern)?;
        let blended_plan = plan_blended(self.blended, &mut intern)?;

        let mut named_refs = FxHashMap::default();
        for (name, func_id) in &self.named_ids {
            let func_ref = self
                .module
                .inner_mut()
                .declare_func_in_func(*func_id, builder.func);
            named_refs.insert(name.clone(), func_ref);
        }

        let fill_ref = match (shape, self.fill_id) {
            (Shape::Fill4x, Some(id)) => Some(
                self.module
                    .inner_mut()
                    .declare_func_in_func(id, builder.func),
            ),
            _ => None,
        };

        let (cache, pos, fill, interpolated, out) = match shape {
            Shape::Full => (
                params[0],
                Position {
                    x: params[1],
                    y: params[2],
                    z: params[3],
                },
                None,
                None,
                None,
            ),
            Shape::Fill => (
                params[0],
                Position {
                    x: params[1],
                    y: params[2],
                    z: params[3],
                },
                Some(params[4]),
                None,
                Some(params[5]),
            ),
            Shape::Fill4x => (
                params[0],
                Position {
                    x: params[1],
                    y: params[1],
                    z: params[3],
                },
                None,
                None,
                Some(params[5]),
            ),
            Shape::Combine => (
                params[0],
                Position {
                    x: params[2],
                    y: params[3],
                    z: params[4],
                },
                None,
                Some(params[1]),
                None,
            ),
        };

        let end_islands = match (self.end_islands, self.end_islands_id) {
            (Some(instance), Some(id)) => {
                let func = self
                    .module
                    .inner_mut()
                    .declare_func_in_func(id, builder.func);
                let instance = builder.ins().iconst(ptr, instance as i64);
                Some(EndIslandsCall { func, instance })
            }
            _ => None,
        };

        let mut ctx = EmitCtx {
            flags,
            gradient,
            noises: &plans,
            blended: blended_plan.as_ref(),
            program: self.program,
            analysis: self.analysis,
            layout: self.layout,
            inlining: &self.inlining,
            named_refs: &named_refs,
            cache,
            pos,
            is_flat,
            fill,
            interpolated,
            end_islands,
            interp_index: interp_start,
            ref_memo: FxHashMap::default(),
        };

        let store_flags = MemFlagsData::trusted();

        if let (Shape::Fill, Some(channels), Some(out)) = (shape, channels, out) {
            for (index, channel) in channels.iter().enumerate() {
                let value = emit_expr(&mut builder, &mut ctx, unwrap_markers(channel))?;
                let offset = i32::try_from(index * size_of::<f64>())
                    .map_err(|_| CompileError::TooManyChannels)?;
                builder.ins().store(store_flags, value, out, offset);
            }
            builder.ins().return_(&[]);
        } else if let (Shape::Fill4x, Some(channels), Some(out), Some(fill_ref)) =
            (shape, channels, out, fill_ref)
        {
            let ys_ptr = params[2];
            let blended_ptr = params[4];
            let count = channels.len();

            for lane in 0..LANES {
                let lane_offset = i32::try_from(lane * size_of::<f64>())
                    .map_err(|_| CompileError::TooManyChannels)?;
                let y = builder.ins().load(types::F64, flags, ys_ptr, lane_offset);
                let blended = builder
                    .ins()
                    .load(types::F64, flags, blended_ptr, lane_offset);
                let out_offset = i64::try_from(lane * count * size_of::<f64>())
                    .map_err(|_| CompileError::TooManyChannels)?;
                let lane_out = builder.ins().iadd_imm(out, out_offset);
                builder.ins().call(
                    fill_ref,
                    &[ctx.cache, ctx.pos.x, y, ctx.pos.z, blended, lane_out],
                );
            }
            builder.ins().return_(&[]);
        } else {
            let result = emit_expr(&mut builder, &mut ctx, df)?;
            builder.ins().return_(&[result]);
        }

        builder.finalize(self.module.inner().target_config());

        let mut context = Context::for_function(func);
        self.module
            .inner_mut()
            .define_function(id, &mut context)
            .map_err(JitError::from)?;
        Ok(())
    }
}

pub(crate) struct FlatStep {
    pub(crate) slot: usize,
    pub(crate) func: DensityFn,
}

#[derive(Clone, Copy)]
pub(crate) enum RouterEntry {
    Cached(usize),
    Computed(DensityFn),
}

#[derive(Clone, Copy, Default)]
pub(crate) struct WellKnownRouters {
    pub(crate) temperature: Option<RouterEntry>,
    pub(crate) vegetation: Option<RouterEntry>,
    pub(crate) continentalness: Option<RouterEntry>,
    pub(crate) erosion: Option<RouterEntry>,
    pub(crate) depth: Option<RouterEntry>,
    pub(crate) ridges: Option<RouterEntry>,
    pub(crate) barrier: Option<RouterEntry>,
    pub(crate) fluid_level_floodedness: Option<RouterEntry>,
    pub(crate) fluid_level_spread: Option<RouterEntry>,
    pub(crate) lava: Option<RouterEntry>,
    pub(crate) preliminary_surface_level: Option<RouterEntry>,
}

impl WellKnownRouters {
    fn from_map(routers: &FxHashMap<String, RouterEntry>) -> Self {
        let get = |name: &str| routers.get(name).copied();
        Self {
            temperature: get("temperature"),
            vegetation: get("vegetation"),
            continentalness: get("continentalness"),
            erosion: get("erosion"),
            depth: get("depth"),
            ridges: get("ridges"),
            barrier: get("barrier"),
            fluid_level_floodedness: get("fluid_level_floodedness"),
            fluid_level_spread: get("fluid_level_spread"),
            lava: get("lava"),
            preliminary_surface_level: get("preliminary_surface_level"),
        }
    }
}

struct CodeMapping(JitModule);

// SAFETY: `CodeMapping` is only ever constructed after `finalize_definitions`,
// and from that point nothing calls into the wrapped module except
// `free_memory` on drop, which takes ownership. The compiled code itself is
// immutable and read-only, so sharing the mapping across threads is sound
// even though sharing a *compiler* would not be.
unsafe impl Send for CodeMapping {}
// SAFETY: as above — the mapping is immutable once finalized, so shared
// references hand out nothing that can be mutated.
unsafe impl Sync for CodeMapping {}

pub struct CompiledDimension {
    module: Option<CodeMapping>,
    flat_steps: Vec<FlatStep>,
    routers: FxHashMap<String, RouterEntry>,
    well_known_routers: WellKnownRouters,
    fill: FillFn,
    fill_4x: Fill4xFn,
    combine_final: CombineFn,
    combine_vein_toggle: CombineFn,
    combine_vein_ridged: CombineFn,
    interpolated_count: usize,
    vein_interp_enabled: bool,
    slots: usize,
    grid_side: usize,
}

impl CompiledDimension {
    pub(crate) fn flat_steps(&self) -> &[FlatStep] {
        &self.flat_steps
    }

    pub(crate) fn router(&self, name: &str) -> Option<&RouterEntry> {
        self.routers.get(name)
    }

    pub(crate) const fn routers(&self) -> &WellKnownRouters {
        &self.well_known_routers
    }

    pub(crate) const fn fill(&self) -> FillFn {
        self.fill
    }

    pub(crate) const fn fill_4x(&self) -> Fill4xFn {
        self.fill_4x
    }

    pub(crate) const fn combine_final(&self) -> CombineFn {
        self.combine_final
    }

    pub(crate) const fn combine_vein_toggle(&self) -> CombineFn {
        self.combine_vein_toggle
    }

    pub(crate) const fn combine_vein_ridged(&self) -> CombineFn {
        self.combine_vein_ridged
    }

    pub const fn interpolated_count(&self) -> usize {
        self.interpolated_count
    }

    pub const fn vein_interp_enabled(&self) -> bool {
        self.vein_interp_enabled
    }

    pub const fn slots(&self) -> usize {
        self.slots
    }

    pub const fn grid_side(&self) -> usize {
        self.grid_side
    }

    pub(crate) const fn grid_len_for(&self, slots: usize) -> usize {
        self.grid_side * self.grid_side * slots
    }
}

impl Drop for CompiledDimension {
    fn drop(&mut self) {
        if let Some(mapping) = self.module.take() {
            // SAFETY: every function pointer handed out lives in this module,
            // none is reachable after `self` is dropped, and `drop` taking
            // `&mut self` means none can be executing.
            unsafe { mapping.0.free_memory() };
        }
    }
}

impl DimensionCompiler<'_> {
    pub(crate) fn compile(mut self) -> Result<CompiledDimension, CompileError> {
        let ptr = self.ptr_type();
        self.declare_end_islands()?;
        self.declare_named()?;

        for name in &self.analysis.topo_order.clone() {
            let Some(df) = self.program.registry.get(name).cloned() else {
                continue;
            };
            let id = self.named_ids[name];
            let is_flat = self.analysis.flat_cached.contains(name);
            let inner = unwrap_markers(&df).clone();
            self.define(id, Shape::Full, &inner, is_flat, None, 0)?;
        }

        let mut router_fn_ids: FxHashMap<String, FuncId> = FxHashMap::default();
        let mut flat_router_ids: FxHashMap<String, FuncId> = FxHashMap::default();

        for (name, df) in &self.program.router_entries.clone() {
            let inner = unwrap_markers(df).clone();
            let sig = Shape::Full.signature(ptr);

            if self.analysis.flat_routers.contains(name) {
                let id = self
                    .module
                    .inner_mut()
                    .declare_function(&format!("compute_router_{name}"), Linkage::Local, &sig)
                    .map_err(JitError::from)?;
                self.define(id, Shape::Full, &inner, true, None, 0)?;
                flat_router_ids.insert(name.clone(), id);
            } else {
                let id = self
                    .module
                    .inner_mut()
                    .declare_function(&format!("router_{name}"), Linkage::Local, &sig)
                    .map_err(JitError::from)?;
                let is_flat = is_flat_cached(df);
                self.define(id, Shape::Full, &inner, is_flat, None, 0)?;
                router_fn_ids.insert(name.clone(), id);
            }
        }

        let mut inline_noise_ids: FxHashMap<u64, FuncId> = FxHashMap::default();
        for (fingerprint, (index, noise_id, xz_scale)) in &self.analysis.inline_flat_noises.clone()
        {
            let df = DensityFunction::Noise(Noise {
                noise_id: noise_id.clone(),
                xz_scale: *xz_scale,
                y_scale: 0.0,
            });
            let sig = Shape::Full.signature(ptr);
            let id = self
                .module
                .inner_mut()
                .declare_function(&format!("inline_noise_{index}"), Linkage::Local, &sig)
                .map_err(JitError::from)?;
            self.define(id, Shape::Full, &df, true, None, 0)?;
            inline_noise_ids.insert(*fingerprint, id);
        }

        let mut channels: Vec<DensityFunction> = Vec::new();
        let mut entry_starts: FxHashMap<&str, usize> = FxHashMap::default();
        for name in INTERPOLATED_ENTRIES {
            let Some(df) = self.program.router_entries.get(name) else {
                continue;
            };
            let inners = collect_interpolated_inners(df, &self.program.registry);
            if inners.is_empty() {
                continue;
            }
            entry_starts.insert(name, channels.len());
            channels.extend(inners);
        }
        let interpolated_count = channels.len();
        let vein_interp_enabled =
            entry_starts.contains_key("vein_toggle") || entry_starts.contains_key("vein_ridged");

        let fill_sig = Shape::Fill.signature(ptr);
        let fill_id = self
            .module
            .inner_mut()
            .declare_function("fill_cell_corner_densities", Linkage::Local, &fill_sig)
            .map_err(JitError::from)?;
        self.fill_id = Some(fill_id);
        let zero_df = DensityFunction::constant(0.0);
        self.define(fill_id, Shape::Fill, &zero_df, false, Some(&channels), 0)?;

        let fill_4x_sig = Shape::Fill4x.signature(ptr);
        let fill_4x_id = self
            .module
            .inner_mut()
            .declare_function(
                "fill_cell_corner_densities_4x",
                Linkage::Local,
                &fill_4x_sig,
            )
            .map_err(JitError::from)?;
        self.define(
            fill_4x_id,
            Shape::Fill4x,
            &zero_df,
            false,
            Some(&channels),
            0,
        )?;

        let mut combine_ids = FxHashMap::default();
        for name in INTERPOLATED_ENTRIES {
            let sig = Shape::Combine.signature(ptr);
            let id = self
                .module
                .inner_mut()
                .declare_function(&format!("combine_{name}"), Linkage::Local, &sig)
                .map_err(JitError::from)?;

            let body = self
                .program
                .router_entries
                .get(name)
                .cloned()
                .unwrap_or_else(|| DensityFunction::constant(0.0));
            let start = entry_starts.get(name).copied().unwrap_or(0);
            self.define(id, Shape::Combine, &body, false, None, start)?;
            combine_ids.insert(name, id);
        }

        self.module
            .inner_mut()
            .finalize_definitions()
            .map_err(JitError::from)?;

        let flat_order = self.layout.flat_order(self.analysis);
        let mut flat_steps = Vec::with_capacity(flat_order.len());
        for step in flat_order {
            let (slot, id) = match step {
                FlatSlot::Named(name) => (
                    self.layout
                        .named_offset(&name)
                        .map(|o| o as usize / size_of::<f64>())
                        .ok_or(CompileError::MissingSlot)?,
                    self.named_ids[&name],
                ),
                FlatSlot::Router(name) => (
                    self.layout
                        .router_slot(&name)
                        .ok_or(CompileError::MissingSlot)?,
                    flat_router_ids[&name],
                ),
                FlatSlot::InlineNoise(fingerprint) => (
                    self.layout
                        .inline_noise_offset(fingerprint)
                        .map(|o| o as usize / size_of::<f64>())
                        .ok_or(CompileError::MissingSlot)?,
                    inline_noise_ids[&fingerprint],
                ),
            };
            let code = self.module.inner().get_finalized_function(id);
            // SAFETY: `id` was declared with `Shape::Full`'s signature, which is
            // exactly `DensityFn`, and finalized above.
            let func: DensityFn = unsafe { transmute::<*const u8, DensityFn>(code) };
            flat_steps.push(FlatStep { slot, func });
        }

        let mut routers = FxHashMap::default();
        for name in self.program.router_entries.keys() {
            let entry = if let Some(slot) = self.layout.router_slot(name) {
                RouterEntry::Cached(slot)
            } else {
                let id = router_fn_ids[name];
                let code = self.module.inner().get_finalized_function(id);
                // SAFETY: declared with `Shape::Full`'s signature and finalized.
                RouterEntry::Computed(unsafe { transmute::<*const u8, DensityFn>(code) })
            };
            routers.insert(name.clone(), entry);
        }

        let fill_code = self.module.inner().get_finalized_function(fill_id);
        // SAFETY: declared with `Shape::Fill`'s signature and finalized.
        let fill: FillFn = unsafe { transmute::<*const u8, FillFn>(fill_code) };

        let fill_4x_code = self.module.inner().get_finalized_function(fill_4x_id);
        // SAFETY: declared with `Shape::Fill4x`'s signature and finalized.
        let fill_4x: Fill4xFn = unsafe { transmute::<*const u8, Fill4xFn>(fill_4x_code) };

        let combine = |name: &str| -> CombineFn {
            let code = self
                .module
                .inner()
                .get_finalized_function(combine_ids[name]);
            // SAFETY: declared with `Shape::Combine`'s signature and finalized.
            unsafe { transmute::<*const u8, CombineFn>(code) }
        };
        let combine_final = combine("final_density");
        let combine_vein_toggle = combine("vein_toggle");
        let combine_vein_ridged = combine("vein_ridged");

        Ok(CompiledDimension {
            module: Some(CodeMapping(self.module)),
            flat_steps,
            well_known_routers: WellKnownRouters::from_map(&routers),
            routers,
            fill,
            fill_4x,
            combine_final,
            combine_vein_toggle,
            combine_vein_ridged,
            interpolated_count,
            vein_interp_enabled,
            slots: self.layout.slots(),
            grid_side: self.layout.grid_side(),
        })
    }
}
