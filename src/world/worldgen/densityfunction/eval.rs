use shulkr::util::{Axis, BoundingBox};

use crate::util::interval::{java_max, java_min, java_signum};
use crate::world::worldgen::backend::jit;
use crate::world::worldgen::synth::gradient_noise::lerp3;
use crate::world::worldgen::synth::{Noise, SimplexNoise};

use super::context::{SamplerContext, block_pos_as_long};
use super::function::{DistanceMetric, RoundType, SimpleType, clamp, round_to_integer, squeeze};
use super::sampler::{GradientTiling, S, Sampler, SamplerNode, SplineNode};
use super::spline::{find_interval_start, linear_extend};
use super::volume::DensityVolume;
use crate::world::worldgen::beardifier::Beardifier;
use std::mem;
use std::ptr;

fn lerp(alpha: f32, p0: f32, p1: f32) -> f32 {
    p0 + alpha * (p1 - p0)
}

fn leaky_relu(negative_factor: f32, input: f32) -> f32 {
    if input > 0.0 {
        input
    } else {
        input * negative_factor
    }
}

fn sqrt(v: f32) -> f32 {
    f64::from(v).sqrt() as f32
}

pub fn distance(metric: DistanceMetric, dx: f32, dy: f32, dz: f32) -> f32 {
    match metric {
        DistanceMetric::Euclidean => sqrt(dx * dx + dy * dy + dz * dz),
        DistanceMetric::EuclideanSquared => dx * dx + dy * dy + dz * dz,
        DistanceMetric::Manhattan => dx.abs() + dy.abs() + dz.abs(),
        DistanceMetric::Chebyshev => java_max(java_max(dx.abs(), dy.abs()), dz.abs()),
    }
}

pub fn gradient_compute(
    tiling: GradientTiling,
    from_coordinate: i32,
    from_value: f32,
    coordinate_factor: f32,
    coordinate: i32,
) -> f32 {
    match tiling {
        GradientTiling::Clamped { min, max } => {
            let relative = coordinate.clamp(min, max) - from_coordinate;
            from_value + relative as f32 * coordinate_factor
        }
        GradientTiling::Repeat { range } => {
            let relative = coordinate - from_coordinate;
            from_value + relative.rem_euclid(range) as f32 * coordinate_factor
        }
        GradientTiling::MirroredRepeat { range } => {
            let relative = coordinate - from_coordinate;
            let tile = relative.div_euclid(range);
            let local = relative - tile * range;
            if tile & 1 == 0 {
                from_value + local as f32 * coordinate_factor
            } else {
                from_value + (range - local) as f32 * coordinate_factor
            }
        }
    }
}

pub fn end_island_height(noise: &SimplexNoise, section_x: i32, section_z: i32) -> f32 {
    let chunk_x = section_x / 2;
    let chunk_z = section_z / 2;
    let sub_x = section_x % 2;
    let sub_z = section_z % 2;
    let mut doffs = -100.0f32;
    for xo in -12..=12 {
        for zo in -12..=12 {
            let total_x = i64::from(chunk_x + xo);
            let total_z = i64::from(chunk_z + zo);
            if total_x * total_x + total_z * total_z > 4096
                && noise.get2(total_x as f64, total_z as f64) < -0.9
            {
                let island_size =
                    ((total_x as f32).abs() * 3439.0 + (total_z as f32).abs() * 147.0) % 13.0 + 9.0;
                let xd = (sub_x - xo * 2) as f32;
                let zd = (sub_z - zo * 2) as f32;
                let new = 100.0 - sqrt(xd * xd + zd * zd) * island_size;
                let new = clamp(new, -100.0, 80.0);
                doffs = java_max(doffs, new);
            }
        }
    }
    doffs
}

fn round_apply(kind: RoundType, input: f32, multiple: f32) -> f32 {
    if multiple == 0.0 {
        input
    } else {
        round_to_integer(input / multiple, kind) * multiple
    }
}

fn select_index(thresholds: &[f32], input: f32, count: usize) -> usize {
    for (i, t) in thresholds.iter().enumerate() {
        if input < *t {
            return i;
        }
    }
    count - 1
}

pub fn spline_sample(node: &SplineNode, coordinate: &mut impl FnMut(usize) -> f32) -> f32 {
    match node {
        SplineNode::Constant(v) => *v,
        SplineNode::Multipoint {
            coordinate: index,
            locations,
            values,
            derivatives,
        } => {
            let input = coordinate(*index);
            let start = find_interval_start(locations, input);
            let last = locations.len() - 1;
            if start < 0 {
                return linear_extend(
                    input,
                    locations,
                    spline_sample(&values[0], coordinate),
                    derivatives,
                    0,
                );
            }
            let start = start as usize;
            if start == last {
                return linear_extend(
                    input,
                    locations,
                    spline_sample(&values[last], coordinate),
                    derivatives,
                    last,
                );
            }
            let x1 = locations[start];
            let x2 = locations[start + 1];
            let t = (input - x1) / (x2 - x1);
            let d1 = derivatives[start];
            let d2 = derivatives[start + 1];
            let y1 = spline_sample(&values[start], coordinate);
            let y2 = spline_sample(&values[start + 1], coordinate);
            let a = d1 * (x2 - x1) - (y2 - y1);
            let b = -d2 * (x2 - x1) + (y2 - y1);
            lerp(t, y1, y2) + t * (1.0 - t) * lerp(t, a, b)
        }
    }
}

fn binary_children(s: &Sampler) -> (&S, &S) {
    match s {
        Sampler::Add(a, b) | Sampler::Sub(a, b) | Sampler::Mul(a, b) | Sampler::Div(a, b) => (a, b),
        _ => unreachable!(),
    }
}

pub fn sample_value(s: &SamplerNode, ctx: &mut SamplerContext<'_>, x: i32, y: i32, z: i32) -> f32 {
    match jit::value_fn(s) {
        // SAFETY: `compiled` was built for `s` and expects a `SamplerContext`.
        Some(compiled) => unsafe { compiled(ptr::from_mut(ctx).cast::<u8>(), x, y, z) },
        None => interp_value(s, ctx, x, y, z),
    }
}

