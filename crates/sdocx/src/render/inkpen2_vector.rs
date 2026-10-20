//! Vector projection of the two recovered InkPen2 V9 paint routes.
//!
//! With constant RGB, the enhanced shader's normalization of its source-over
//! color canvas cancels ideally. We retain MAX coverage and apply ARGB alpha
//! once. Native intermediate RGBA8 rounding can still change the final RGB.
use super::vector::{
    Blend, Circle, ColorMatrix, ColorValue, Definitions, Filter, Gradient, Group, Paint,
    RadialGradient, Rectangle, ReplayPart, Scene, Stop, StrokeIndex, Styled, SvgFilter, Transform,
};
use crate::PreparedInkPen2V9;

pub(super) fn render(svg: &mut Scene, ink: &PreparedInkPen2V9, replay_index: Option<usize>) {
    let draw = |svg: &mut Scene| {
        if ink.enhanced_antialias {
            render_max(svg, ink, replay_index.is_some());
        } else {
            render_direct(svg, ink, replay_index.is_some());
        }
    };
    if let Some(index) = replay_index {
        svg.scope(Group::new().replay_stroke(StrokeIndex(index)), draw);
    } else {
        draw(svg);
    }
}

fn render_direct(svg: &mut Scene, ink: &PreparedInkPen2V9, replay: bool) {
    for (index, vertex) in ink.records.iter().enumerate() {
        let [cx, cy, rx, ry, edge, coverage, ..] = *vertex;
        let color = ColorValue::Rgb(ink.color);
        let id = svg.definition::<Gradient>();
        let opacity = f64::from(coverage * ink.opacity as f32);
        let stop = (edge * 2.0).clamp(0.0, 1.0);
        let matrix = Transform::matrix_unrounded([
            f64::from(rx),
            0.0,
            0.0,
            f64::from(ry),
            f64::from(cx),
            f64::from(cy),
        ]);
        let gradient = RadialGradient::new(&id)
            .cx(0)
            .cy(0)
            .r(1)
            .add(Stop::new(0, color).opacity(opacity))
            .add(Stop::new(stop, color).opacity(opacity))
            .add(Stop::new(1, color).opacity(0));
        svg.push(
            Group::new()
                .transformed(matrix)
                .add(Definitions::new().add(gradient))
                .replay_part(replay.then_some(ReplayPart(index + 1)))
                .add(Circle::new().cx(0).cy(0).r(1).fill(Paint::Gradient(id))),
        );
    }
}

fn render_max(svg: &mut Scene, ink: &PreparedInkPen2V9, replay: bool) {
    if ink.opacity == 0.0 {
        return;
    }
    let bounds = ink.bounds;
    // Two canonical page units include boundary antialiasing and keep the
    // structural PDF colorizer's mask from clipping native half-pixel extents.
    let x = bounds.x_min - 2.;
    let y = bounds.y_min - 2.;
    let width = bounds.x_max - bounds.x_min + 4.;
    let height = bounds.y_max - bounds.y_min + 4.;
    let filter = svg.definition::<Filter>();
    let color = ink.color;
    let matrix = ColorMatrix::matrix([
        0.,
        0.,
        0.,
        0.,
        f64::from(color.r) / 255.,
        0.,
        0.,
        0.,
        0.,
        f64::from(color.g) / 255.,
        0.,
        0.,
        0.,
        0.,
        f64::from(color.b) / 255.,
        1.,
        0.,
        0.,
        0.,
        0.,
    ]);
    let mut definitions = Definitions::new().add(
        SvgFilter::new(&filter)
            .region(x, y, width, height)
            .add(matrix),
    );
    let mut fields = Vec::with_capacity(ink.records.len());
    for [_, _, _, _, edge, coverage, ..] in &ink.records {
        let id = svg.definition::<Gradient>();
        let stop = (*edge * 2.).clamp(0., 1.);
        let mut gradient = RadialGradient::new(&id)
            .cx(0)
            .cy(0)
            .r(1)
            .add(Stop::new(0, ColorValue::WHITE).opacity(*coverage));
        if stop > 0. {
            gradient = gradient.add(Stop::new(stop, ColorValue::WHITE).opacity(*coverage));
        }
        let gradient = gradient.add(Stop::new(1, ColorValue::WHITE).opacity(0));
        definitions = definitions.add(gradient);
        fields.push(id);
    }
    svg.scope(definitions, |_| {});
    svg.scope(Group::new().filtered(&filter).opacity(ink.opacity), |svg| {
        svg.scope(Group::new().isolated(), |svg| {
            svg.push(
                Rectangle::new()
                    .x(x)
                    .y(y)
                    .width(width)
                    .height(height)
                    .fill(Paint::BLACK),
            );
            for (index, ([cx, cy, rx, ry, ..], id)) in ink.records.iter().zip(fields).enumerate() {
                let matrix = Transform::matrix_unrounded([
                    f64::from(*rx),
                    0.,
                    0.,
                    f64::from(*ry),
                    f64::from(*cx),
                    f64::from(*cy),
                ]);
                svg.push(
                    Group::new()
                        .blend(Blend::Lighten)
                        .isolated()
                        .transformed(matrix)
                        .replay_part(replay.then_some(ReplayPart(index + 1)))
                        .add(Circle::new().cx(0).cy(0).r(1).fill(Paint::BLACK))
                        .add(Circle::new().cx(0).cy(0).r(1).fill(Paint::Gradient(id))),
                );
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::super::vector::Svg;
    use super::*;

    fn center_alpha(enhanced_antialias: bool, alpha: u8) -> u8 {
        // RTV3's minimum-size expansion yields extent=1, inner=0 and
        // coverage=.2/.3 for source radii .1/.15. At this pixel center the
        // native mask is max(.2,.3), while direct source-over accumulates.
        let ink = PreparedInkPen2V9 {
            enhanced_antialias,
            records: vec![
                [32.5, 32.5, 1., 1., 0., 0.2, 0.2, 0.4, 0.6],
                [32.5, 32.5, 1., 1., 0., 0.3, 0.2, 0.4, 0.6],
            ],
            sample_ends: vec![0, 2],
            color: crate::Color {
                r: 51,
                g: 102,
                b: 153,
            },
            opacity: f64::from(f32::from(alpha) / 255.),
            bounds: crate::BoundingBox {
                x_min: 31.5,
                y_min: 31.5,
                x_max: 33.5,
                y_max: 33.5,
            },
        };
        let mut scene = Scene::new(Svg::new().width(64).height(64));
        render(&mut scene, &ink, None);
        let tree = resvg::usvg::Tree::from_str(&scene.finish(), &Default::default()).unwrap();
        let mut pixels = resvg::tiny_skia::Pixmap::new(64, 64).unwrap();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::identity(),
            &mut pixels.as_mut(),
        );
        pixels.pixel(32, 32).unwrap().alpha()
    }

    #[test]
    fn overlapping_radial_stamps_follow_the_selected_native_blend_equation() {
        for (alpha, native_max_alpha) in [(0, 0), (128, 39), (255, 77)] {
            let enhanced = center_alpha(true, alpha);
            // Native R8 mask quantization precedes final ARGB multiplication;
            // vector rasterizers can round the ideal continuous field once.
            assert!((i16::from(enhanced) - native_max_alpha).abs() <= 1);
            let direct = center_alpha(false, alpha);
            if alpha == 0 {
                assert_eq!(direct, 0);
            } else {
                assert!(direct > enhanced + 15, "direct={direct}, max={enhanced}");
            }
        }
    }
}
