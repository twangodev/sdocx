//! Shared float32 point and SmPath quadratic measurement.
//!
//! `SmPath::helper_compute_quad_segs` uses a 15-bit parameter and a 0.5-unit
//! midpoint deviation threshold. Length is the sum of subdivision chords.
//! `getPosTan` evaluates the original quadratic at the interpolated parameter.
//! Its parameter scale is the stored float `0x380000fd`, not `1.0 / 32767.0`.
use crate::Point;

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct P {
    pub(super) x: f32,
    pub(super) y: f32,
}

impl P {
    pub(super) fn mid(self, b: Self) -> Self {
        Self {
            x: (self.x + b.x) * 0.5,
            y: (self.y + b.y) * 0.5,
        }
    }

    pub(super) fn sub(self, b: Self) -> Self {
        Self {
            x: self.x - b.x,
            y: self.y - b.y,
        }
    }

    pub(super) fn dot(self, b: Self) -> f32 {
        self.x.mul_add(b.x, self.y * b.y)
    }

    pub(super) fn len(self) -> f32 {
        self.dot(self).sqrt()
    }

    pub(super) fn lerp(self, b: Self, t: f32) -> Self {
        Self {
            x: (b.x - self.x).mul_add(t, self.x),
            y: (b.y - self.y).mul_add(t, self.y),
        }
    }
}

impl From<Point> for P {
    fn from(p: Point) -> Self {
        Self {
            x: p.x as f32,
            y: p.y as f32,
        }
    }
}

pub(super) struct Quad {
    p: [P; 3],
    segments: Vec<(f32, u16)>,
    pub(super) length: f32,
}

impl Quad {
    pub(super) fn new(a: P, b: P, c: P) -> Self {
        let mut q = Self {
            p: [a, b, c],
            segments: Vec::new(),
            length: 0.,
        };
        q.split([a, b, c], 0, 32767);
        q
    }

    fn split(&mut self, p: [P; 3], t0: u16, t1: u16) {
        let mid = p[0].mid(p[2]);
        let dx = p[1].x.mul_add(0.5, mid.x * -0.5).abs();
        let dy = p[1].y.mul_add(0.5, mid.y * -0.5).abs();
        if t1 - t0 >= 1024 && dx.max(dy) > 0.5 {
            let a = p[0].mid(p[1]);
            let b = p[1].mid(p[2]);
            let c = a.mid(b);
            let t = (t0 + t1) / 2;
            self.split([p[0], a, c], t0, t);
            self.split([c, b, p[2]], t, t1);
        } else {
            let next = self.length + p[0].sub(p[2]).len();
            if next > self.length {
                self.length = next;
                self.segments.push((next, t1));
            }
        }
    }

    pub(super) fn at(&self, d: f32) -> P {
        let i = self
            .segments
            .partition_point(|&(end, _)| end < d)
            .min(self.segments.len() - 1);
        let (start, t0) = if i == 0 {
            (0., 0)
        } else {
            self.segments[i - 1]
        };
        let (end, t1) = self.segments[i];
        let scale = f32::from_bits(0x3800_00fd);
        let t0 = t0 as f32 * scale;
        let t1 = t1 as f32 * scale;
        let t = t0 + (d - start) * (t1 - t0) / (end - start);
        self.p[0]
            .lerp(self.p[1], t)
            .lerp(self.p[1].lerp(self.p[2], t), t)
    }
}