pub fn interp_value(s: &Sampler, ctx: &mut SamplerContext<'_>, x: i32, y: i32, z: i32) -> f32 {
    match s {
        Sampler::Constant(v) => *v,
        Sampler::Noise {
            noise,
            xz_scale,
            y_scale,
        } => noise.get(
            f64::from(x) * xz_scale,
            f64::from(y) * y_scale,
            f64::from(z) * xz_scale,
        ),
        Sampler::ShiftedXz {
            shift_x,
            shift_z,
            noise,
            xz_scale,
            y_scale,
        } => {
            let nx = f64::from(x) * xz_scale + f64::from(sample_value(shift_x, ctx, x, y, z));
            let ny = f64::from(y) * y_scale;
            let nz = f64::from(z) * xz_scale + f64::from(sample_value(shift_z, ctx, x, y, z));
            noise.get(nx, ny, nz)
        }
        Sampler::ShiftedXyz {
            shift_x,
            shift_y,
            shift_z,
            noise,
            xz_scale,
            y_scale,
        } => {
            let nx = f64::from(x) * xz_scale + f64::from(sample_value(shift_x, ctx, x, y, z));
            let ny = f64::from(y) * y_scale + f64::from(sample_value(shift_y, ctx, x, y, z));
            let nz = f64::from(z) * xz_scale + f64::from(sample_value(shift_z, ctx, x, y, z));
            noise.get(nx, ny, nz)
        }
        Sampler::ShiftB(noise) => noise.get(f64::from(z) * 0.25, f64::from(x) * 0.25, 0.0) * 4.0,
        Sampler::EndIsland(noise) => (end_island_height(noise, x / 8, z / 8) - 8.0) / 128.0,
        Sampler::DistanceToPoint { point, metric } => distance(
            *metric,
            (point[0] - x) as f32,
            (point[1] - y) as f32,
            (point[2] - z) as f32,
        ),
        Sampler::Gradient {
            axis,
            tiling,
            from_coordinate,
            from_value,
            coordinate_factor,
        } => gradient_compute(
            *tiling,
            *from_coordinate,
            *from_value,
            *coordinate_factor,
            match axis {
                Axis::X => x,
                Axis::Y => y,
                Axis::Z => z,
            },
        ),
        Sampler::Abs(i) => sample_value(i, ctx, x, y, z).abs(),
        Sampler::Square(i) => {
            let v = sample_value(i, ctx, x, y, z);
            v * v
        }
        Sampler::Cube(i) => {
            let v = sample_value(i, ctx, x, y, z);
            v * v * v
        }
        Sampler::Sqrt(i) => sqrt(sample_value(i, ctx, x, y, z)),
        Sampler::LeakyRelu(i, f) => leaky_relu(*f, sample_value(i, ctx, x, y, z)),
        Sampler::Reciprocal(i) => 1.0 / sample_value(i, ctx, x, y, z),
        Sampler::Negate(i) => -sample_value(i, ctx, x, y, z),
        Sampler::Squeeze(i) => squeeze(sample_value(i, ctx, x, y, z)),
        Sampler::Log(i) => f64::from(sample_value(i, ctx, x, y, z)).ln() as f32,
        Sampler::Sign(i) => java_signum(sample_value(i, ctx, x, y, z)),
        Sampler::RoundInteger(kind, i) => round_to_integer(sample_value(i, ctx, x, y, z), *kind),
        Sampler::Round(kind, i, m) => {
            let input = sample_value(i, ctx, x, y, z);
            let multiple = sample_value(m, ctx, x, y, z);
            round_apply(*kind, input, multiple)
        }
        Sampler::Add(..) => {
            let (l, r) = binary_children(s);
            sample_value(l, ctx, x, y, z) + sample_value(r, ctx, x, y, z)
        }
        Sampler::ConstAdd(l, c) => sample_value(l, ctx, x, y, z) + c,
        Sampler::Sub(..) => {
            let (l, r) = binary_children(s);
            sample_value(l, ctx, x, y, z) - sample_value(r, ctx, x, y, z)
        }
        Sampler::ConstSub(c, r) => c - sample_value(r, ctx, x, y, z),
        Sampler::Mul(..) => {
            let (l, r) = binary_children(s);
            let left = sample_value(l, ctx, x, y, z);
            if left == 0.0 {
                0.0
            } else {
                left * sample_value(r, ctx, x, y, z)
            }
        }
        Sampler::ConstMul(l, c) => sample_value(l, ctx, x, y, z) * c,
        Sampler::Div(..) => {
            let (l, r) = binary_children(s);
            let left = sample_value(l, ctx, x, y, z);
            if left == 0.0 {
                0.0
            } else {
                left / sample_value(r, ctx, x, y, z)
            }
        }
        Sampler::ConstDiv(c, r) => c / sample_value(r, ctx, x, y, z),
        Sampler::Min(l, r, right_min) => {
            let left = sample_value(l, ctx, x, y, z);
            if left <= *right_min {
                left
            } else {
                java_min(left, sample_value(r, ctx, x, y, z))
            }
        }
        Sampler::ConstMin(l, c) => java_min(sample_value(l, ctx, x, y, z), *c),
        Sampler::Max(l, r, right_max) => {
            let left = sample_value(l, ctx, x, y, z);
            if left >= *right_max {
                left
            } else {
                java_max(left, sample_value(r, ctx, x, y, z))
            }
        }
        Sampler::ConstMax(l, c) => java_max(sample_value(l, ctx, x, y, z), *c),
        Sampler::PowConstBase(base, e) => {
            base.powf(f64::from(sample_value(e, ctx, x, y, z))) as f32
        }
        Sampler::PowConstExponent(b, exponent) => {
            f64::from(sample_value(b, ctx, x, y, z)).powf(*exponent) as f32
        }
        Sampler::Pow(b, e) => {
            let base = sample_value(b, ctx, x, y, z);
            let exponent = sample_value(e, ctx, x, y, z);
            f64::from(base).powf(f64::from(exponent)) as f32
        }
        Sampler::Spline { root, coordinates } => {
            let mut cached = vec![f32::NAN; coordinates.len()];
            spline_sample(root, &mut |index| {
                let value = cached[index];
                if !value.is_nan() {
                    return value;
                }
                let value = sample_value(&coordinates[index], ctx, x, y, z);
                cached[index] = value;
                value
            })
        }
        Sampler::LerpConstFirst(alpha, first, second) => {
            let a = sample_value(alpha, ctx, x, y, z);
            if a == 0.0 {
                *first
            } else if a == 1.0 {
                sample_value(second, ctx, x, y, z)
            } else {
                lerp(a, *first, sample_value(second, ctx, x, y, z))
            }
        }
        Sampler::LerpConstSecond(alpha, first, second) => {
            let a = sample_value(alpha, ctx, x, y, z);
            if a == 0.0 {
                sample_value(first, ctx, x, y, z)
            } else if a == 1.0 {
                *second
            } else {
                lerp(a, sample_value(first, ctx, x, y, z), *second)
            }
        }
        Sampler::Lerp(alpha, first, second) => {
            let a = sample_value(alpha, ctx, x, y, z);
            if a == 0.0 {
                sample_value(first, ctx, x, y, z)
            } else if a == 1.0 {
                sample_value(second, ctx, x, y, z)
            } else {
                let f = sample_value(first, ctx, x, y, z);
                let sv = sample_value(second, ctx, x, y, z);
                lerp(a, f, sv)
            }
        }
        Sampler::Clamp(i, min, max) => clamp(sample_value(i, ctx, x, y, z), *min, *max),
        Sampler::RangeChoiceConst {
            input,
            min_inclusive,
            max_exclusive,
            when_in_range,
            when_out_of_range,
        } => {
            let v = sample_value(input, ctx, x, y, z);
            if v >= *min_inclusive && v < *max_exclusive {
                *when_in_range
            } else {
                *when_out_of_range
            }
        }
        Sampler::RangeChoice {
            input,
            min_inclusive,
            max_exclusive,
            when_in_range,
            when_out_of_range,
        } => {
            let v = sample_value(input, ctx, x, y, z);
            if v >= *min_inclusive && v < *max_exclusive {
                sample_value(when_in_range, ctx, x, y, z)
            } else {
                sample_value(when_out_of_range, ctx, x, y, z)
            }
        }
        Sampler::IntervalSelect {
            input,
            thresholds,
            samplers,
        } => {
            let v = sample_value(input, ctx, x, y, z);
            let index = select_index(thresholds, v, samplers.len());
            sample_value(&samplers[index], ctx, x, y, z)
        }
        Sampler::IntervalSelectSingle {
            input,
            threshold,
            if_below,
            if_above,
        } => {
            let v = sample_value(input, ctx, x, y, z);
            if v < *threshold {
                sample_value(if_below, ctx, x, y, z)
            } else {
                sample_value(if_above, ctx, x, y, z)
            }
        }
        Sampler::Caching { id, input } => sample_value_cached(*id, input, ctx, x, y, z),
        Sampler::ContextBound { key, fallback } => match key {
            SimpleType::Beardifier => match ctx.beardifier {
                Some(b) => b.sample_value(x, y, z),
                None => *fallback,
            },
            SimpleType::BlendAlpha | SimpleType::BlendOffset => *fallback,
        },
        Sampler::BlendDensity(i) => sample_value(i, ctx, x, y, z),
        Sampler::Interpolated {
            input,
            cell_size_xz,
            cell_size_y,
            ..
        } => {
            let xin = x.rem_euclid(*cell_size_xz);
            let yin = y.rem_euclid(*cell_size_y);
            let zin = z.rem_euclid(*cell_size_xz);
            if xin == 0 && yin == 0 && zin == 0 {
                return sample_value(input, ctx, x, y, z);
            }
            let volume = DensityVolume::stepped(
                [2, 2, 2],
                [x - xin, y - yin, z - zin],
                [*cell_size_xz, *cell_size_y, *cell_size_xz],
            );
            let mut buffer = vec![0.0f32; volume.size()];
            sample_volume(input, ctx, &mut buffer, &volume);
            lerp3(
                xin as f32 / *cell_size_xz as f32,
                yin as f32 / *cell_size_y as f32,
                zin as f32 / *cell_size_xz as f32,
                buffer[volume.index_unchecked(0, 0, 0)],
                buffer[volume.index_unchecked(1, 0, 0)],
                buffer[volume.index_unchecked(0, 1, 0)],
                buffer[volume.index_unchecked(1, 1, 0)],
                buffer[volume.index_unchecked(0, 0, 1)],
                buffer[volume.index_unchecked(1, 0, 1)],
                buffer[volume.index_unchecked(0, 1, 1)],
                buffer[volume.index_unchecked(1, 1, 1)],
            )
        }
        Sampler::SliceX(i, sx) => sample_value(i, ctx, *sx, y, z),
        Sampler::SliceY(i, sy) => sample_value(i, ctx, x, *sy, z),
        Sampler::SliceZ(i, sz) => sample_value(i, ctx, x, y, *sz),
        Sampler::SliceXz(i, sx, sz) => sample_value(i, ctx, *sx, y, *sz),
        Sampler::FindTopSurface {
            density,
            upper_bound,
            lower_bound,
            cell_height,
        } => {
            let upper = sample_value(upper_bound, ctx, x, y, z);
            find_surface_from(density, *lower_bound, *cell_height, ctx, x, z, upper) as f32
        }
    }
}

