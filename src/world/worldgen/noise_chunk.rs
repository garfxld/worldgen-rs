use crate::world::worldgen::aquifer::{Aquifer, FluidPicker};
use crate::world::worldgen::beardifier::Beardifier;
use crate::world::worldgen::densityfunction::context::SamplerContext;
use crate::world::worldgen::densityfunction::volume::DensityVolume;
use crate::world::worldgen::noise_generator_settings::NoiseGeneratorSettings;
use crate::world::worldgen::random_state::RandomState;

const AQUIFER_RANDOM: &str = "minecraft:aquifer";

pub struct NoiseChunk<'a> {
    pub volume: DensityVolume,
    pub aquifer: Aquifer,
    pub caching_samplers: SamplerContext<'a>,
}

impl<'a> NoiseChunk<'a> {
    pub fn new(
        random_state: &RandomState,
        beardifier: Option<&'a Beardifier>,
        settings: &NoiseGeneratorSettings,
        global_fluid_picker: FluidPicker,
        volume: DensityVolume,
    ) -> Self {
        let mut caching_samplers = SamplerContext::cached().with_beardifier(beardifier);
        let aquifer = match &settings.aquifers {
            Some(config) => config.create(
                random_state,
                &mut caching_samplers,
                random_state.get_or_create_random_factory(AQUIFER_RANDOM),
                &volume,
                global_fluid_picker,
            ),
            None => Aquifer::create_disabled(global_fluid_picker),
        };
        Self {
            volume,
            aquifer,
            caching_samplers,
        }
    }
}
