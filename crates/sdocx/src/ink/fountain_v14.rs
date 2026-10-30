//! Saved GLV14 geometry. Shares SmPath and prepared dots with V16;
//! V14 filters samples by distance and has no width-history smoothing pass.
use super::{
    fountain::Dots,
    path::{P, Quad},
};
use crate::{Point, Stroke};

pub(super) fn prepare(s: &Stroke, size: f32, tolerance: f32) -> Dots {
    let rendering = s.rendering.as_ref().unwrap();
    let stylus = rendering.tool_type_raw == 2;
    let pressure_at = |i| {
        if stylus {
            (s.pressures[i] as f32).min(1.)
        } else {
            0.5
        }
    };
    let short_move_threshold = if rendering.tool_type_raw == 1 {
        50.
    } else {
        5.
    };
    let mut previous = P::from(s.points[0]);
    let mut midpoint = previous;
    let mut width = (size * 0.5) * pressure_at(0);
    let mut residual = 0.;
    let mut alternate = false;
    let mut ratios = [0.; 3];
    let mut ratio_count = 0;
    let mut dots = Dots {
        directions: Some(vec![]),
        points: vec![],
        radii: vec![],
        sample_ends: vec![0; s.points.len()],
        rgb: None,
        rainbow_final_offset: None,
        #[cfg(test)]
        rainbow_path_offsets: Vec::new(),
    };
    const REPEAT: f32 = 0.82;
    for i in 1..s.points.len() {
        let p = P::from(s.points[i]);
        let delta = p.sub(previous);
        let distance = delta.len();
        if distance >= tolerance {
            alternate = if distance < short_move_threshold {
                !alternate
            } else {
                true
            };
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
                let pressure = pressure_at(i);
                let tilt = if stylus {
                    s.tilts.get(i).copied().unwrap_or(0.) as f32
                } else {
                    0.8
                };
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
                        emit(&mut dots, q.at(d), w * 0.5, Some(q.normalized_tangent(d)));
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
        emit(&mut dots, previous, size * 0.25, None);
    } else {
        let p = P::from(s.points[last]);
        let q = Quad::new(midpoint, previous, p);
        let mut d = residual;
        while q.length > 0. && d <= q.length {
            emit(
                &mut dots,
                q.at(d),
                width * 0.5,
                Some(q.normalized_tangent(d)),
            );
            d += REPEAT;
        }
        emit(&mut dots, p, width * 0.5, None);
    }
    dots.sample_ends[last] = dots.points.len();
    if rendering.properties.fixed_width {
        let fixed_radius = (rendering.style.fixed_width.unwrap() * 0.5).max(0.1);
        dots.radii.fill(fixed_radius as f64);
    }
    dots
}

fn emit(dots: &mut Dots, p: P, radius: f32, tangent: Option<P>) {
    let directions = dots.directions.as_mut().unwrap();
    let fallback = Point { x: 0., y: 1. };
    let direction = tangent.map_or_else(
        || directions.last().copied().unwrap_or(fallback),
        |v| {
            if v.x == 0. && v.y == 0. {
                fallback
            } else {
                Point {
                    x: v.x as f64,
                    y: v.y as f64,
                }
            }
        },
    );
    directions.push(direction);
    dots.points.push(Point {
        x: p.x as f64,
        y: p.y as f64,
    });
    dots.radii.push(radius.max(0.1) as f64);
}

pub(crate) fn addpoint_record(
    point: crate::Point,
    radius: f64,
    direction: crate::Point,
    output_scale: f32,
) -> Option<[f32; 9]> {
    let (x, y, radius, direction_x, direction_y) = (
        point.x as f32,
        point.y as f32,
        radius as f32,
        direction.x as f32,
        direction.y as f32,
    );
    if !output_scale.is_finite()
        || output_scale <= 0.0
        || !x.is_finite()
        || !y.is_finite()
        || !radius.is_finite()
        || radius <= 0.0
        || !direction_x.is_finite()
        || !direction_y.is_finite()
    {
        return None;
    }
    let inverse_scale = 1.0_f32 / output_scale;
    if !inverse_scale.is_finite() || inverse_scale <= 0.0 {
        return None;
    }
    let rx = radius / inverse_scale;
    let ry = radius / inverse_scale;
    let minimum = rx.min(ry);
    if !minimum.is_finite() || minimum <= 0.0 {
        return None;
    }
    let factor = if minimum < 0.5 { 0.5 / minimum } else { 1.0 };
    let width_alpha = 1.0 / factor;
    let extent_device_x = rx * factor.max(1.0) + 0.5;
    let extent_device_y = ry * factor.max(1.0) + 0.5;
    let maximum = extent_device_x.max(extent_device_y);
    let radial_inner = 0.5 * (maximum - 1.0) / maximum;
    let extent_x = extent_device_x * inverse_scale;
    let extent_y = extent_device_y * inverse_scale;
    if !width_alpha.is_finite()
        || !(0.0..=1.0).contains(&width_alpha)
        || !extent_x.is_finite()
        || !extent_y.is_finite()
        || extent_x <= 0.0
        || extent_y <= 0.0
        || !radial_inner.is_finite()
        || !(0.0..0.5).contains(&radial_inner)
    {
        return None;
    }
    Some([
        x,
        y,
        extent_x,
        extent_y,
        direction_x,
        direction_y,
        1.0,
        radial_inner,
        width_alpha,
    ])
}