fn find_surface_from(
    density: &S,
    lower_bound: i32,
    cell_height: i32,
    ctx: &mut SamplerContext<'_>,
    x: i32,
    z: i32,
    upper_bound: f32,
) -> i32 {
    let top_y = (f64::from(upper_bound / cell_height as f32).floor() as i32) * cell_height;
    if top_y <= lower_bound {
        return lower_bound;
    }
    let mut probe_y = top_y;
    while probe_y >= lower_bound {
        if sample_value(density, ctx, x, probe_y, z) > 0.0 {
            return probe_y;
        }
        probe_y -= cell_height;
    }
    lower_bound
}

fn sample_value_cached(
    id: usize,
    input: &S,
    ctx: &mut SamplerContext<'_>,
    x: i32,
    y: i32,
    z: i32,
) -> f32 {
    let Some(cell) = ctx.cache_cell(id) else {
        return sample_value(input, ctx, x, y, z);
    };
    let key = block_pos_as_long(x, y, z);
    if cell.value_key == key && !cell.value.is_nan() {
        return cell.value;
    }
    if let Some(volume) = cell.volume
        && let Some(index) = volume.index_of_block(x, y, z)
    {
        return cell.buffer[index];
    }
    let value = sample_value(input, ctx, x, y, z);
    let cell = ctx.cache_cell(id).expect("caches enabled");
    cell.value_key = key;
    cell.value = value;
    value
}

