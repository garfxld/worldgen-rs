mod parameter_list;
pub(crate) mod types;

pub use parameter_list::ParameterList;
pub use types::{Parameter, ParameterPoint, TargetPoint};

pub const QUANTIZATION_FACTOR: f32 = 10000.0;

pub const PARAMETER_COUNT: usize = 7;

#[inline]
pub fn quantize_coord(coord: f64) -> i64 {
    ((coord as f32) * QUANTIZATION_FACTOR) as i64
}

#[inline]
pub fn unquantize_coord(coord: i64) -> f32 {
    coord as f32 / QUANTIZATION_FACTOR
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantize_coord() {
        assert_eq!(quantize_coord(0.0), 0);
        assert_eq!(quantize_coord(1.0), 10000);
        assert_eq!(quantize_coord(-1.0), -10000);
        assert_eq!(quantize_coord(0.5), 5000);
        assert_eq!(quantize_coord(-0.5), -5000);
    }

    #[test]
    fn test_unquantize_coord() {
        assert!((unquantize_coord(0) - 0.0).abs() < 1e-6);
        assert!((unquantize_coord(10000) - 1.0).abs() < 1e-6);
        assert!((unquantize_coord(-10000) - -1.0).abs() < 1e-6);
    }

    #[test]
    fn test_quantize_roundtrip() {
        let values = [0.0, 0.5, -0.5, 1.0, -1.0, 0.123, -0.456];
        for v in values {
            let quantized = quantize_coord(v);
            let unquantized = unquantize_coord(quantized);
            assert!(
                (v as f32 - unquantized).abs() < 0.0001,
                "Roundtrip failed for {v}: got {unquantized}",
            );
        }
    }
}
