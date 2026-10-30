use super::Marker3OutputScale;
use super::vector::{Data, Group, Paint, ReplayPath, Scene, Styled, coordinate};
use crate::PreparedStroke;
use std::cell::Cell;

pub(super) fn render(
    scene: &mut Scene,
    paint: &PreparedStroke<'_>,
    profile: Marker3OutputScale,
    remaining: &Cell<usize>,
) -> bool {
    let Some(radii) = &paint.dot_radii else {
        return false;
    };
    if paint
        .points
        .iter()
        .any(|p| !p.x.is_finite() || !p.y.is_finite() || p.x.abs() > 256.0 || p.y.abs() > 256.0)
        || radii.is_empty()
        || radii.len() != paint.points.len()
        || radii.len() > 1024
        || radii
            .iter()
            .any(|r| !r.is_finite() || *r <= 0.0 || *r > 400.0)
        || !paint.opacity.is_finite()
        || !(0.0..=1.0).contains(&paint.opacity)
    {
        return false;
    }
    let scale = match profile {
        Marker3OutputScale::Half => return false,
        Marker3OutputScale::One => 1.,
        Marker3OutputScale::Two => 2.,
        Marker3OutputScale::Four => 4.,
        Marker3OutputScale::Eight => return false,
    };
    let Some(cost) = radii.len().checked_mul(4 * 128) else {
        return false;
    };
    let Some(after) = remaining.get().checked_sub(cost) else {
        return false;
    };
    remaining.set(after);
    scene.scope(Group::new().opacity(paint.opacity), |scene| {
        for layer in 1..=4 {
            let t = layer as f64 / 4.;
            let mut path = ReplayPath::default();
            for (point, radius) in paint.points.iter().zip(radii) {
                let mut d = Data::new();
                let fringe = if *radius == 1.5 { 0.2 } else { 0.5 };
                for i in 0..128 {
                    let angle = std::f64::consts::TAU * i as f64 / 128.;
                    let (s, c) = angle.sin_cos();
                    let r = (radius + fringe - t * fringe * (c.abs() + s.abs()) / scale).max(0.);
                    let p = (
                        coordinate(point.x + r * c, 4),
                        coordinate(point.y + r * s, 4),
                    );
                    d = if i == 0 { d.move_to(p) } else { d.line_to(p) };
                }
                path.push(d.close(), false);
            }
            let opacity = 1. / (5 - layer) as f64;
            scene.push(
                path.finish()
                    .fill(Paint::from_hex(&paint.color))
                    .fill_opacity(opacity),
            );
        }
    });
    true
}
