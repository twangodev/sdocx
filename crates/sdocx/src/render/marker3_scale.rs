//! Bounded two-level approximation of Marker3 V5's output-scale fringe.
use super::Marker3OutputScale;
use super::vector::{Data, Group, Paint, ReplayPath, Scene, Styled, coordinate};
use crate::PreparedStroke;

const MAX_STAMPS: usize = 4096;

pub(super) fn render(
    scene: &mut Scene,
    paint: &PreparedStroke<'_>,
    profile: Marker3OutputScale,
) -> bool {
    let Some(radii) = &paint.dot_radii else {
        return false;
    };
    if radii.is_empty()
        || radii.len() != paint.points.len()
        || radii.len() > MAX_STAMPS
        || !paint.opacity.is_finite()
        || !(0.0..=1.0).contains(&paint.opacity)
        || radii
            .iter()
            .any(|r| !r.is_finite() || *r <= 0.0 || *r > 400.5)
        || paint
            .points
            .iter()
            .any(|p| !p.x.is_finite() || !p.y.is_finite() || p.x.abs() > 1e7 || p.y.abs() > 1e7)
    {
        return false;
    }
    let scale = match profile {
        Marker3OutputScale::Half => 0.5,
        Marker3OutputScale::One => 1.0,
        Marker3OutputScale::Two => 2.0,
        Marker3OutputScale::Four => 4.0,
        Marker3OutputScale::Eight => 8.0,
    };
    let build = |shrink: f64| {
        let mut path = ReplayPath::default();
        for (point, radius) in paint.points.iter().zip(radii) {
            let radius = (radius - shrink).max(0.0);
            let arc = coordinate(radius, 4);
            path.push(
                Data::new()
                    .move_to((coordinate(point.x - radius, 4), coordinate(point.y, 4)))
                    .elliptical_arc_by((arc, arc, 0., true, false, coordinate(radius * 2., 4), 0.))
                    .elliptical_arc_by((arc, arc, 0., true, false, coordinate(-radius * 2., 4), 0.))
                    .close(),
                false,
            );
        }
        path.finish().fill(Paint::from_hex(&paint.color))
    };
    scene.scope(Group::new().opacity(paint.opacity), |scene| {
        scene.push(build(0.0).fill_opacity(0.5));
        scene.push(build(1.0 / scale));
    });
    true
}
