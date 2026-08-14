use super::{PARAMETER_COUNT, QUANTIZATION_FACTOR, quantize_coord};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetPoint {
    pub temperature: i64,
    pub humidity: i64,
    pub continentalness: i64,
    pub erosion: i64,
    pub depth: i64,
    pub weirdness: i64,
}

impl TargetPoint {
    pub const fn new(
        temperature: i64,
        humidity: i64,
        continentalness: i64,
        erosion: i64,
        depth: i64,
        weirdness: i64,
    ) -> Self {
        Self {
            temperature,
            humidity,
            continentalness,
            erosion,
            depth,
            weirdness,
        }
    }

    pub fn from_floats(
        temperature: f64,
        humidity: f64,
        continentalness: f64,
        erosion: f64,
        depth: f64,
        weirdness: f64,
    ) -> Self {
        Self {
            temperature: quantize_coord(temperature),
            humidity: quantize_coord(humidity),
            continentalness: quantize_coord(continentalness),
            erosion: quantize_coord(erosion),
            depth: quantize_coord(depth),
            weirdness: quantize_coord(weirdness),
        }
    }

    pub const fn to_parameter_array(self) -> [i64; PARAMETER_COUNT] {
        [
            self.temperature,
            self.humidity,
            self.continentalness,
            self.erosion,
            self.depth,
            self.weirdness,
            0,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parameter {
    pub min: i64,
    pub max: i64,
}

impl Parameter {
    pub const fn new(min: i64, max: i64) -> Self {
        Self { min, max }
    }

    pub fn point(value: f32) -> Self {
        Self::span(value, value)
    }

    pub fn span(min: f32, max: f32) -> Self {
        debug_assert!(min <= max, "min > max: {min} > {max}");
        Self {
            min: (min * QUANTIZATION_FACTOR) as i64,
            max: (max * QUANTIZATION_FACTOR) as i64,
        }
    }

    pub const fn span_params(min: &Parameter, max: &Parameter) -> Self {
        debug_assert!(min.min <= max.max, "span_params: min > max");
        Self {
            min: min.min,
            max: max.max,
        }
    }

    #[inline]
    pub const fn distance(&self, target: i64) -> i64 {
        let above = target - self.max;
        let below = self.min - target;
        if above > 0 {
            above
        } else if below > 0 {
            below
        } else {
            0
        }
    }

    pub const fn span_with(&self, other: Option<&Parameter>) -> Self {
        match other {
            Some(o) => Self {
                min: self.min.min(o.min),
                max: self.max.max(o.max),
            },
            None => *self,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ParameterPoint {
    pub temperature: Parameter,
    pub humidity: Parameter,
    pub continentalness: Parameter,
    pub erosion: Parameter,
    pub depth: Parameter,
    pub weirdness: Parameter,
    pub offset: i64,
}

impl ParameterPoint {
    pub const fn new(
        temperature: Parameter,
        humidity: Parameter,
        continentalness: Parameter,
        erosion: Parameter,
        depth: Parameter,
        weirdness: Parameter,
        offset: i64,
    ) -> Self {
        Self {
            temperature,
            humidity,
            continentalness,
            erosion,
            depth,
            weirdness,
            offset,
        }
    }

    pub const fn fitness(&self, target: &TargetPoint) -> i64 {
        let t = self.temperature.distance(target.temperature);
        let h = self.humidity.distance(target.humidity);
        let c = self.continentalness.distance(target.continentalness);
        let e = self.erosion.distance(target.erosion);
        let d = self.depth.distance(target.depth);
        let w = self.weirdness.distance(target.weirdness);

        t * t + h * h + c * c + e * e + d * d + w * w + self.offset * self.offset
    }

    pub const fn parameter_space(&self) -> [Parameter; PARAMETER_COUNT] {
        [
            self.temperature,
            self.humidity,
            self.continentalness,
            self.erosion,
            self.depth,
            self.weirdness,
            Parameter::new(self.offset, self.offset),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_point_from_floats() {
        let target = TargetPoint::from_floats(0.5, -0.3, 0.0, 0.1, 0.0, 0.2);
        assert_eq!(target.temperature, 5000);
        assert_eq!(target.humidity, -3000);
        assert_eq!(target.continentalness, 0);
        assert_eq!(target.erosion, 1000);
        assert_eq!(target.depth, 0);
        assert_eq!(target.weirdness, 2000);
    }

    #[test]
    fn test_parameter_distance() {
        let param = Parameter::new(-5000, 5000);

        assert_eq!(param.distance(0), 0);
        assert_eq!(param.distance(5000), 0);
        assert_eq!(param.distance(-5000), 0);

        assert_eq!(param.distance(6000), 1000);
        assert_eq!(param.distance(-6000), 1000);
        assert_eq!(param.distance(10000), 5000);
    }

    #[test]
    fn test_parameter_point_fitness() {
        let params = ParameterPoint::new(
            Parameter::new(0, 0),
            Parameter::new(0, 0),
            Parameter::new(0, 0),
            Parameter::new(0, 0),
            Parameter::new(0, 0),
            Parameter::new(0, 0),
            0,
        );

        let target = TargetPoint::new(0, 0, 0, 0, 0, 0);
        assert_eq!(params.fitness(&target), 0);

        let target = TargetPoint::new(100, 0, 0, 0, 0, 0);
        assert_eq!(params.fitness(&target), 100 * 100);

        let target = TargetPoint::new(100, 100, 0, 0, 0, 0);
        assert_eq!(params.fitness(&target), 100 * 100 + 100 * 100);
    }
}
