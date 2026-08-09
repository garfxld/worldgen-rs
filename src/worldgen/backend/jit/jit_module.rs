use cranelift_codegen::settings::{self, Configurable};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{DataDescription, DataId, Linkage, Module, ModuleError};
use rustc_hash::FxHashMap;

use crate::util::math::GRADIENT_4;
use crate::util::noise::EndIslands;

#[derive(Debug, thiserror::Error)]
pub enum JitError {
    #[error("unsupported host for JIT codegen: {0}")]
    UnsupportedHost(String),
    #[error("invalid Cranelift setting: {0}")]
    Settings(#[from] settings::SetError),
    #[error("Cranelift module error: {0}")]
    Module(#[from] Box<ModuleError>),
    #[error("failed to compile {name}: {source}")]
    Compile {
        name: String,
        source: Box<cranelift_codegen::CodegenError>,
    },
}

impl From<ModuleError> for JitError {
    fn from(value: ModuleError) -> Self {
        Self::Module(Box::new(value))
    }
}

pub(crate) const END_ISLANDS_SYMBOL: &str = "jit_end_islands_sample";

unsafe extern "C" fn end_islands_sample(
    instance: *const EndIslands,
    x: f64,
    y: f64,
    z: f64,
) -> f64 {
    // SAFETY: `instance` points at a live `EndIslands` owned by the compiled
    // dimension, which outlives every call into its code.
    unsafe { (*instance).sample(x, y, z) }
}

const GRADIENT_STRIDE_SHIFT: i64 = 5;

pub struct JitModule {
    module: JITModule,
    gradient: DataId,
    permutations: FxHashMap<[u8; 256], DataId>,
    next_data_id: usize,
}

impl JitModule {
    pub fn new() -> Result<Self, JitError> {
        let mut flags = settings::builder();
        flags.set("use_colocated_libcalls", "false")?;
        flags.set("is_pic", "false")?;
        flags.set("opt_level", "speed")?;

        let isa_builder =
            cranelift_native::builder().map_err(|e| JitError::UnsupportedHost(e.to_string()))?;
        let isa = isa_builder
            .finish(settings::Flags::new(flags))
            .map_err(|e| JitError::UnsupportedHost(e.to_string()))?;

        let mut builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
        builder.symbol(END_ISLANDS_SYMBOL, end_islands_sample as *const u8);
        let mut module = JITModule::new(builder);

        let gradient = Self::define_gradient(&mut module)?;

        Ok(Self {
            module,
            gradient,
            permutations: FxHashMap::default(),
            next_data_id: 0,
        })
    }

    fn define_gradient(module: &mut JITModule) -> Result<DataId, JitError> {
        let mut bytes = Vec::with_capacity(GRADIENT_4.len() * 4 * 8);
        for entry in &GRADIENT_4 {
            for component in entry {
                bytes.extend_from_slice(&component.to_le_bytes());
            }
        }

        let id = module.declare_data("jit_gradient", Linkage::Local, false, false)?;
        let mut desc = DataDescription::new();
        desc.define(bytes.into_boxed_slice());
        module.define_data(id, &desc)?;
        Ok(id)
    }

    pub(crate) fn permutation_table(&mut self, table: &[u8; 256]) -> Result<DataId, JitError> {
        if let Some(id) = self.permutations.get(table) {
            return Ok(*id);
        }

        let name = format!("jit_perm_{}", self.next_data_id);
        self.next_data_id += 1;

        let id = self
            .module
            .declare_data(&name, Linkage::Local, false, false)?;
        let mut desc = DataDescription::new();
        desc.define(Box::new(*table) as Box<[u8]>);
        self.module.define_data(id, &desc)?;

        self.permutations.insert(*table, id);
        Ok(id)
    }

    pub(crate) const fn gradient(&self) -> DataId {
        self.gradient
    }

    pub(crate) const fn gradient_stride_shift() -> i64 {
        GRADIENT_STRIDE_SHIFT
    }

    pub(crate) const fn inner(&self) -> &JITModule {
        &self.module
    }

    pub(crate) const fn inner_mut(&mut self) -> &mut JITModule {
        &mut self.module
    }

    /// # Safety
    ///
    /// No compiled function from this module may be called afterwards, and none
    /// may be executing.
    pub(crate) unsafe fn free_memory(self) {
        // SAFETY: forwarded to the caller's contract above.
        unsafe { self.module.free_memory() };
    }
}