fn sample_volume_cached(
    id: usize,
    input: &S,
    ctx: &mut SamplerContext<'_>,
    out: &mut [f32],
    volume: &DensityVolume,
) {
    let Some(cell) = ctx.cache_cell(id) else {
        sample_volume(input, ctx, out, volume);
        return;
    };
    if cell.volume.as_ref() != Some(volume) {
        let mut buffer = mem::take(&mut cell.buffer);
        buffer.clear();
        buffer.resize(volume.size(), 0.0);
        cell.volume = None;
        sample_volume(input, ctx, &mut buffer, volume);
        let cell = ctx.cache_cell(id).expect("caches enabled");
        cell.volume = Some(*volume);
        cell.buffer = buffer;
    }
    let cell = ctx.cache_cell(id).expect("caches enabled");
    out.copy_from_slice(&cell.buffer[..volume.size()]);
}

fn add_noise(noise: &Noise, out: &mut [f32], volume: &DensityVolume, xz: f64, y: f64, amp: f32) {
    noise.add_to_volume(out, volume, xz, y, amp);
}

pub fn sample_volume(
    s: &SamplerNode,
    ctx: &mut SamplerContext<'_>,
    out: &mut [f32],
    volume: &DensityVolume,
) {
    let Some(kernel) = jit::volume_kernel(s) else {
        interp_volume(s, ctx, out, volume);
        return;
    };
    let size = volume.size();
    let buffers: Vec<Vec<f32>> = kernel
        .boundaries
        .iter()
        .map(|boundary| {
            let mut buffer = vec![0.0f32; size];
            sample_volume(boundary, ctx, &mut buffer, volume);
            buffer
        })
        .collect();
    let pointers: Vec<*const f32> = buffers.iter().map(|buffer| buffer.as_ptr()).collect();
    // SAFETY: the kernel was built for `s`; `out` and every boundary buffer hold
    // `volume.size()` values, matching the extents passed below.
    unsafe {
        (kernel.code)(
            out.as_mut_ptr(),
            pointers.as_ptr(),
            volume.size_x,
            volume.size_y,
            volume.size_z,
            volume.min_block_x,
            volume.min_block_y,
            volume.min_block_z,
            volume.step_block_x,
            volume.step_block_y,
            volume.step_block_z,
        );
    }
}

