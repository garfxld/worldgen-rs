use std::collections::HashMap;
use std::sync::Arc;

use shulkr::util::Axis;

use crate::util::rng::RandomFactory;
use crate::util::rng::RandomSource;
use crate::util::rng::legacy_random::LegacyRandomSource;
use crate::util::rng::name_hash::NameHash;
use crate::util::rng::xoroshiro::XoroshiroRandomSource;
use crate::world::worldgen::synth::smeared_perlin_noise::SmearedPerlinNoise;
use crate::world::worldgen::synth::{Noise, NoiseStack, SimplexNoise};

use super::function::{
    ALL_AXES, AXIS_X, AXIS_Y, AXIS_Z, BinaryType, BlendedNoise, DensityFunction, Df, NoiseHolder,
    PreparedCache, SimpleType, TilingMode, UnaryType, axes_from,
};
use super::sampler::{GradientTiling, S, Sampler, SplineNode, node};
use super::spline::CubicSpline;

pub const BLENDED_NOISE_SEED: &str = "minecraft:terrain";
const TEMPERATURE_NETHER: &str = "minecraft:nether/temperature";
const VEGETATION_NETHER: &str = "minecraft:nether/vegetation";

pub struct CompileContext {
    seed: i64,
    legacy_random_source: bool,
    random: RandomFactory,
    instances: HashMap<String, Arc<Noise>>,
}

impl CompileContext {
    pub fn new(seed: i64, legacy_random_source: bool) -> Self {
        let mut root = if legacy_random_source {
            RandomSource::Legacy(LegacyRandomSource::from_seed(seed as u64))
        } else {
            RandomSource::Xoroshiro(XoroshiroRandomSource::from_seed(seed as u64))
        };
        Self {
            seed,
            legacy_random_source,
            random: root.next_positional(),
            instances: HashMap::new(),
        }
    }

    pub const fn seed(&self) -> i64 {
        self.seed
    }

    pub const fn random(&self) -> &RandomFactory {
        &self.random
    }

    const fn legacy_instance(&self, offset: i64) -> RandomSource {
        RandomSource::Legacy(LegacyRandomSource::from_seed(
            self.seed.wrapping_add(offset) as u64,
        ))
    }

    pub fn get_or_create_noise(&mut self, holder: &NoiseHolder) -> Arc<Noise> {
        if let Some(noise) = self.instances.get(&holder.key) {
            return noise.clone();
        }
        let mut random = self.random.with_hash_of(&NameHash::new(&holder.key));
        let noise = Arc::new(holder.noise.create(&mut random));
        self.instances.insert(holder.key.clone(), noise.clone());
        noise
    }

    pub fn create_noise_sampler(&mut self, holder: &NoiseHolder) -> Arc<Noise> {
        if holder.key == TEMPERATURE_NETHER {
            Arc::new(
                holder
                    .noise
                    .create_for_legacy_nether_biome(&mut self.legacy_instance(0)),
            )
        } else if holder.key == VEGETATION_NETHER {
            Arc::new(
                holder
                    .noise
                    .create_for_legacy_nether_biome(&mut self.legacy_instance(1)),
            )
        } else {
            self.get_or_create_noise(holder)
        }
    }

    pub const fn create_random(&self, seed: &str) -> RandomSource {
        if self.legacy_random_source && seed == BLENDED_NOISE_SEED {
            self.legacy_instance(0)
        } else {
            self.random.with_hash_of(&NameHash::new(seed))
        }
    }

    pub const fn create_end_island_random(&self) -> RandomSource {
        self.legacy_instance(0)
    }
}

pub struct DensityFunctionCompiler {
    pub context: CompileContext,
    samplers: HashMap<Df, S>,
    prepared_caches: HashMap<Df, Df>,
    cache_samplers: Vec<S>,
}

#[derive(Clone, Copy)]
enum Rule {
    Optimize,
    SliceUniformAxes(u8),
}

