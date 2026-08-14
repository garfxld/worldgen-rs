use std::mem;

use cranelift_codegen::Context;
use cranelift_codegen::ir::{
    AbiParam, Function, InstBuilder, MemFlagsData, Signature, UserFuncName, Value, types,
};
use cranelift_codegen::isa::CallConv;
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_module::{Linkage, Module};
use rustc_hash::FxHashMap;

use crate::util::noise::{BlendedNoise, NormalNoise};
use crate::worldgen::backend::ir::{DensityFunction, DensityProgram};

mod blended;
mod expr;
mod math;
mod noise;
mod spline;

pub use expr::{EmitCtx, EndIslandsCall, Position, Unsupported, emit as emit_expr};

use super::inlining::InlinePolicy;
use super::jit_module::{JitError, JitModule};
use super::layout::CacheLayout;

pub struct CompiledExpr {
    module: Option<JitModule>,
    func: unsafe extern "C" fn(f64, f64, f64) -> f64,
}

#[derive(Debug, thiserror::Error)]
pub enum CompileError {
    #[error(transparent)]
    Unsupported(#[from] Unsupported),
    #[error(transparent)]
    Jit(#[from] JitError),
    #[error("too many interpolated channels")]
    TooManyChannels,
    #[error("column cache slot missing from the layout")]
    MissingSlot,
}

pub(crate) fn plan_noises<E>(
    noises: &FxHashMap<String, NormalNoise>,
    intern: &mut impl FnMut(&[u8; 256]) -> Result<Value, E>,
) -> Result<FxHashMap<String, noise::NormalNoisePlan>, E> {
    noises
        .iter()
        .map(|(id, noise)| Ok((id.clone(), noise::NormalNoisePlan::new(noise, intern)?)))
        .collect()
}

pub(crate) fn plan_blended<E>(
    blended: Option<&BlendedNoise>,
    intern: &mut impl FnMut(&[u8; 256]) -> Result<Value, E>,
) -> Result<Option<blended::BlendedNoisePlan>, E> {
    blended
        .map(|b| blended::BlendedNoisePlan::new(b.snapshot(), intern))
        .transpose()
}

impl CompiledExpr {
    pub fn compile(
        mut module: JitModule,
        df: &DensityFunction,
        program: &DensityProgram,
        noises: &FxHashMap<String, NormalNoise>,
        blended: Option<&BlendedNoise>,
        is_flat: bool,
    ) -> Result<Self, CompileError> {
        let analysis = program.analyze();
        let layout = CacheLayout::new(program, &analysis);
        let inlining = InlinePolicy::new(program, &analysis);
        let named_refs = FxHashMap::default();

        let frontend_config = module.inner().target_config();
        let ptr_ty = frontend_config.pointer_type();

        let mut sig = Signature::new(CallConv::SystemV);
        sig.params
            .extend([AbiParam::new(types::F64); 3].iter().copied());
        sig.returns.push(AbiParam::new(types::F64));

        let func_id = module
            .inner_mut()
            .declare_function("density", Linkage::Export, &sig)
            .map_err(JitError::from)?;

        let mut func = Function::with_name_signature(UserFuncName::default(), sig);
        let mut builder_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut func, &mut builder_ctx);

        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);
        builder.switch_to_block(entry);
        builder.seal_block(entry);

        let params = builder.block_params(entry);
        let pos = expr::Position {
            x: params[0],
            y: params[1],
            z: params[2],
        };

        let flags = MemFlagsData::trusted().with_readonly();

        let gradient_id = module.gradient();
        let gradient_gv = module
            .inner_mut()
            .declare_data_in_func(gradient_id, builder.func);
        let gradient = builder.ins().symbol_value(ptr_ty, gradient_gv);

        let mut intern = |table: &[u8; 256]| {
            let data_id = module.permutation_table(table)?;
            let gv = module
                .inner_mut()
                .declare_data_in_func(data_id, builder.func);
            Ok::<_, JitError>(builder.ins().symbol_value(ptr_ty, gv))
        };

        let plans = plan_noises(noises, &mut intern)?;
        let blended_plan = plan_blended(blended, &mut intern)?;

        let cache = builder.ins().iconst(ptr_ty, 0);

        let mut ctx = expr::EmitCtx {
            flags,
            gradient,
            noises: &plans,
            blended: blended_plan.as_ref(),
            program,
            analysis: &analysis,
            layout: &layout,
            inlining: &inlining,
            named_refs: &named_refs,
            cache,
            pos,
            is_flat,
            fill: None,
            interpolated: None,
            end_islands: None,
            interp_index: 0,
        };

        let result = expr::emit(&mut builder, &mut ctx, df)?;
        builder.ins().return_(&[result]);
        builder.finalize(frontend_config);

        let mut context = Context::for_function(func);
        module
            .inner_mut()
            .define_function(func_id, &mut context)
            .map_err(JitError::from)?;
        module
            .inner_mut()
            .finalize_definitions()
            .map_err(JitError::from)?;

        let code = module.inner().get_finalized_function(func_id);

        // SAFETY: `code` points at the function just compiled and finalized
        // from `sig`, which is `(f64, f64, f64) -> f64` in the SystemV
        // convention — exactly the type transmuted to. The module is moved into
        // the returned value, so the mapping outlives every call through it.
        let func = unsafe {
            mem::transmute::<*const u8, unsafe extern "C" fn(f64, f64, f64) -> f64>(code)
        };

        Ok(Self {
            module: Some(module),
            func,
        })
    }

    pub fn eval(&self, x: f64, y: f64, z: f64) -> f64 {
        // SAFETY: `self.func` was produced by `compile` from a matching
        // signature, and `self.module` keeps its code mapped for as long as
        // `self` is alive.
        unsafe { (self.func)(x, y, z) }
    }
}

impl Drop for CompiledExpr {
    fn drop(&mut self) {
        if let Some(module) = self.module.take() {
            // SAFETY: `self.func` is the only pointer handed out into this
            // module's code, it is not callable after `self` is dropped, and no
            // call can be in progress because `drop` takes `&mut self`.
            unsafe { module.free_memory() };
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::worldgen::backend::ir::TwoArgType;
    use crate::worldgen::backend::ir::types::{Constant, TwoArgumentSimple};

    fn program() -> DensityProgram {
        DensityProgram {
            registry: BTreeMap::new(),
            router_entries: BTreeMap::new(),
            prefix: String::new(),
            cell_width: 4,
            legacy_random_source: false,
        }
    }

    #[test]
    fn compiles_and_runs_arithmetic() {
        let df = DensityFunction::TwoArgumentSimple(TwoArgumentSimple {
            op: TwoArgType::Add,
            argument1: DensityFunction::Constant(Constant { value: 1.5 }).into(),
            argument2: DensityFunction::Constant(Constant { value: 2.5 }).into(),
        });

        let module = JitModule::new().expect("JIT unavailable on this host");
        let program = program();
        let noises = FxHashMap::default();

        let compiled = CompiledExpr::compile(module, &df, &program, &noises, None, true)
            .expect("density function should compile");

        assert_eq!(compiled.eval(0.0, 0.0, 0.0), 4.0);
    }
}