pub fn interp_volume(
    s: &Sampler,
    ctx: &mut SamplerContext<'_>,
    out: &mut [f32],
    volume: &DensityVolume,
) {
    let size = volume.size();
    let out = &mut out[..size];
    let scratch = |ctx: &mut SamplerContext<'_>, child: &S| {
        let mut buffer = vec![0.0f32; size];
        sample_volume(child, ctx, &mut buffer, volume);
        buffer
    };
    match s {
        Sampler::Constant(v) => out.fill(*v),
        Sampler::Noise {
            noise,
            xz_scale,
            y_scale,
        } => {
            out.fill(0.0);
            add_noise(noise, out, volume, *xz_scale, *y_scale, 1.0);
        }
        Sampler::ShiftedXz {
            shift_x,
            shift_z,
            noise,
            xz_scale,
            y_scale,
        } => {
            sample_volume(shift_x, ctx, out, volume);
            let shift_z_buffer = scratch(ctx, shift_z);
            let mut index = 0;
            for zi in 0..volume.size_z {
                let base_z = f64::from(volume.block_z(zi)) * xz_scale;
                for xi in 0..volume.size_x {
                    let base_x = f64::from(volume.block_x(xi)) * xz_scale;
                    for yi in 0..volume.size_y {
                        let nx = base_x + f64::from(out[index]);
                        let ny = f64::from(volume.block_y(yi)) * y_scale;
                        let nz = base_z + f64::from(shift_z_buffer[index]);
                        out[index] = noise.get(nx, ny, nz);
                        index += 1;
                    }
                }
            }
        }
        Sampler::ShiftedXyz {
            shift_x,
            shift_y,
            shift_z,
            noise,
            xz_scale,
            y_scale,
        } => {
            sample_volume(shift_x, ctx, out, volume);
            let shift_y_buffer = scratch(ctx, shift_y);
            let shift_z_buffer = scratch(ctx, shift_z);
            let mut index = 0;
            for zi in 0..volume.size_z {
                let base_z = f64::from(volume.block_z(zi)) * xz_scale;
                for xi in 0..volume.size_x {
                    let base_x = f64::from(volume.block_x(xi)) * xz_scale;
                    for yi in 0..volume.size_y {
                        let nx = base_x + f64::from(out[index]);
                        let ny = f64::from(volume.block_y(yi)) * y_scale
                            + f64::from(shift_y_buffer[index]);
                        let nz = base_z + f64::from(shift_z_buffer[index]);
                        out[index] = noise.get(nx, ny, nz);
                        index += 1;
                    }
                }
            }
        }
        Sampler::ShiftB(noise) => {
            let transposed = DensityVolume::stepped(
                [volume.size_z, volume.size_x, 1],
                [volume.min_block_z, volume.min_block_x, 0],
                [volume.step_block_z, volume.step_block_x, 1],
            );
            let mut buffer = vec![0.0f32; transposed.size()];
            add_noise(noise, &mut buffer, &transposed, 0.25, 0.25, 4.0);
            for zi in 0..volume.size_z {
                for xi in 0..volume.size_x {
                    let value = buffer[transposed.index_unchecked(zi, xi, 0)];
                    let start = volume.index_unchecked(xi, 0, zi);
                    out[start..start + volume.size_y as usize].fill(value);
                }
            }
        }
        Sampler::EndIsland(_) => {
            for zi in 0..volume.size_z {
                let bz = volume.block_z(zi);
                for xi in 0..volume.size_x {
                    let bx = volume.block_x(xi);
                    let value = interp_value(s, ctx, bx, 0, bz);
                    let start = volume.index_unchecked(xi, 0, zi);
                    out[start..start + volume.size_y as usize].fill(value);
                }
            }
        }
        Sampler::DistanceToPoint { .. } => {
            let mut index = 0;
            for zi in 0..volume.size_z {
                let bz = volume.block_z(zi);
                for xi in 0..volume.size_x {
                    let bx = volume.block_x(xi);
                    for yi in 0..volume.size_y {
                        out[index] = interp_value(s, ctx, bx, volume.block_y(yi), bz);
                        index += 1;
                    }
                }
            }
        }
        Sampler::Gradient {
            axis,
            tiling,
            from_coordinate,
            from_value,
            coordinate_factor,
        } => {
            let compute = |c: i32| {
                gradient_compute(
                    *tiling,
                    *from_coordinate,
                    *from_value,
                    *coordinate_factor,
                    c,
                )
            };
            match axis {
                Axis::X => {
                    for xi in 0..volume.size_x {
                        let value = compute(volume.block_x(xi));
                        for zi in 0..volume.size_z {
                            let start = volume.index_unchecked(xi, 0, zi);
                            out[start..start + volume.size_y as usize].fill(value);
                        }
                    }
                }
                Axis::Y => {
                    for yi in 0..volume.size_y {
                        let value = compute(volume.block_y(yi));
                        for zi in 0..volume.size_z {
                            for xi in 0..volume.size_x {
                                out[volume.index_unchecked(xi, yi, zi)] = value;
                            }
                        }
                    }
                }
                Axis::Z => {
                    for zi in 0..volume.size_z {
                        let value = compute(volume.block_z(zi));
                        let start = volume.index_unchecked(0, 0, zi);
                        out[start..start + (volume.size_x * volume.size_y) as usize].fill(value);
                    }
                }
            }
        }
        Sampler::Abs(i) => map(i, ctx, out, volume, f32::abs),
        Sampler::Square(i) => map(i, ctx, out, volume, |v| v * v),
        Sampler::Cube(i) => map(i, ctx, out, volume, |v| v * v * v),
        Sampler::Sqrt(i) => map(i, ctx, out, volume, sqrt),
        Sampler::LeakyRelu(i, f) => map(i, ctx, out, volume, |v| leaky_relu(*f, v)),
        Sampler::Reciprocal(i) => map(i, ctx, out, volume, |v| 1.0 / v),
        Sampler::Negate(i) => map(i, ctx, out, volume, |v| -v),
        Sampler::Squeeze(i) => map(i, ctx, out, volume, squeeze),
        Sampler::Log(i) => map(i, ctx, out, volume, |v| f64::from(v).ln() as f32),
        Sampler::Sign(i) => map(i, ctx, out, volume, java_signum),
        Sampler::RoundInteger(kind, i) => {
            map(i, ctx, out, volume, |v| round_to_integer(v, *kind));
        }
        Sampler::Round(kind, i, m) => {
            sample_volume(i, ctx, out, volume);
            let mb = scratch(ctx, m);
            for (o, mult) in out.iter_mut().zip(&mb) {
                *o = round_apply(*kind, *o, *mult);
            }
        }
        Sampler::Add(..) | Sampler::Sub(..) | Sampler::Mul(..) | Sampler::Div(..) => {
            let (l, r) = binary_children(s);
            sample_volume(l, ctx, out, volume);
            let rb = scratch(ctx, r);
            for (o, rv) in out.iter_mut().zip(&rb) {
                *o = match s {
                    Sampler::Add(..) => *o + rv,
                    Sampler::Sub(..) => *o + -rv,
                    Sampler::Mul(..) => *o * rv,
                    _ => *o / rv,
                };
            }
        }
        Sampler::ConstAdd(l, c) => map(l, ctx, out, volume, |v| v + c),
        Sampler::ConstSub(c, r) => map(r, ctx, out, volume, |v| c - v),
        Sampler::ConstMul(l, c) => map(l, ctx, out, volume, |v| v * c),
        Sampler::ConstDiv(c, r) => map(r, ctx, out, volume, |v| c / v),
        Sampler::Min(l, r, _) => {
            sample_volume(l, ctx, out, volume);
            let rb = scratch(ctx, r);
            for (o, rv) in out.iter_mut().zip(&rb) {
                if *rv < *o {
                    *o = *rv;
                }
            }
        }
        Sampler::ConstMin(l, c) => map(l, ctx, out, volume, |v| if *c < v { *c } else { v }),
        Sampler::Max(l, r, _) => {
            sample_volume(l, ctx, out, volume);
            let rb = scratch(ctx, r);
            for (o, rv) in out.iter_mut().zip(&rb) {
                if *rv > *o {
                    *o = *rv;
                }
            }
        }
        Sampler::ConstMax(l, c) => map(l, ctx, out, volume, |v| if *c > v { *c } else { v }),
        Sampler::PowConstBase(base, e) => {
            map(e, ctx, out, volume, |v| base.powf(f64::from(v)) as f32);
        }
        Sampler::PowConstExponent(b, exponent) => {
            map(b, ctx, out, volume, |v| f64::from(v).powf(*exponent) as f32);
        }
        Sampler::Pow(b, e) => {
            sample_volume(b, ctx, out, volume);
            let eb = scratch(ctx, e);
            for (o, ev) in out.iter_mut().zip(&eb) {
                *o = f64::from(*o).powf(f64::from(*ev)) as f32;
            }
        }
        Sampler::Spline { root, coordinates } => {
            let mut buffers: Vec<Option<Vec<f32>>> = vec![None; coordinates.len()];
            for (i, slot) in out[..size].iter_mut().enumerate() {
                *slot = spline_sample(root, &mut |index| {
                    if buffers[index].is_none() {
                        let mut buffer = vec![0.0f32; size];
                        sample_volume(&coordinates[index], ctx, &mut buffer, volume);
                        buffers[index] = Some(buffer);
                    }
                    buffers[index].as_ref().expect("sampled")[i]
                });
            }
        }
        Sampler::LerpConstFirst(alpha, first, second) => {
            sample_volume(alpha, ctx, out, volume);
            let sb = scratch(ctx, second);
            for (o, sv) in out.iter_mut().zip(&sb) {
                let a = *o;
                *o = if a == 0.0 {
                    *first
                } else if a == 1.0 {
                    *sv
                } else {
                    lerp(a, *first, *sv)
                };
            }
        }
        Sampler::LerpConstSecond(alpha, first, second) => {
            sample_volume(alpha, ctx, out, volume);
            let fb = scratch(ctx, first);
            for (o, fv) in out.iter_mut().zip(&fb) {
                let a = *o;
                *o = if a == 0.0 {
                    *fv
                } else if a == 1.0 {
                    *second
                } else {
                    lerp(a, *fv, *second)
                };
            }
        }
        Sampler::Lerp(alpha, first, second) => {
            sample_volume(alpha, ctx, out, volume);
            let fb = scratch(ctx, first);
            let sb = scratch(ctx, second);
            for i in 0..size {
                let a = out[i];
                out[i] = if a == 0.0 {
                    fb[i]
                } else if a == 1.0 {
                    sb[i]
                } else {
                    lerp(a, fb[i], sb[i])
                };
            }
        }
        Sampler::Clamp(i, min, max) => map(i, ctx, out, volume, |v| clamp(v, *min, *max)),
        Sampler::RangeChoiceConst {
            input,
            min_inclusive,
            max_exclusive,
            when_in_range,
            when_out_of_range,
        } => map(input, ctx, out, volume, |v| {
            if v >= *min_inclusive && v < *max_exclusive {
                *when_in_range
            } else {
                *when_out_of_range
            }
        }),
        Sampler::RangeChoice {
            input,
            min_inclusive,
            max_exclusive,
            when_in_range,
            when_out_of_range,
        } => {
            sample_volume(when_in_range, ctx, out, volume);
            let ib = scratch(ctx, input);
            let ob = scratch(ctx, when_out_of_range);
            for i in 0..size {
                let v = ib[i];
                if !(*min_inclusive..*max_exclusive).contains(&v) {
                    out[i] = ob[i];
                }
            }
        }
        Sampler::IntervalSelect {
            input,
            thresholds,
            samplers,
        } => {
            sample_volume(input, ctx, out, volume);
            let buffers: Vec<Vec<f32>> = samplers.iter().map(|c| scratch(ctx, c)).collect();
            for (i, slot) in out[..size].iter_mut().enumerate() {
                let index = select_index(thresholds, *slot, samplers.len());
                *slot = buffers[index][i];
            }
        }
        Sampler::IntervalSelectSingle {
            input,
            threshold,
            if_below,
            if_above,
        } => {
            sample_volume(input, ctx, out, volume);
            let bb = scratch(ctx, if_below);
            let ab = scratch(ctx, if_above);
            for i in 0..size {
                out[i] = if out[i] < *threshold { bb[i] } else { ab[i] };
            }
        }
        Sampler::Caching { id, input } => sample_volume_cached(*id, input, ctx, out, volume),
        Sampler::ContextBound { key, fallback } => match (key, ctx.beardifier) {
            (SimpleType::Beardifier, Some(b)) => beardifier_volume(b, out, volume),
            _ => out.fill(*fallback),
        },
        Sampler::BlendDensity(i) => sample_volume(i, ctx, out, volume),
        Sampler::Interpolated {
            input,
            cell_size_xz,
            cell_size_y,
            cell_size_xz_inv,
            cell_size_y_inv,
        } => interpolated_volume(
            input,
            *cell_size_xz,
            *cell_size_y,
            *cell_size_xz_inv,
            *cell_size_y_inv,
            ctx,
            out,
            volume,
        ),
        Sampler::SliceX(i, sx) => {
            if volume.size_x == 1 && volume.min_block_x == *sx {
                sample_volume(i, ctx, out, volume);
            } else {
                let iv = DensityVolume::stepped(
                    [1, volume.size_y, volume.size_z],
                    [*sx, volume.min_block_y, volume.min_block_z],
                    [
                        volume.step_block_x,
                        volume.step_block_y,
                        volume.step_block_z,
                    ],
                );
                let mut ib = vec![0.0f32; iv.size()];
                sample_volume(i, ctx, &mut ib, &iv);
                let mut index = 0;
                for zi in 0..volume.size_z {
                    for _ in 0..volume.size_x {
                        for yi in 0..volume.size_y {
                            out[index] = ib[iv.index_unchecked(0, yi, zi)];
                            index += 1;
                        }
                    }
                }
            }
        }
        Sampler::SliceXz(i, sx, sz) => {
            if volume.size_x == 1
                && volume.size_z == 1
                && volume.min_block_x == *sx
                && volume.min_block_z == *sz
            {
                sample_volume(i, ctx, out, volume);
            } else {
                let iv = DensityVolume::stepped(
                    [1, volume.size_y, 1],
                    [*sx, volume.min_block_y, *sz],
                    [
                        volume.step_block_x,
                        volume.step_block_y,
                        volume.step_block_z,
                    ],
                );
                let mut ib = vec![0.0f32; iv.size()];
                sample_volume(i, ctx, &mut ib, &iv);
                for yi in 0..volume.size_y {
                    let input = ib[iv.index_unchecked(0, yi, 0)];
                    let mut index = volume.index_unchecked(0, yi, 0);
                    for _ in 0..volume.size_z {
                        for _ in 0..volume.size_x {
                            out[index] = input;
                            index += volume.size_y as usize;
                        }
                    }
                }
            }
        }
        Sampler::SliceY(i, sy) => {
            if volume.size_y == 1 && volume.min_block_y == *sy {
                sample_volume(i, ctx, out, volume);
            } else {
                let iv = DensityVolume::stepped(
                    [volume.size_x, 1, volume.size_z],
                    [volume.min_block_x, *sy, volume.min_block_z],
                    [
                        volume.step_block_x,
                        volume.step_block_y,
                        volume.step_block_z,
                    ],
                );
                let mut ib = vec![0.0f32; iv.size()];
                sample_volume(i, ctx, &mut ib, &iv);
                for zi in 0..volume.size_z {
                    for xi in 0..volume.size_x {
                        let input = ib[iv.index_unchecked(xi, 0, zi)];
                        let start = volume.index_unchecked(xi, 0, zi);
                        out[start..start + volume.size_y as usize].fill(input);
                    }
                }
            }
        }
        Sampler::SliceZ(i, sz) => {
            if volume.size_z == 1 && volume.min_block_z == *sz {
                sample_volume(i, ctx, out, volume);
            } else {
                let iv = DensityVolume::stepped(
                    [volume.size_x, volume.size_y, 1],
                    [volume.min_block_x, volume.min_block_y, *sz],
                    [
                        volume.step_block_x,
                        volume.step_block_y,
                        volume.step_block_z,
                    ],
                );
                let mut ib = vec![0.0f32; iv.size()];
                sample_volume(i, ctx, &mut ib, &iv);
                let mut index = 0;
                for _ in 0..volume.size_z {
                    for xi in 0..volume.size_x {
                        for yi in 0..volume.size_y {
                            out[index] = ib[iv.index_unchecked(xi, yi, 0)];
                            index += 1;
                        }
                    }
                }
            }
        }
        Sampler::FindTopSurface {
            density,
            upper_bound,
            lower_bound,
            cell_height,
        } => {
            assert!(
                volume.size_y == 1,
                "Cannot sample with sizeY={}",
                volume.size_y
            );
            sample_volume(upper_bound, ctx, out, volume);
            let mut index = 0;
            for zi in 0..volume.size_z {
                let bz = volume.block_z(zi);
                for xi in 0..volume.size_x {
                    let bx = volume.block_x(xi);
                    let upper = out[index];
                    out[index] =
                        find_surface_from(density, *lower_bound, *cell_height, ctx, bx, bz, upper)
                            as f32;
                    index += 1;
                }
            }
        }
    }
}