fn rebuild(f: &Df, rule: &mut impl FnMut(&Df) -> Df) -> Df {
    use DensityFunction as D;
    let new = match &**f {
        D::Constant(_)
        | D::EndOuterIslands
        | D::DistanceToPoint { .. }
        | D::Gradient(_)
        | D::OldBlendedNoise(_)
        | D::Simple(_)
        | D::PreparedCache(_) => return f.clone(),
        D::ShiftA(n) => D::ShiftA(n.clone()),
        D::ShiftB(n) => D::ShiftB(n.clone()),
        D::Shift(n) => D::Shift(n.clone()),
        D::Holder(h) => return rule(h.value()),
        D::Noise {
            noise,
            xz_scale,
            y_scale,
            shift_x,
            shift_y,
            shift_z,
        } => D::Noise {
            noise: noise.clone(),
            xz_scale: *xz_scale,
            y_scale: *y_scale,
            shift_x: rule(shift_x),
            shift_y: rule(shift_y),
            shift_z: rule(shift_z),
        },
        D::Unary(t, input) => D::Unary(*t, rule(input)),
        D::Round(t, input, multiple) => D::Round(*t, rule(input), rule(multiple)),
        D::Binary(t, left, right) => {
            let left = rule(left);
            let right = rule(right);
            D::Binary(*t, left, right)
        }
        D::Pow(base, exponent) => {
            let base = rule(base);
            let exponent = rule(exponent);
            D::Pow(base, exponent)
        }
        D::Spline(spline) => D::Spline(spline.map_coordinates(&mut |c| rule(c))),
        D::Lerp(alpha, first, second) => {
            let alpha = rule(alpha);
            let first = rule(first);
            let second = rule(second);
            D::Lerp(alpha, first, second)
        }
        D::Clamp(input, min, max) => D::Clamp(rule(input), *min, *max),
        D::RangeChoice {
            input,
            min_inclusive,
            max_exclusive,
            when_in_range,
            when_out_of_range,
        } => {
            let input = rule(input);
            let when_in_range = rule(when_in_range);
            let when_out_of_range = rule(when_out_of_range);
            D::RangeChoice {
                input,
                min_inclusive: *min_inclusive,
                max_exclusive: *max_exclusive,
                when_in_range,
                when_out_of_range,
            }
        }
        D::IntervalSelect {
            input,
            thresholds,
            functions,
        } => {
            let input = rule(input);
            let functions = functions.iter().map(&mut *rule).collect();
            D::IntervalSelect {
                input,
                thresholds: thresholds.clone(),
                functions,
            }
        }
        D::Cache(input) => D::Cache(rule(input)),
        D::BlendDensity(input) => D::BlendDensity(rule(input)),
        D::Interpolated {
            input,
            cell_size_xz,
            cell_size_y,
        } => D::Interpolated {
            input: rule(input),
            cell_size_xz: *cell_size_xz,
            cell_size_y: *cell_size_y,
        },
        D::Slice {
            axis,
            coordinate,
            input,
        } => D::Slice {
            axis: *axis,
            coordinate: *coordinate,
            input: rule(input),
        },
        D::FindTopSurface {
            density,
            upper_bound,
            lower_bound,
            cell_height,
        } => {
            let density = rule(density);
            let upper_bound = rule(upper_bound);
            D::FindTopSurface {
                density,
                upper_bound,
                lower_bound: *lower_bound,
                cell_height: *cell_height,
            }
        }
    };
    Arc::new(new)
}

fn inline_reference(f: &Df) -> Df {
    match &**f {
        DensityFunction::Holder(h) => h.value().clone(),
        _ => f.clone(),
    }
}

fn existing_removed_axes(f: &Df) -> u8 {
    let mut axes = 0;
    let mut current = f;
    while let DensityFunction::Slice { axis, input, .. } = &**current {
        axes |= axes_from(*axis);
        current = input;
    }
    axes
}

fn remove_axes(f: Df, axes: u8) -> Df {
    let filtered = axes & !existing_removed_axes(&f);
    let mut f = f;
    for (bit, axis) in [(AXIS_X, Axis::X), (AXIS_Z, Axis::Z), (AXIS_Y, Axis::Y)] {
        if filtered & bit != 0 {
            f = Arc::new(DensityFunction::Slice {
                axis,
                coordinate: 0,
                input: f,
            });
        }
    }
    f
}

fn slice_uniform_axes(f: &Df, parent: u8) -> Df {
    if matches!(
        &**f,
        DensityFunction::Constant(_) | DensityFunction::Gradient(_)
    ) {
        return f.clone();
    }
    let domain = f.domain_axes();
    if domain == parent {
        return rebuild(f, &mut |c| slice_uniform_axes(c, parent));
    }
    let new = rebuild(f, &mut |c| slice_uniform_axes(c, domain));
    remove_axes(new, parent & !domain)
}

impl DensityFunctionCompiler {
    pub fn new(context: CompileContext) -> Self {
        Self {
            context,
            samplers: HashMap::new(),
            prepared_caches: HashMap::new(),
            cache_samplers: Vec::new(),
        }
    }

