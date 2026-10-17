use crate::util::interval::Interval;
use crate::world::worldgen::densityfunction::volume::DensityVolume;

use super::perlin_noise::PerlinNoise;
use super::simplex_noise::SimplexNoise;
use super::smeared_perlin_noise::SmearedPerlinNoise;

#[derive(Debug, Clone)]
pub enum Noise {
    Perlin(PerlinNoise),
    SmearedPerlin(SmearedPerlinNoise),
    Simplex(SimplexNoise),
    Stack(NoiseStack),
}

impl Noise {
    pub fn range(&self) -> Interval {
        match self {
            Self::Perlin(_) => PerlinNoise::range(),
            Self::SmearedPerlin(n) => n.range(),
            Self::Simplex(_) => SimplexNoise::range(),
            Self::Stack(n) => n.range,
        }
    }

    pub fn get2(&self, x: f64, y: f64) -> f32 {
        match self {
            Self::Perlin(n) => n.get2(x, y),
            Self::SmearedPerlin(n) => n.get2(x, y),
            Self::Simplex(n) => n.get2(x, y),
            Self::Stack(n) => n.get2(x, y),
        }
    }

    pub fn get(&self, x: f64, y: f64, z: f64) -> f32 {
        match self {
            Self::Perlin(n) => n.get(x, y, z),
            Self::SmearedPerlin(n) => n.get(x, y, z),
            Self::Simplex(n) => n.get(x, y, z),
            Self::Stack(n) => n.get(x, y, z),
        }
    }

    pub fn add_to_volume(
        &self,
        buffer: &mut [f32],
        volume: &DensityVolume,
        xz_scale: f64,
        y_scale: f64,
        amplitude: f32,
    ) {
        match self {
            Self::Perlin(n) => n.add_to_volume(buffer, volume, xz_scale, y_scale, amplitude),
            Self::SmearedPerlin(n) => {
                n.add_to_volume(buffer, volume, xz_scale, y_scale, amplitude);
            }
            Self::Stack(n) => n.add_to_volume(buffer, volume, xz_scale, y_scale, amplitude),
            Self::Simplex(_) => {
                let mut index = 0;
                for index_z in 0..volume.size_z {
                    let z = f64::from(volume.block_z(index_z)) * xz_scale;
                    for index_x in 0..volume.size_x {
                        let x = f64::from(volume.block_x(index_x)) * xz_scale;
                        for index_y in 0..volume.size_y {
                            let y = f64::from(volume.block_y(index_y)) * y_scale;
                            buffer[index] += amplitude * self.get(x, y, z);
                            index += 1;
                        }
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Layer {
    pub noise: Noise,
    pub frequency: f64,
    pub amplitude: f32,
}

#[derive(Debug, Clone)]
pub struct NoiseStack {
    pub layers: Vec<Layer>,
    pub range: Interval,
}

impl NoiseStack {
    pub const fn builder() -> NoiseStackBuilder {
        NoiseStackBuilder { layers: Vec::new() }
    }

    fn new(layers: Vec<Layer>) -> Self {
        let mut range = Interval::of_exact(0.0);
        for layer in &layers {
            let layer_range =
                Interval::mul(layer.noise.range(), Interval::of_exact(layer.amplitude));
            range = Interval::add(range, layer_range);
        }
        Self { layers, range }
    }

    pub fn get(&self, x: f64, y: f64, z: f64) -> f32 {
        let mut value = 0.0f32;
        for layer in &self.layers {
            let frequency = layer.frequency;
            value += layer.amplitude * layer.noise.get(x * frequency, y * frequency, z * frequency);
        }
        value
    }

    pub fn get2(&self, x: f64, y: f64) -> f32 {
        let mut value = 0.0f32;
        for layer in &self.layers {
            let frequency = layer.frequency;
            value += layer.amplitude * layer.noise.get2(x * frequency, y * frequency);
        }
        value
    }

    pub fn add_to_volume(
        &self,
        buffer: &mut [f32],
        volume: &DensityVolume,
        xz_scale: f64,
        y_scale: f64,
        amplitude: f32,
    ) {
        for layer in &self.layers {
            let frequency = layer.frequency;
            layer.noise.add_to_volume(
                buffer,
                volume,
                xz_scale * frequency,
                y_scale * frequency,
                amplitude * layer.amplitude,
            );
        }
    }
}

pub struct NoiseStackBuilder {
    layers: Vec<Layer>,
}

impl NoiseStackBuilder {
    #[must_use]
    pub fn add(mut self, noise: Noise, frequency: f64, amplitude: f32) -> Self {
        self.layers.push(Layer {
            noise,
            frequency,
            amplitude,
        });
        self
    }

    #[must_use]
    pub fn add_stack(mut self, stack: NoiseStack, frequency: f64, amplitude: f32) -> Self {
        for layer in stack.layers {
            self.layers.push(Layer {
                noise: layer.noise,
                frequency: layer.frequency * frequency,
                amplitude: layer.amplitude * amplitude,
            });
        }
        self
    }

    pub fn build(self) -> NoiseStack {
        NoiseStack::new(self.layers)
    }
}
