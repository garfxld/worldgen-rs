pub struct MarsagliaPolarGaussian {
    cached: Option<f64>,
}

impl Default for MarsagliaPolarGaussian {
    fn default() -> Self {
        Self::new()
    }
}

impl MarsagliaPolarGaussian {
    pub const fn new() -> Self {
        Self { cached: None }
    }

    pub fn next(&mut self, mut next_f64: impl FnMut() -> f64) -> f64 {
        if let Some(value) = self.cached.take() {
            return value;
        }
        loop {
            let d = 2.0 * next_f64() - 1.0;
            let e = 2.0 * next_f64() - 1.0;
            let f = d * d + e * e;

            if f < 1.0 && f != 0.0 {
                let g = (-2.0 * f.ln() / f).sqrt();
                self.cached = Some(e * g);
                return d * g;
            }
        }
    }
}
