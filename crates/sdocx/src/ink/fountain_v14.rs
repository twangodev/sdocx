//! Saved GLV14 stylus geometry. Shares SmPath and prepared dots with V16;
//! V14 filters samples by distance and has no width-history smoothing pass.
use super::{
    fountain::Dots,
    path::{P, Quad},
};
use crate::{Point, Stroke};

pub(super) fn prepare(s: &Stroke, tolerance: f32) -> Dots {
    let size = s.pen_width;
    let mut previous = P::from(s.points[0]);
    let mut midpoint = previous;
    let mut width = (size * 0.5) * (s.pressures[0] as f32).min(1.);
    let mut residual = 0.;
    let mut alternate = false;
    let mut ratios = [0.; 3];
    let mut ratio_count = 0;
    let mut dots = Dots {
        points: vec![],
        radii: vec![],
        sample_ends: vec![0; s.points.len()],
    };
    const REPEAT: f32 = 0.82;
    for i in 1..s.points.len() {
        let p = P::from(s.points[i]);
        let delta = p.sub(previous);
        let distance = delta.len();
        if distance >= tolerance {
            alternate = if distance < 5. { !alternate } else { true };
            if alternate {
                // At zero tolerance native admits zero distance. NaN ratios
                // resolve through the pressure/size floor below.
                let ratio = delta.y / distance;
                ratios[ratio_count % 3] = ratio;
                if ratio_count == 0 {
                    ratios.fill(ratio);
                }
                ratio_count += 1;
                let ratio = ((ratios[0] + ratios[1]) + ratios[2]) / 3.;
                let pressure = (s.pressures[i] as f32).min(1.);
                let tilt = s.tilts.get(i).copied().unwrap_or(0.) as f32;
                let degrees = ((tilt * 180.) as f64 / std::f64::consts::PI) as f32;
                let tilt = (degrees.min(75.) - 15.).max(0.) / 60. * 3.;
                let raw = size / 3. + (tilt.mul_add(0.5, (pressure + pressure) * 0.5) * size) * 0.5;
                let target = (((ratio * raw) as f64).mul_add(0.8, raw as f64) * 0.5) as f32;
                let step = size / if ratio > 0. { 4. } else { 2. };
                let limited = if (width - target).abs() > step {
                    width + if width > target { -step } else { step }
                } else {
                    target
                };
                let target = ((pressure * 0.7) * size).max(limited).max(size / 3.);
                let next_mid = previous.mid(p);
                let q = Quad::new(midpoint, previous, next_mid);
                if q.length > 0. && q.length >= residual {
                    let count = ((q.length - residual) / REPEAT) as usize + 1;
                    let increment = (target - width) / count as f32;
                    let mut d = residual;
                    let mut w = width;
                    while d <= q.length {
                        w += increment;
                        emit(&mut dots, q.at(d), w * 0.5);
                        d += REPEAT;
                    }
                    residual = d - q.length;
                    previous = p;
                    midpoint = next_mid;
                    width = target;
                } else if q.length == 0. {
                    midpoint = next_mid;
                }
            }
        }
        dots.sample_ends[i] = dots.points.len();
    }
    let last = s.points.len() - 1;
    if dots.points.is_empty() {
        emit(&mut dots, previous, size * 0.25);
    } else {
        let p = P::from(s.points[last]);
        let q = Quad::new(midpoint, previous, p);
        let mut d = residual;
        while q.length > 0. && d <= q.length {
            emit(&mut dots, q.at(d), width * 0.5);
            d += REPEAT;
        }
        emit(&mut dots, p, width * 0.5);
    }
    dots.sample_ends[last] = dots.points.len();
    dots
}

fn emit(dots: &mut Dots, p: P, radius: f32) {
    dots.points.push(Point {
        x: p.x as f64,
        y: p.y as f64,
    });
    dots.radii.push(radius.max(0.1) as f64);
}