    pub fn get_sampler(&mut self, f: &Df) -> S {
        if let Some(s) = self.samplers.get(f) {
            return s.clone();
        }
        let optimized = self.optimize(f);
        let sampler = self.compile(&optimized);
        self.samplers.insert(f.clone(), sampler.clone());
        sampler
    }

    pub fn optimize(&mut self, f: &Df) -> Df {
        let rewritten = self.apply(Rule::Optimize, f);
        self.apply(Rule::SliceUniformAxes(ALL_AXES), &rewritten)
    }

    fn apply(&mut self, rule: Rule, f: &Df) -> Df {
        match rule {
            Rule::Optimize => {
                let f = inline_reference(f);
                if let DensityFunction::Cache(input) = &*f {
                    self.reuse_or_prepare_cache(input)
                } else {
                    rebuild(&f, &mut |c| self.apply(Rule::Optimize, c))
                }
            }
            Rule::SliceUniformAxes(parent) => slice_uniform_axes(f, parent),
        }
    }

    fn reuse_or_prepare_cache(&mut self, input: &Df) -> Df {
        if let Some(prepared) = self.prepared_caches.get(input) {
            return prepared.clone();
        }
        let id = self.cache_samplers.len();
        self.cache_samplers.push(node(Sampler::Constant(f32::NAN)));
        let optimized = self.optimize(input);
        let input_sampler = self.compile(&optimized);
        self.cache_samplers[id] = node(Sampler::Caching {
            id,
            input: input_sampler,
        });
        let prepared = Arc::new(DensityFunction::PreparedCache(PreparedCache {
            id,
            range: optimized.range(),
            domain_axes: optimized.domain_axes(),
            input: optimized,
        }));
        self.prepared_caches.insert(input.clone(), prepared.clone());
        prepared
    }