fn map(
    input: &S,
    ctx: &mut SamplerContext<'_>,
    out: &mut [f32],
    volume: &DensityVolume,
    f: impl Fn(f32) -> f32,
) {
    sample_volume(input, ctx, out, volume);
    for v in out.iter_mut() {
        *v = f(*v);
    }
}

pub fn beardifier_volume(beardifier: &Beardifier, out: &mut [f32], volume: &DensityVolume) {
    out.fill(0.0);
    let Some(affected) = beardifier.affected_box() else {
        return;
    };
    let volume_box = BoundingBox::new(
        volume.min_block_x,
        volume.min_block_y,
        volume.min_block_z,
        volume.max_block_x(),
        volume.max_block_y(),
        volume.max_block_z(),
    );
    if !affected.intersects(volume_box) {
        return;
    }
    let min_x = 0
        .max(affected.min_x() - volume.min_block_x)
        .div_euclid(volume.step_block_x);
    let min_y = 0
        .max(affected.min_y() - volume.min_block_y)
        .div_euclid(volume.step_block_y);
    let min_z = 0
        .max(affected.min_z() - volume.min_block_z)
        .div_euclid(volume.step_block_z);
    let max_x = (volume.size_x - 1)
        .min((affected.max_x() - volume.min_block_x).div_euclid(volume.step_block_x));
    let max_y = (volume.size_y - 1)
        .min((affected.max_y() - volume.min_block_y).div_euclid(volume.step_block_y));
    let max_z = (volume.size_z - 1)
        .min((affected.max_z() - volume.min_block_z).div_euclid(volume.step_block_z));
    for zi in min_z..=max_z {
        let bz = volume.block_z(zi);
        for xi in min_x..=max_x {
            let bx = volume.block_x(xi);
            for yi in min_y..=max_y {
                out[volume.index_unchecked(xi, yi, zi)] =
                    beardifier.sample_value_unchecked(bx, volume.block_y(yi), bz);
            }
        }
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "mirrors InterpolatedFunction.Sampler fields"
)]
fn interpolated_volume(
    input: &S,
    cell_size_xz: i32,
    cell_size_y: i32,
    cell_size_xz_inv: f32,
    cell_size_y_inv: f32,
    ctx: &mut SamplerContext<'_>,
    out: &mut [f32],
    volume: &DensityVolume,
) {
    if (volume.step_block_x == cell_size_xz || volume.size_x == 1)
        && (volume.step_block_y == cell_size_y || volume.size_y == 1)
        && (volume.step_block_z == cell_size_xz || volume.size_z == 1)
        && volume.min_block_x.rem_euclid(cell_size_xz) == 0
        && volume.min_block_y.rem_euclid(cell_size_y) == 0
        && volume.min_block_z.rem_euclid(cell_size_xz) == 0
    {
        sample_volume(input, ctx, out, volume);
    } else if volume.step_block_x == 1 && volume.step_block_y == 1 && volume.step_block_z == 1 {
        interpolated_block_step(
            input,
            cell_size_xz,
            cell_size_y,
            cell_size_xz_inv,
            cell_size_y_inv,
            ctx,
            out,
            volume,
        );
    } else {
        let block_volume = DensityVolume::new(
            [
                volume.size_x * volume.step_block_x,
                volume.size_y * volume.step_block_y,
                volume.size_z * volume.step_block_z,
            ],
            [volume.min_block_x, volume.min_block_y, volume.min_block_z],
        );
        let mut block_buffer = vec![0.0f32; block_volume.size()];
        interpolated_block_step(
            input,
            cell_size_xz,
            cell_size_y,
            cell_size_xz_inv,
            cell_size_y_inv,
            ctx,
            &mut block_buffer,
            &block_volume,
        );
        for zi in 0..volume.size_z {
            for xi in 0..volume.size_x {
                for yi in 0..volume.size_y {
                    out[volume.index_unchecked(xi, yi, zi)] = block_buffer[block_volume
                        .index_unchecked(
                            xi * volume.step_block_x,
                            yi * volume.step_block_y,
                            zi * volume.step_block_z,
                        )];
                }
            }
        }
    }
}

