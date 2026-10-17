use std::sync::{LazyLock, Mutex};

use cranelift_codegen::Context;
use cranelift_codegen::ir::{AbiParam, Signature, types};
use cranelift_codegen::isa::CallConv;
use cranelift_codegen::settings::{self, Configurable};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::Module;
use std::env;

pub struct Jit {
    pub module: JITModule,
    pub context: Context,
    pub call_conv: CallConv,
}

static JIT: LazyLock<Option<Mutex<Jit>>> = LazyLock::new(|| {
    if env::var("WORLDGEN_JIT").is_ok_and(|value| value == "0") {
        return None;
    }
    let mut flags = settings::builder();
    flags.set("opt_level", "speed").ok()?;
    flags.set("is_pic", "false").ok()?;
    flags.set("use_colocated_libcalls", "false").ok()?;
    let isa = cranelift_native::builder()
        .ok()?
        .finish(settings::Flags::new(flags))
        .ok()?;
    let call_conv = isa.default_call_conv();
    let module = JITModule::new(JITBuilder::with_isa(
        isa,
        cranelift_module::default_libcall_names(),
    ));
    let context = module.make_context();
    Some(Mutex::new(Jit {
        module,
        context,
        call_conv,
    }))
});

pub fn with_jit<R>(f: impl FnOnce(&mut Jit) -> Option<R>) -> Option<R> {
    let jit = JIT.as_ref()?;
    let mut guard = jit.lock().ok()?;
    f(&mut guard)
}

impl Jit {
    pub fn pointer_type(&self) -> types::Type {
        self.module.target_config().pointer_type()
    }

    pub fn signature(&self, params: &[types::Type], returns: &[types::Type]) -> Signature {
        let mut signature = Signature::new(self.call_conv);
        signature
            .params
            .extend(params.iter().map(|ty| AbiParam::new(*ty)));
        signature
            .returns
            .extend(returns.iter().map(|ty| AbiParam::new(*ty)));
        signature
    }

    pub fn finish(&mut self) -> Option<*const u8> {
        let id = self
            .module
            .declare_anonymous_function(&self.context.func.signature)
            .ok()?;
        let defined = self.module.define_function(id, &mut self.context);
        self.module.clear_context(&mut self.context);
        defined.ok()?;
        self.module.finalize_definitions().ok()?;
        Some(self.module.get_finalized_function(id))
    }
}