    pub fn compile(&mut self, f: &Df) -> S {
        use DensityFunction as D;
        let sampler = match &**f {
            D::Constant(v) => Sampler::Constant(*v),
            D::Holder(h) => return self.compile(&h.value().clone()),
            D::PreparedCache(p) => return self.cache_samplers[p.id].clone(),
            D::Cache(_) => panic!("Cannot compile cache before it has been deduplicated"),
            D::Noise {
                noise,
                xz_scale,
                y_scale,
                shift_x,
                shift_y,
                shift_z,
            } => {
                let instance = self.context.create_noise_sampler(noise);
                if shift_x.is_zero_constant()
                    && shift_y.is_zero_constant()
                    && shift_z.is_zero_constant()
                {
                    Sampler::Noise {
                        noise: instance,
                        xz_scale: *xz_scale,
                        y_scale: *y_scale,
                    }
                } else {
                    let sx = self.compile(shift_x);
                    let sz = self.compile(shift_z);
                    if shift_y.is_zero_constant() {
                        Sampler::ShiftedXz {
                            shift_x: sx,
                            shift_z: sz,
                            noise: instance,
                            xz_scale: *xz_scale,
                            y_scale: *y_scale,
                        }
                    } else {
                        Sampler::ShiftedXyz {
                            shift_x: sx,
                            shift_y: self.compile(shift_y),
                            shift_z: sz,
                            noise: instance,
                            xz_scale: *xz_scale,
                            y_scale: *y_scale,
                        }
                    }
                }
            }
            D::Shift(n) => Sampler::ConstMul(
                node(Sampler::Noise {
                    noise: self.context.create_noise_sampler(n),
                    xz_scale: 0.25,
                    y_scale: 0.25,
                }),
                4.0,
            ),
            D::ShiftA(n) => Sampler::ConstMul(
                node(Sampler::Noise {
                    noise: self.context.create_noise_sampler(n),
                    xz_scale: 0.25,
                    y_scale: 0.0,
                }),
                4.0,
            ),
            D::ShiftB(n) => Sampler::ShiftB(self.context.create_noise_sampler(n)),
            D::EndOuterIslands => {
                let mut random = self.context.create_end_island_random();
                random.consume_count(17292);
                Sampler::EndIsland(Arc::new(SimplexNoise::new_discarding_offset(
                    &mut random,
                    true,
                )))
            }
            D::DistanceToPoint { point, metric } => Sampler::DistanceToPoint {
                point: *point,
                metric: *metric,
            },
            D::Gradient(g) => {
                let coordinate_range = g.to_coordinate - g.from_coordinate;
                let coordinate_factor = (g.to_value - g.from_value) / coordinate_range as f32;
                Sampler::Gradient {
                    axis: g.axis,
                    tiling: match g.tiling {
                        TilingMode::ClampToEdge => GradientTiling::Clamped {
                            min: g.from_coordinate.min(g.to_coordinate),
                            max: g.from_coordinate.max(g.to_coordinate),
                        },
                        TilingMode::Repeat => GradientTiling::Repeat {
                            range: coordinate_range,
                        },
                        TilingMode::MirroredRepeat => GradientTiling::MirroredRepeat {
                            range: coordinate_range,
                        },
                    },
                    from_coordinate: g.from_coordinate,
                    from_value: g.from_value,
                    coordinate_factor,
                }
            }
            D::Unary(kind, input) => {
                let input = self.compile(input);
                match kind {
                    UnaryType::Abs => Sampler::Abs(input),
                    UnaryType::Square => Sampler::Square(input),
                    UnaryType::Cube => Sampler::Cube(input),
                    UnaryType::Sqrt => Sampler::Sqrt(input),
                    UnaryType::HalfNegative => Sampler::LeakyRelu(input, 0.5),
                    UnaryType::QuarterNegative => Sampler::LeakyRelu(input, 0.25),
                    UnaryType::Reciprocal => Sampler::Reciprocal(input),
                    UnaryType::Negate => Sampler::Negate(input),
                    UnaryType::Squeeze => Sampler::Squeeze(input),
                    UnaryType::Log => Sampler::Log(input),
                    UnaryType::Sign => Sampler::Sign(input),
                }
            }
            D::Round(kind, input, multiple) => {
                let input = self.compile(input);
                if matches!(&**multiple, D::Constant(v) if *v == 1.0) {
                    Sampler::RoundInteger(*kind, input)
                } else {
                    Sampler::Round(*kind, input, self.compile(multiple))
                }
            }
            D::Binary(kind, left_fn, right_fn) => {
                return self.compile_binary(*kind, left_fn, right_fn);
            }
            D::Pow(base_fn, exponent_fn) => {
                let base = self.compile(base_fn);
                let exponent = self.compile(exponent_fn);
                if let D::Constant(b) = &**base_fn {
                    Sampler::PowConstBase(f64::from(*b), exponent)
                } else if let D::Constant(e) = &**exponent_fn {
                    return compile_const_exponent(base, *e);
                } else {
                    Sampler::Pow(base, exponent)
                }
            }
            D::Spline(spline) => self.compile_spline(spline),
            D::Lerp(alpha_fn, first_fn, second_fn) => {
                let alpha = self.compile(alpha_fn);
                let first = self.compile(first_fn);
                let second = self.compile(second_fn);
                if let D::Constant(v) = &**first_fn {
                    Sampler::LerpConstFirst(alpha, *v, second)
                } else if let D::Constant(v) = &**second_fn {
                    Sampler::LerpConstSecond(alpha, first, *v)
                } else {
                    Sampler::Lerp(alpha, first, second)
                }
            }
            D::Clamp(input, min, max) => Sampler::Clamp(self.compile(input), *min, *max),
            D::RangeChoice {
                input,
                min_inclusive,
                max_exclusive,
                when_in_range,
                when_out_of_range,
            } => {
                let input = self.compile(input);
                if let (D::Constant(a), D::Constant(b)) = (&**when_in_range, &**when_out_of_range) {
                    Sampler::RangeChoiceConst {
                        input,
                        min_inclusive: *min_inclusive,
                        max_exclusive: *max_exclusive,
                        when_in_range: *a,
                        when_out_of_range: *b,
                    }
                } else {
                    Sampler::RangeChoice {
                        input,
                        min_inclusive: *min_inclusive,
                        max_exclusive: *max_exclusive,
                        when_in_range: self.compile(when_in_range),
                        when_out_of_range: self.compile(when_out_of_range),
                    }
                }
            }
            D::IntervalSelect {
                input,
                thresholds,
                functions,
            } => {
                let input = self.compile(input);
                if thresholds.len() == 1 {
                    Sampler::IntervalSelectSingle {
                        input,
                        threshold: thresholds[0],
                        if_below: self.compile(&functions[0]),
                        if_above: self.compile(&functions[functions.len() - 1]),
                    }
                } else {
                    Sampler::IntervalSelect {
                        input,
                        thresholds: thresholds.clone(),
                        samplers: functions.iter().map(|f| self.compile(f)).collect(),
                    }
                }
            }
            D::BlendDensity(input) => Sampler::BlendDensity(self.compile(input)),
            D::Interpolated {
                input,
                cell_size_xz,
                cell_size_y,
            } => Sampler::Interpolated {
                input: self.compile(input),
                cell_size_xz: *cell_size_xz,
                cell_size_y: *cell_size_y,
                cell_size_xz_inv: 1.0 / *cell_size_xz as f32,
                cell_size_y_inv: 1.0 / *cell_size_y as f32,
            },
            D::Slice {
                axis,
                coordinate,
                input,
            } => {
                if let D::Slice {
                    axis: inner_axis,
                    coordinate: inner_coordinate,
                    input: inner_input,
                } = &**input
                    && matches!((axis, inner_axis), (Axis::X, Axis::Z) | (Axis::Z, Axis::X))
                {
                    let (x, z) = if *axis == Axis::X {
                        (*coordinate, *inner_coordinate)
                    } else {
                        (*inner_coordinate, *coordinate)
                    };
                    Sampler::SliceXz(self.compile(inner_input), x, z)
                } else {
                    let input = self.compile(input);
                    match axis {
                        Axis::X => Sampler::SliceX(input, *coordinate),
                        Axis::Y => Sampler::SliceY(input, *coordinate),
                        Axis::Z => Sampler::SliceZ(input, *coordinate),
                    }
                }
            }
            D::FindTopSurface {
                density,
                upper_bound,
                lower_bound,
                cell_height,
            } => {
                let inner = Sampler::FindTopSurface {
                    density: self.compile(density),
                    upper_bound: self.compile(upper_bound),
                    lower_bound: *lower_bound,
                    cell_height: *cell_height,
                };
                Sampler::SliceY(node(inner), 0)
            }
            D::OldBlendedNoise(blended) => self.compile_blended(*blended),
            D::Simple(kind) => Sampler::ContextBound {
                key: *kind,
                fallback: match kind {
                    SimpleType::BlendAlpha => 1.0,
                    SimpleType::BlendOffset | SimpleType::Beardifier => 0.0,
                },
            },
        };
        node(sampler)
    }