pub fn interpolation_cell_volume(
    cell_size_xz: i32,
    cell_size_y: i32,
    volume: &DensityVolume,
) -> (DensityVolume, [i32; 3]) {
    let min_cell_x = volume.min_block_x.div_euclid(cell_size_xz);
    let min_cell_y = volume.min_block_y.div_euclid(cell_size_y);
    let min_cell_z = volume.min_block_z.div_euclid(cell_size_xz);
    let count_x = volume.max_block_x().div_euclid(cell_size_xz) - min_cell_x + 1;
    let count_y = volume.max_block_y().div_euclid(cell_size_y) - min_cell_y + 1;
    let count_z = volume.max_block_z().div_euclid(cell_size_xz) - min_cell_z + 1;
    let extend = |max: i32, cell: i32, count: i32| {
        if max.rem_euclid(cell) == 0 {
            count
        } else {
            count + 1
        }
    };
    (
        DensityVolume::stepped(
            [
                extend(volume.max_block_x(), cell_size_xz, count_x),
                extend(volume.max_block_y(), cell_size_y, count_y),
                extend(volume.max_block_z(), cell_size_xz, count_z),
            ],
            [
                min_cell_x * cell_size_xz,
                min_cell_y * cell_size_y,
                min_cell_z * cell_size_xz,
            ],
            [cell_size_xz, cell_size_y, cell_size_xz],
        ),
        [count_x, count_y, count_z],
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "mirrors InterpolatedFunction.Sampler fields"
)]
fn interpolated_block_step(
    input: &S,
    cell_size_xz: i32,
    cell_size_y: i32,
    cell_size_xz_inv: f32,
    cell_size_y_inv: f32,
    ctx: &mut SamplerContext<'_>,
    out: &mut [f32],
    volume: &DensityVolume,
) {
    let (cell_volume, counts) = interpolation_cell_volume(cell_size_xz, cell_size_y, volume);
    let mut cells = vec![0.0f32; cell_volume.size()];
    sample_volume(input, ctx, &mut cells, &cell_volume);
    fill_interpolated(
        &cells,
        &cell_volume,
        counts,
        cell_size_xz,
        cell_size_y,
        cell_size_xz_inv,
        cell_size_y_inv,
        out,
        volume,
    );
}

