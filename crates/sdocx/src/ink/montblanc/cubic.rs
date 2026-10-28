use crate::ink::path::P;
/// SmPath cubic representation selected by endPen's retained path flag.
/// Subdivision and parameter interpolation mirror the pinned native cubic
/// helper and the existing GLV1 cubic model.
pub(in crate::ink::montblanc) struct Cubic {
    p: [P; 4],
    segments: Vec<(f32, u16)>,
    pub(in crate::ink::montblanc) length: f32,
}

impl Cubic {
    pub(in crate::ink::montblanc) fn new(p: [P; 4]) -> Self {
        let mut path = Self {
            p,
            segments: Vec::new(),
            length: 0.0,
        };
        path.split(p, 0, 32767);
        path
    }

    fn split(&mut self, p: [P; 4], t0: u16, t1: u16) {
        let line = |a: P, b: P, t: f32| P {
            x: (b.x - a.x).mul_add(t, a.x),
            y: (b.y - a.y).mul_add(t, a.y),
        };
        let one = line(p[0], p[3], f32::from_bits(0x3eaa_aaab));
        let two = line(p[0], p[3], f32::from_bits(0x3f2a_aaab));
        let dev1 = (p[1].x - one.x).abs().max((p[1].y - one.y).abs());
        let dev2 = (p[2].x - two.x).abs().max((p[2].y - two.y).abs());
        if t1 - t0 >= 1024 && (dev1 > 0.5 || dev2 > 0.5) {
            let a = p[0].mid(p[1]);
            let b = p[1].mid(p[2]);
            let c = p[2].mid(p[3]);
            let d = a.mid(b);
            let e = b.mid(c);
            let f = d.mid(e);
            let t = (t0 + t1) / 2;
            self.split([p[0], a, d, f], t0, t);
            self.split([f, e, c, p[3]], t, t1);
        } else {
            let next = self.length + p[0].sub(p[3]).len();
            if next > self.length {
                self.length = next;
                self.segments.push((next, t1));
            }
        }
    }

    fn parameter(&self, distance: f32) -> f32 {
        let index = self
            .segments
            .partition_point(|&(end, _)| end < distance)
            .min(self.segments.len() - 1);
        let (start, t0) = if index == 0 {
            (0.0, 0)
        } else {
            self.segments[index - 1]
        };
        let (end, t1) = self.segments[index];
        let scale = f32::from_bits(0x3800_00fd);
        let t0 = t0 as f32 * scale;
        let t1 = t1 as f32 * scale;
        t0 + (distance - start) * (t1 - t0) / (end - start)
    }

    pub(in crate::ink::montblanc) fn at(&self, distance: f32) -> P {
        let t = self.parameter(distance);
        let eval = |a: f32, b: f32, c: f32, d: f32| {
            if t == 0.0 {
                return a;
            }
            let x = (b - a).mul_add(t, a);
            let y = (c - b).mul_add(t, b);
            let z = (d - c).mul_add(t, c);
            let q = (y - x).mul_add(t, x);
            let r = (z - y).mul_add(t, y);
            (r - q).mul_add(t, q)
        };
        P {
            x: eval(self.p[0].x, self.p[1].x, self.p[2].x, self.p[3].x),
            y: eval(self.p[0].y, self.p[1].y, self.p[2].y, self.p[3].y),
        }
    }

    pub(in crate::ink::montblanc) fn normalized_tangent(&self, distance: f32) -> P {
        let t = self.parameter(distance);
        // Native cubic getPosTan uses its expanded derivative polynomial,
        // without the common factor three.
        let component = |a: f32, b: f32, c: f32, d: f32| {
            let first = b - a;
            let linear = ((-2.0_f32).mul_add(b, c) + a) * 2.0;
            let quadratic = 3.0_f32.mul_add(b - c, d) - a;
            let curvature = quadratic.mul_add(t, linear);
            curvature.mul_add(t, first)
        };
        let mut tangent = P {
            x: component(self.p[0].x, self.p[1].x, self.p[2].x, self.p[3].x),
            y: component(self.p[0].y, self.p[1].y, self.p[2].y, self.p[3].y),
        };
        let squared = tangent.x.mul_add(tangent.x, tangent.y * tangent.y);
        if squared > 2.0_f32.powi(-24) {
            let inverse = 1.0_f32 / squared.sqrt();
            tangent.x *= inverse;
            tangent.y *= inverse;
        }
        tangent
    }
}