    fn compile_binary(&mut self, kind: BinaryType, left_fn: &Df, right_fn: &Df) -> S {
        use DensityFunction as D;
        let left = self.compile(left_fn);
        let right = self.compile(right_fn);
        let left_const = match &**left_fn {
            D::Constant(v) => Some(*v),
            _ => None,
        };
        let right_const = match &**right_fn {
            D::Constant(v) => Some(*v),
            _ => None,
        };
        let sampler = match kind {
            BinaryType::Add => match (left_const, right_const) {
                (Some(l), _) => Sampler::ConstAdd(right, l),
                (None, Some(r)) => Sampler::ConstAdd(left, r),
                _ => Sampler::Add(left, right),
            },
            BinaryType::Sub => match (left_const, right_const) {
                (Some(l), _) => Sampler::ConstSub(l, right),
                (None, Some(r)) => Sampler::ConstAdd(left, -r),
                _ => Sampler::Sub(left, right),
            },
            BinaryType::Mul => match (left_const, right_const) {
                (Some(l), _) => Sampler::ConstMul(right, l),
                (None, Some(r)) => Sampler::ConstMul(left, r),
                _ => Sampler::Mul(left, right),
            },
            BinaryType::Div => match (left_const, right_const) {
                (Some(l), _) => Sampler::ConstDiv(l, right),
                (None, Some(r)) => Sampler::ConstMul(left, 1.0 / r),
                _ => Sampler::Div(left, right),
            },
            BinaryType::Min => {
                let lr = left_fn.range();
                let rr = right_fn.range();
                if lr.max() < rr.min() {
                    tracing::warn!("Compiling a min function between two non-overlapping inputs");
                    return left;
                } else if rr.max() < lr.min() {
                    tracing::warn!("Compiling a min function between two non-overlapping inputs");
                    return right;
                } else if let Some(l) = left_const {
                    Sampler::ConstMin(right, l)
                } else if let Some(r) = right_const {
                    Sampler::ConstMin(left, r)
                } else {
                    Sampler::Min(left, right, rr.min())
                }
            }
            BinaryType::Max => {
                let lr = left_fn.range();
                let rr = right_fn.range();
                if lr.min() > rr.max() {
                    tracing::warn!("Compiling a max function between two non-overlapping inputs");
                    return left;
                } else if rr.min() > lr.max() {
                    tracing::warn!("Compiling a max function between two non-overlapping inputs");
                    return right;
                } else if let Some(l) = left_const {
                    Sampler::ConstMax(right, l)
                } else if let Some(r) = right_const {
                    Sampler::ConstMax(left, r)
                } else {
                    Sampler::Max(left, right, rr.max())
                }
            }
        };
        node(sampler)
    }

