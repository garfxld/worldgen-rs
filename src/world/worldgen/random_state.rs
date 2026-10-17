use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;
use rustc_hash::FxHashMap;

use crate::util::rng::RandomFactory;
use crate::util::rng::name_hash::NameHash;
use crate::world::worldgen::biome::climate::Sampler;
use crate::world::worldgen::densityfunction::compiler::{CompileContext, DensityFunctionCompiler};
use crate::world::worldgen::densityfunction::context::SamplerContext;
use crate::world::worldgen::densityfunction::function::{Df, NoiseHolder};
use crate::world::worldgen::densityfunction::registry::DensityRegistry;
use crate::world::worldgen::densityfunction::sampler::S;
use crate::world::worldgen::material::MaterialSystem;
use crate::world::worldgen::noise_generator_settings::{NoiseGeneratorSettings, NoiseRouter};
use crate::world::worldgen::synth::Noise;
use crate::world::worldgen::synth::normal_noise::NormalNoise;

pub struct RandomState {
    seed: i64,
    random: RandomFactory,
    noises: BTreeMap<String, Arc<NormalNoise>>,
    router: NoiseRouter,
    material_system: OnceLock<MaterialSystem>,
    positional_randoms: Mutex<FxHashMap<String, RandomFactory>>,
    density_function_compiler: Mutex<DensityFunctionCompiler>,
}

impl RandomState {
    pub fn create(
        registry: &DensityRegistry,
        seed: i64,
        settings: &NoiseGeneratorSettings,
    ) -> Self {
        let context = CompileContext::new(seed, settings.use_legacy_random_source);
        let random_state = Self {
            seed,
            random: context.random().clone(),
            noises: registry.noises.clone(),
            router: settings.noise_router.clone(),
            material_system: OnceLock::new(),
            positional_randoms: Mutex::new(FxHashMap::default()),
            density_function_compiler: Mutex::new(DensityFunctionCompiler::new(context)),
        };
        let material_system = MaterialSystem::new(
            &random_state,
            settings.default_block,
            settings.sea_level,
            settings.noise_router.chunk_surface_level.clone(),
            random_state.random.clone(),
        );
        let _ = random_state.material_system.set(material_system);
        random_state
    }

    pub fn surface_system(&self) -> &MaterialSystem {
        self.material_system
            .get()
            .expect("material system is set in RandomState::create")
    }

    pub fn create_climate_sampler(&self, context: SamplerContext<'static>) -> Sampler {
        Sampler {
            temperature: self.get_sampler(&self.router.temperature),
            humidity: self.get_sampler(&self.router.vegetation),
            continentalness: self.get_sampler(&self.router.continents),
            erosion: self.get_sampler(&self.router.erosion),
            depth: self.get_sampler(&self.router.depth),
            weirdness: self.get_sampler(&self.router.ridges),
            context: Rc::new(RefCell::new(context)),
        }
    }

    pub const fn router(&self) -> &NoiseRouter {
        &self.router
    }

    pub fn get_or_create_noise(&self, key: &str) -> Arc<Noise> {
        let noise = self
            .noises
            .get(key)
            .unwrap_or_else(|| panic!("unknown noise {key}"))
            .clone();
        self.density_function_compiler
            .lock()
            .context
            .get_or_create_noise(&NoiseHolder {
                key: key.to_owned(),
                noise,
            })
    }

    pub fn get_or_create_random_factory(&self, name: &str) -> RandomFactory {
        self.positional_randoms
            .lock()
            .entry(name.to_owned())
            .or_insert_with(|| {
                self.random
                    .with_hash_of(&NameHash::new(name))
                    .next_positional()
            })
            .clone()
    }

    pub fn get_sampler(&self, function: &Df) -> S {
        self.density_function_compiler.lock().get_sampler(function)
    }

    pub const fn random(&self) -> &RandomFactory {
        &self.random
    }

    pub const fn seed(&self) -> i64 {
        self.seed
    }
}
