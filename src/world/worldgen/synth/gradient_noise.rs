use crate::util::rng::RandomSource;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gradient {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl Gradient {
    const fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    pub fn dot_f64(self, x: f64, y: f64, z: f64) -> f64 {
        f64::from(self.x) * x + f64::from(self.y) * y + f64::from(self.z) * z
    }

    pub fn dot(self, x: f32, y: f32, z: f32) -> f32 {
        self.x as f32 * x + self.y as f32 * y + self.z as f32 * z
    }

    pub fn dot_xz(self, x: f32, z: f32) -> f32 {
        self.x as f32 * x + self.z as f32 * z
    }
}

pub const GRADIENT: [Gradient; 16] = [
    Gradient::new(1, 1, 0),
    Gradient::new(-1, 1, 0),
    Gradient::new(1, -1, 0),
    Gradient::new(-1, -1, 0),
    Gradient::new(1, 0, 1),
    Gradient::new(-1, 0, 1),
    Gradient::new(1, 0, -1),
    Gradient::new(-1, 0, -1),
    Gradient::new(0, 1, 1),
    Gradient::new(0, -1, 1),
    Gradient::new(0, 1, -1),
    Gradient::new(0, -1, -1),
    Gradient::new(1, 1, 0),
    Gradient::new(0, -1, 1),
    Gradient::new(-1, 1, 0),
    Gradient::new(0, -1, -1),
];

const ROUND_OFF: f64 = 33_554_432.0;
const HALF_ROUND_OFF: f64 = f64::from_bits(16_777_216.0_f64.to_bits() - 1);

pub fn wrap(x: f64) -> f64 {
    if (-HALF_ROUND_OFF..HALF_ROUND_OFF).contains(&x) {
        x
    } else {
        x - (x / ROUND_OFF + 0.5).floor() * ROUND_OFF
    }
}

pub const fn floor(v: f64) -> i32 {
    v.floor() as i32
}

pub fn smoothstep(x: f32) -> f32 {
    x * x * x * (x * (x * 6.0 - 15.0) + 10.0)
}

pub fn lerp(alpha: f32, p0: f32, p1: f32) -> f32 {
    p0 + alpha * (p1 - p0)
}

pub fn lerp2(a1: f32, a2: f32, x00: f32, x10: f32, x01: f32, x11: f32) -> f32 {
    lerp(a2, lerp(a1, x00, x10), lerp(a1, x01, x11))
}

#[expect(clippy::too_many_arguments, reason = "mirrors Mth.lerp3")]
pub fn lerp3(
    a1: f32,
    a2: f32,
    a3: f32,
    x000: f32,
    x100: f32,
    x010: f32,
    x110: f32,
    x001: f32,
    x101: f32,
    x011: f32,
    x111: f32,
) -> f32 {
    lerp(
        a3,
        lerp2(a1, a2, x000, x100, x010, x110),
        lerp2(a1, a2, x001, x101, x011, x111),
    )
}

#[derive(Debug, Clone)]
pub struct GradientNoise {
    pub perms: [u8; 256],
    pub offset_x: f64,
    pub offset_y: f64,
    pub offset_z: f64,
}

impl GradientNoise {
    pub fn new(random: &mut RandomSource, noise_offset_scale: f64) -> Self {
        let offset_x = random.next_f64() * noise_offset_scale;
        let offset_y = random.next_f64() * noise_offset_scale;
        let offset_z = random.next_f64() * noise_offset_scale;
        let mut perms = [0u8; 256];
        for (i, p) in perms.iter_mut().enumerate() {
            *p = i as u8;
        }
        for i in 0..256 {
            let offset = random.next_i32_bounded((256 - i) as i32) as usize;
            perms.swap(i, offset + i);
        }
        Self {
            perms,
            offset_x,
            offset_y,
            offset_z,
        }
    }

    pub const fn permute(&self, x: i32) -> i32 {
        self.perms[(x & 0xFF) as usize] as i32
    }

    pub const fn permute_to_grad(&self, x: i32) -> Gradient {
        GRADIENT[(self.permute(x) & 15) as usize]
    }
}

pub const fn grad_dot(hash: i32, x: f32, y: f32, z: f32) -> f32 {
    let g = GRADIENT[(hash & 15) as usize];
    g.x as f32 * x + g.y as f32 * y + g.z as f32 * z
}
