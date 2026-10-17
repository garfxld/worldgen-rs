mod emit;
mod module;
mod runtime;
mod value;
mod volume;

use std::sync::OnceLock;

use crate::world::worldgen::densityfunction::sampler::SamplerNode;

pub use volume::VolumeKernel;

pub type ValueFn = unsafe extern "C" fn(*mut u8, i32, i32, i32) -> f32;

#[derive(Default)]
pub struct Slots {
    value: OnceLock<Option<ValueFn>>,
    volume: OnceLock<Option<VolumeKernel>>,
}

pub fn value_fn(node: &SamplerNode) -> Option<ValueFn> {
    *node
        .jit
        .value
        .get_or_init(|| module::with_jit(|jit| value::compile(jit, node)))
}

pub fn volume_kernel(node: &SamplerNode) -> Option<&VolumeKernel> {
    node.jit
        .volume
        .get_or_init(|| module::with_jit(|jit| volume::compile(jit, node)))
        .as_ref()
}
