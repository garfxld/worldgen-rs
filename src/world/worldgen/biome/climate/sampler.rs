use std::cell::RefCell;
use std::rc::Rc;

use crate::world::worldgen::densityfunction::context::SamplerContext;
use crate::world::worldgen::densityfunction::eval::{sample_value, sample_volume};
use crate::world::worldgen::densityfunction::sampler::S;
use crate::world::worldgen::densityfunction::volume::DensityVolume;

use super::TargetPoint;

#[derive(Clone)]
pub struct Sampler {
    pub temperature: S,
    pub humidity: S,
    pub continentalness: S,
    pub erosion: S,
    pub depth: S,
    pub weirdness: S,
    pub context: Rc<RefCell<SamplerContext<'static>>>,
}

impl Sampler {
    pub fn sample(&self, quart_x: i32, quart_y: i32, quart_z: i32) -> TargetPoint {
        let block_x = quart_x << 2;
        let block_y = quart_y << 2;
        let block_z = quart_z << 2;
        let ctx = &mut *self.context.borrow_mut();
        let temperature = sample_value(&self.temperature, ctx, block_x, block_y, block_z);
        let humidity = sample_value(&self.humidity, ctx, block_x, block_y, block_z);
        let continentalness = sample_value(&self.continentalness, ctx, block_x, block_y, block_z);
        let erosion = sample_value(&self.erosion, ctx, block_x, block_y, block_z);
        let depth = sample_value(&self.depth, ctx, block_x, block_y, block_z);
        let weirdness = sample_value(&self.weirdness, ctx, block_x, block_y, block_z);
        TargetPoint::from_floats(
            f64::from(temperature),
            f64::from(humidity),
            f64::from(continentalness),
            f64::from(erosion),
            f64::from(depth),
            f64::from(weirdness),
        )
    }

    pub fn sample_value(&self, sampler: &S, block_x: i32, block_y: i32, block_z: i32) -> f32 {
        sample_value(
            sampler,
            &mut self.context.borrow_mut(),
            block_x,
            block_y,
            block_z,
        )
    }

    pub fn sample_volume(&self, sampler: &S, volume: &DensityVolume) -> Vec<f32> {
        let mut out = vec![0.0f32; volume.size()];
        sample_volume(sampler, &mut self.context.borrow_mut(), &mut out, volume);
        out
    }
}