    fn compile_spline(&mut self, spline: &Arc<CubicSpline>) -> Sampler {
        let mut coordinates: Vec<(Df, S)> = Vec::new();
        let root = self.spline_node(spline, &mut coordinates);
        Sampler::Spline {
            root,
            coordinates: coordinates.into_iter().map(|(_, s)| s).collect(),
        }
    }

    fn spline_node(
        &mut self,
        spline: &Arc<CubicSpline>,
        coordinates: &mut Vec<(Df, S)>,
    ) -> SplineNode {
        match &**spline {
            CubicSpline::Constant(v) => SplineNode::Constant(*v),
            CubicSpline::Multipoint {
                coordinate,
                locations,
                values,
                derivatives,
            } => {
                let index = if let Some(i) = coordinates.iter().position(|(f, _)| f == coordinate) {
                    i
                } else {
                    let sampler = self.compile(coordinate);
                    coordinates.push((coordinate.clone(), sampler));
                    coordinates.len() - 1
                };
                SplineNode::Multipoint {
                    coordinate: index,
                    locations: locations.clone(),
                    values: values
                        .iter()
                        .map(|v| self.spline_node(v, coordinates))
                        .collect(),
                    derivatives: derivatives.clone(),
                }
            }
        }
    }

    fn compile_blended(&mut self, blended: BlendedNoise) -> Sampler {
        let mut random = self.context.create_random(BLENDED_NOISE_SEED);
        let limit_smear_scale_y = blended.y_multiplier() * blended.smear_scale_multiplier;
        let main_smear_scale_y = limit_smear_scale_y / blended.y_factor;
        let limit_factor = f64::from(0.999_984_74_f32);
        let min_limit = create_fbm(&mut random, -15, limit_smear_scale_y, limit_factor);
        let max_limit = create_fbm(&mut random, -15, limit_smear_scale_y, limit_factor);
        let main = create_fbm(&mut random, -7, main_smear_scale_y, 12.75);
        let xz_multiplier = blended.xz_multiplier();
        let y_multiplier = blended.y_multiplier();
        let noise = |stack: NoiseStack, xz_scale: f64, y_scale: f64| {
            node(Sampler::Noise {
                noise: Arc::new(Noise::Stack(stack)),
                xz_scale,
                y_scale,
            })
        };
        let min_limit_noise = noise(min_limit, xz_multiplier, y_multiplier);
        let max_limit_noise = noise(max_limit, xz_multiplier, y_multiplier);
        let main_noise = noise(
            main,
            xz_multiplier / blended.xz_factor,
            y_multiplier / blended.y_factor,
        );
        let choice = node(Sampler::Clamp(
            node(Sampler::ConstAdd(main_noise, 0.5)),
            0.0,
            1.0,
        ));
        Sampler::Lerp(choice, min_limit_noise, max_limit_noise)
    }
}

fn compile_const_exponent(base: S, exponent: f32) -> S {
    let abs = exponent.abs();
    let special = if abs == 0.5 {
        node(Sampler::Sqrt(base))
    } else if abs == 1.0 {
        base
    } else if abs == 2.0 {
        node(Sampler::Square(base))
    } else if abs == 3.0 {
        node(Sampler::Cube(base))
    } else {
        return node(Sampler::PowConstExponent(base, f64::from(exponent)));
    };
    if exponent >= 0.0 {
        special
    } else {
        node(Sampler::Reciprocal(special))
    }
}

pub fn create_fbm(
    random: &mut RandomSource,
    first_octave: i32,
    smear_scale_y: f64,
    value_factor: f64,
) -> NoiseStack {
    assert!(first_octave <= 0, "firstOctave>0");
    let octaves = -first_octave + 1;
    let mut factor = 1.0;
    let mut value_factor = value_factor / (2.0f64.powf(f64::from(octaves)) - 1.0);
    let mut stack = NoiseStack::builder();
    for _ in 0..octaves {
        stack = stack.add(
            Noise::SmearedPerlin(SmearedPerlinNoise::new(random, smear_scale_y * factor)),
            factor,
            value_factor as f32,
        );
        factor /= 2.0;
        value_factor *= 2.0;
    }
    stack.build()
}