#[expect(
    clippy::too_many_arguments,
    reason = "mirrors InterpolatedFunction.Sampler fields"
)]
pub fn fill_interpolated(
    cells: &[f32],
    cell_volume: &DensityVolume,
    counts: [i32; 3],
    cell_size_xz: i32,
    cell_size_y: i32,
    cell_size_xz_inv: f32,
    cell_size_y_inv: f32,
    out: &mut [f32],
    volume: &DensityVolume,
) {
    for cz in 0..counts[2] {
        let next_z = (cz + 1).min(cell_volume.size_z - 1);
        for cx in 0..counts[0] {
            let next_x = (cx + 1).min(cell_volume.size_x - 1);
            let mut v000 = cells[cell_volume.index_unchecked(cx, 0, cz)];
            let mut v100 = cells[cell_volume.index_unchecked(next_x, 0, cz)];
            let mut v001 = cells[cell_volume.index_unchecked(cx, 0, next_z)];
            let mut v101 = cells[cell_volume.index_unchecked(next_x, 0, next_z)];
            for cy in 0..counts[1] {
                let next_y = (cy + 1).min(cell_volume.size_y - 1);
                let v010 = cells[cell_volume.index_unchecked(cx, next_y, cz)];
                let v110 = cells[cell_volume.index_unchecked(next_x, next_y, cz)];
                let v011 = cells[cell_volume.index_unchecked(cx, next_y, next_z)];
                let v111 = cells[cell_volume.index_unchecked(next_x, next_y, next_z)];
                let cell_out_x = cell_volume.block_x(cx) - volume.min_block_x;
                let cell_out_y = cell_volume.block_y(cy) - volume.min_block_y;
                let cell_out_z = cell_volume.block_z(cz) - volume.min_block_z;
                let x0 = 0.max(-cell_out_x);
                let y0 = 0.max(-cell_out_y);
                let z0 = 0.max(-cell_out_z);
                let x1 = cell_size_xz.min(volume.size_x - cell_out_x) - 1;
                let y1 = cell_size_y.min(volume.size_y - cell_out_y) - 1;
                let z1 = cell_size_xz.min(volume.size_z - cell_out_z) - 1;
                for z in z0..=z1 {
                    let out_z = cell_out_z + z;
                    let alpha_z = z as f32 * cell_size_xz_inv;
                    let v00 = lerp(alpha_z, v000, v001);
                    let v01 = lerp(alpha_z, v010, v011);
                    let v10 = lerp(alpha_z, v100, v101);
                    let v11 = lerp(alpha_z, v110, v111);
                    for x in x0..=x1 {
                        let out_x = cell_out_x + x;
                        let alpha_x = x as f32 * cell_size_xz_inv;
                        let v0 = lerp(alpha_x, v00, v10);
                        let v1 = lerp(alpha_x, v01, v11);
                        let step = (v1 - v0) * cell_size_y_inv;
                        let mut value = v0 + step * y0 as f32;
                        let start = volume.index_unchecked(out_x, cell_out_y + y0, out_z);
                        let count = (y1 - y0 + 1) as usize;
                        for slot in &mut out[start..start + count] {
                            *slot = value;
                            value += step;
                        }
                    }
                }
                v000 = v010;
                v100 = v110;
                v001 = v011;
                v101 = v111;
            }
        }
    }
}
