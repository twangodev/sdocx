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
pub(super) fn render(scene: &mut Scene, samples: &[[f32; 3]], color: &str) -> bool {
    let Some(color) = ColorValue::from_hex(color) else {
        return false;
    };
    if samples.is_empty()
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
