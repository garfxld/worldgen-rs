use std::collections::BTreeMap;
use std::ptr;
use std::time::Instant;

use rustc_hash::FxHashMap;

use crate::util::noise::{BlendedNoise, EndIslands, NormalNoise};
use crate::util::rng::legacy_random::LegacyRandomSource;
use crate::util::rng::name_hash::NameHash;
use crate::util::rng::xoroshiro::XoroshiroRandomSource;
use crate::util::rng::{RandomFactory, RandomSource};
use crate::worldgen::backend::ir::json::NoiseSettingsJson;
use crate::worldgen::backend::ir::{DensityProgram, NoiseParameters};
use crate::worldgen::backend::jit::{
    CacheLayout, CompileError, CompiledDimension, DimensionCompiler, JitModule,
};

const NETHER_TEMPERATURE_ID: &str = "minecraft:nether/temperature";
const NETHER_VEGETATION_ID: &str = "minecraft:nether/vegetation";
const AQUIFER_HASH_ID: &str = "minecraft:aquifer";
const TERRAIN_HASH_ID: &str = "minecraft:terrain";

pub struct Noises {
    compiled: CompiledDimension,
    noises: FxHashMap<String, NormalNoise>,
    random: RandomFactory,
    aquifer_random: RandomFactory,
    blended: Option<BlendedNoise>,
    end_islands: Option<Box<EndIslands>>,
    cell_width: i32,
    cell_height: i32,
    min_y: i32,
    height: i32,
    sea_level: i32,
}

impl Noises {
    pub fn new(
        program: &DensityProgram,
        settings: &NoiseSettingsJson,
        params: &BTreeMap<String, NoiseParameters>,
        seed: u64,
    ) -> Result<Self, CompileError> {
        let total = Instant::now();
        let t = Instant::now();
        let analysis = program.analyze();
        let layout = CacheLayout::new(program, &analysis);
        let legacy = analysis.legacy_random_source;
        tracing::debug!(
            "density function analysis + layout planning done in {:?}",
            t.elapsed()
        );

        let t = Instant::now();
        let mut root = if legacy {
            RandomSource::Legacy(LegacyRandomSource::from_seed(seed))
        } else {
            RandomSource::Xoroshiro(XoroshiroRandomSource::from_seed(seed))
        };
        let random = root.next_positional();
        let aquifer_random = random
            .with_hash_of(&NameHash::new(AQUIFER_HASH_ID))
            .next_positional();

        let mut noises = FxHashMap::default();
        for id in &analysis.noise_ids {
            let params = params
                .get(id)
                .unwrap_or_else(|| panic!("no noise parameters for {id}"));
            let normal = if id == NETHER_TEMPERATURE_ID {
                let mut src = RandomSource::Legacy(LegacyRandomSource::from_seed(seed));
                NormalNoise::create_legacy_nether_biome(
                    &mut src,
                    params.first_octave,
                    &params.amplitudes,
                )
            } else if id == NETHER_VEGETATION_ID {
                let mut src =
                    RandomSource::Legacy(LegacyRandomSource::from_seed(seed.wrapping_add(1)));
                NormalNoise::create_legacy_nether_biome(
                    &mut src,
                    params.first_octave,
                    &params.amplitudes,
                )
            } else {
                NormalNoise::create(&random, id, params.first_octave, &params.amplitudes)
            };
            noises.insert(id.clone(), normal);
        }
        tracing::debug!(
            "instantiated {} normal noises in {:?}",
            noises.len(),
            t.elapsed()
        );

        let t = Instant::now();
        let blended = analysis.blended_noise_config.as_ref().map(|cfg| {
            let mut terrain_random = if legacy {
                RandomSource::Legacy(LegacyRandomSource::from_seed(seed))
            } else {
                random.with_hash_of(&NameHash::new(TERRAIN_HASH_ID))
            };
            BlendedNoise::new(
                &mut terrain_random,
                cfg.xz_scale,
                cfg.y_scale,
                cfg.xz_factor,
                cfg.y_factor,
                cfg.smear_scale_multiplier,
            )
        });

        let end_islands = analysis
            .uses_end_islands
            .then(|| Box::new(EndIslands::new(seed)));
        let end_islands_ptr = end_islands.as_deref().map(ptr::from_ref);
        tracing::debug!("blended noise + end islands setup done in {:?}", t.elapsed());

        let t = Instant::now();
        let module = JitModule::new()?;
        let dimension_compiler = DimensionCompiler::new(
            module,
            program,
            &analysis,
            &layout,
            &noises,
            blended.as_ref(),
            end_islands_ptr,
        );
        let compiled = dimension_compiler.compile()?;
        tracing::debug!(
            "density function JIT compilation done in {:?} ({} slots, {} interpolated)",
            t.elapsed(),
            compiled.slots(),
            compiled.interpolated_count()
        );
        tracing::debug!(
            "density functions fully compiled in {:?}",
            total.elapsed()
        );

        Ok(Self {
            compiled,
            noises,
            random,
            aquifer_random,
            blended,
            end_islands,
            cell_width: program.cell_width,
            cell_height: settings.noise.size_vertical * 4,
            min_y: settings.noise.min_y,
            height: settings.noise.height,
            sea_level: settings.sea_level,
        })
    }

    pub const fn compiled(&self) -> &CompiledDimension {
        &self.compiled
    }

    pub const fn cell_width(&self) -> i32 {
        self.cell_width
    }

    pub fn noise(&self, id: &str) -> Option<&NormalNoise> {
        self.noises.get(id)
    }

    pub const fn random(&self) -> &RandomFactory {
        &self.random
    }

    pub const fn aquifer_random(&self) -> &RandomFactory {
        &self.aquifer_random
    }

    pub fn end_islands(&self) -> Option<&EndIslands> {
        self.end_islands.as_deref()
    }

    pub const fn blended(&self) -> Option<&BlendedNoise> {
        self.blended.as_ref()
    }

    pub const fn cell_height(&self) -> i32 {
        self.cell_height
    }

    pub const fn min_y(&self) -> i32 {
        self.min_y
    }

    pub const fn height(&self) -> i32 {
        self.height
    }

    pub const fn sea_level(&self) -> i32 {
        self.sea_level
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test loads the checked-in vanilla datapack"
)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::worldgen::backend::ir::loader::DatapackSource;

    fn datapack_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/datapack/worldgen")
    }

    #[test]
    fn overworld_noises_compile_from_seed() {
        let source = DatapackSource::new(datapack_root());
        let registry = source.density_function_registry().unwrap();
        let (program, settings) = source.program("overworld", "Overworld", &registry).unwrap();
        let params = source.noise_parameters().unwrap();

        let noises = Noises::new(&program, &settings, &params, 42).unwrap();
        assert!(noises.compiled().slots() > 0);
        assert!(noises.compiled().interpolated_count() > 0);
        assert!(noises.noise("minecraft:temperature").is_some());
        assert!(noises.end_islands().is_none());
    }

    #[test]
    fn end_noises_compile_with_end_islands() {
        let source = DatapackSource::new(datapack_root());
        let registry = source.density_function_registry().unwrap();
        let (program, settings) = source.program("end", "End", &registry).unwrap();
        let params = source.noise_parameters().unwrap();

        let noises = Noises::new(&program, &settings, &params, 42).unwrap();
        assert!(noises.end_islands().is_some());
    }
}
