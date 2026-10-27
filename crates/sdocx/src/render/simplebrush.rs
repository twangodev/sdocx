//! Vector paint for reconstructed SimpleBrush circular samples.
//!
//! Each sample is already in the scene's coordinate system. The opaque disc
//! fades to zero over its outermost scene unit, matching the source-derived
//! radial-gradient approximation used by the native paint comparison.
use super::vector::{
    Circle, ColorValue, Definitions, Gradient, Group, Paint, RadialGradient, Scene, Stop, Styled,
};

const MAX_STAMPS: usize = 32_768;
const MAX_ESTIMATED_MARKUP_BYTES: usize = 32 * 1024 * 1024;
const STAMP_MARKUP_BUDGET: usize = 1_024;
const MAX_COORDINATE: f64 = 10_000_000.0;

/// Draw `[x, y, radius]` samples and return false without changing the scene
/// when their geometry, color, or estimated output exceeds the supported bounds.
/// Replay is intentionally left to the caller's fallback route.
pub(super) fn render(
    scene: &mut Scene,
    samples: &[[f32; 3]],
    color: &str,
    page_slice: Option<(f64, f64)>,
) -> bool {
    let Some(color) = ColorValue::from_hex(color) else {
        return false;
    };
    if page_slice
        .is_some_and(|(top, bottom)| !top.is_finite() || !bottom.is_finite() || top > bottom)
        || samples.is_empty()
        || samples.len() > MAX_STAMPS
        || samples
            .len()
            .checked_mul(STAMP_MARKUP_BUDGET)
            .is_none_or(|bytes| bytes > MAX_ESTIMATED_MARKUP_BYTES)
        || samples.iter().any(|&[x, y, radius]| {
            let (x, y, radius) = (f64::from(x), f64::from(y), f64::from(radius));
            !x.is_finite()
                || !y.is_finite()
                || !radius.is_finite()
                || radius <= 0.0
                || x.abs() + radius > MAX_COORDINATE
                || y.abs() + radius > MAX_COORDINATE
        })
    {
        return false;
    }

    for &[x, y, radius] in samples {
        let (x, y, radius) = (f64::from(x), f64::from(y), f64::from(radius));
        // Keep touching discs as well: their antialiased fringe can contribute
        // at the slice boundary. The radius already includes the fade fringe.
        if page_slice.is_some_and(|(top, bottom)| y + radius < top || y - radius > bottom) {
            continue;
        }
        let inner = (radius - 1.0).max(0.0) / radius;
        let id = scene.definition::<Gradient>();
        let gradient = RadialGradient::new(&id)
            .cx(x)
            .cy(y)
            .r(radius)
            .add(Stop::new(0, color))
            .add(Stop::new(inner, color))
            .add(Stop::new(1, color).opacity(0));
        scene.push(
            Group::new().add(Definitions::new().add(gradient)).add(
                Circle::new()
                    .cx(x)
                    .cy(y)
                    .r(radius)
                    .fill(Paint::Gradient(id)),
            ),
        );
    }
    true
}

#[cfg(test)]
mod tests {
    use super::super::vector::Svg;
    use super::*;

    #[test]
    fn slice_keeps_intersecting_and_touching_discs() {
        let samples = [
            [0.0, 7.0, 2.0],
            [1.0, 8.0, 2.0],
            [2.0, 12.0, 3.0],
            [3.0, 22.0, 2.0],
            [4.0, 23.0, 2.0],
        ];
        let mut scene = Scene::new(Svg::new());
        assert!(render(&mut scene, &samples, "#000000", Some((10.0, 20.0))));
        let output = scene.finish();
        let xml = roxmltree::Document::parse(&output).unwrap();
        let circles: Vec<_> = xml
            .descendants()
            .filter(|n| n.has_tag_name("circle"))
            .collect();
        assert_eq!(circles.len(), 3);
        assert_eq!(
            circles
                .iter()
                .map(|n| n.attribute("cx").unwrap())
                .collect::<Vec<_>>(),
            ["1", "2", "3"]
        );
    }

    #[test]
    fn empty_slice_and_failed_preflight_leave_scene_untouched() {
        let empty = Scene::new(Svg::new()).finish();
        let mut scene = Scene::new(Svg::new());
        assert!(render(
            &mut scene,
            &[[0.0, 0.0, 1.0]],
            "#000000",
            Some((10.0, 20.0))
        ));
        assert_eq!(scene.finish(), empty);

        let mut scene = Scene::new(Svg::new());
        assert!(!render(
            &mut scene,
            &[[0.0, 15.0, 1.0], [0.0, f32::NAN, 1.0]],
            "#000000",
            Some((10.0, 20.0))
        ));
        assert_eq!(scene.finish(), empty);
    }

    #[test]
    fn no_slice_renders_all_discs() {
        let mut scene = Scene::new(Svg::new());
        assert!(render(
            &mut scene,
            &[[0.0, 0.0, 1.0], [0.0, 100.0, 1.0]],
            "#000000",
            None
        ));
        let output = scene.finish();
        let xml = roxmltree::Document::parse(&output).unwrap();
        assert_eq!(
            xml.descendants()
                .filter(|n| n.has_tag_name("circle"))
                .count(),
            2
        );
    }
}
