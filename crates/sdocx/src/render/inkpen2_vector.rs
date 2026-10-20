//! Vector projection of the two recovered InkPen2 V9 paint routes.
//!
//! With constant RGB, the enhanced shader's normalization of its source-over
//! color canvas cancels ideally. We retain MAX coverage and apply ARGB alpha
//! once. Native intermediate RGBA8 rounding can still change the final RGB.
use super::vector::{
    Blend, Circle, ColorMatrix, ColorValue, Definitions, Filter, Gradient, Group, Mask, Paint,
    RadialGradient, Rectangle, ReplayPart, Scene, Stop, StrokeIndex, Styled, SvgFilter, Transform,
};
use crate::PreparedInkPen2V9;

mod max_field;

pub(super) fn render(svg: &mut Scene, ink: &PreparedInkPen2V9, replay_index: Option<usize>) {
    let draw = |svg: &mut Scene| {
        if ink.per_stamp_rgb {
            render_per_stamp_rgb(svg, ink, replay_index.is_some());
        } else if ink.enhanced_antialias {
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

/// Internal staged renderer for records whose RGB channels differ by stamp.
///
/// Enhanced-AA uses ordered source-over RGB under a separate MAX-coverage
/// luminance mask, with saved alpha applied once around the masked group. The
/// mask isolates each stamp with explicitly cropped filter primitives. Keep
/// active-gradient public admission gated while the complete route is checked.
fn render_per_stamp_rgb(svg: &mut Scene, ink: &PreparedInkPen2V9, replay: bool) {
    if ink.opacity == 0.0
        || ink.records.is_empty()
        || ink.records.iter().all(|record| record[5] == 0.0)
    {
        return;
    }

    if !valid_record_rgb(&ink.records) {
        #[cfg(feature = "pdf")]
        if svg.retains_text() {
            svg.reject_native_text("InkPen2 per-stamp RGB channel is outside [0,1]".into());
            return;
        }
        // This private route is only enabled for prepared records with valid
        // normalized RGB. If that invariant is violated in a non-retained SVG
        // scene, fall back as one stroke to the established uniform-color path.
        if ink.enhanced_antialias {
            render_max(svg, ink, replay);
        } else {
            render_direct(svg, ink, replay);
        }
        return;
    }

    let max_atlas = if ink.enhanced_antialias {
        match max_field::MaxAtlas::new(ink) {
            Ok(atlas) => Some(atlas),
            Err(error) => {
                #[cfg(feature = "pdf")]
                if svg.retains_text() {
                    svg.reject_native_text(error);
                    return;
                }
                #[cfg(not(feature = "pdf"))]
                let _ = error;
                // Unsupported private prepared values follow the same whole-
                // stroke fallback policy as invalid RGB; no partial atlas.
                render_max(svg, ink, replay);
                return;
            }
        }
    } else {
        None
    };

    #[cfg(feature = "pdf")]
    let native_ink_id = if svg.retains_text() {
        match svg.native_ink().register_radial(
            &ink.records,
            ink.bounds,
            ink.opacity as f32,
            ink.enhanced_antialias,
        ) {
            Ok(id) => Some(id),
            Err(error) => {
                svg.reject_native_text(format!("InkPen2 radial field: {error}"));
                return;
            }
        }
    } else {
        None
    };

    let outer = Group::new().opacity(if ink.enhanced_antialias {
        ink.opacity
    } else {
        1.0
    });
    #[cfg(feature = "pdf")]
    let outer = match native_ink_id {
        Some(id) => outer.native_ink_id(id),
        None => outer,
    };

    svg.scope(outer, |svg| {
        if let Some(atlas) = &max_atlas {
            render_per_stamp_rgb_max(svg, ink, replay, atlas);
        } else {
            render_per_stamp_rgb_direct(svg, ink, replay);
        }
    });
}

fn valid_record_rgb(records: &[[f32; 9]]) -> bool {
    records.iter().all(|record| {
        record[6..9]
            .iter()
            .all(|channel| channel.is_finite() && (0.0..=1.0).contains(channel))
    })
}

fn field_region(ink: &PreparedInkPen2V9) -> (f64, f64, f64, f64) {
    let bounds = ink.bounds;
    let x = bounds.x_min - 2.0;
    let y = bounds.y_min - 2.0;
    (
        x,
        y,
        bounds.x_max - bounds.x_min + 4.0,
        bounds.y_max - bounds.y_min + 4.0,
    )
}

fn render_per_stamp_rgb_direct(svg: &mut Scene, ink: &PreparedInkPen2V9, replay: bool) {
    for (index, vertex) in ink.records.iter().enumerate() {
        let [cx, cy, rx, ry, edge, coverage, red, green, blue] = *vertex;
        let color = ColorValue::rgb_components(f64::from(red), f64::from(green), f64::from(blue))
            .expect("all per-stamp RGB channels were prevalidated");
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

fn render_per_stamp_rgb_max(
    svg: &mut Scene,
    ink: &PreparedInkPen2V9,
    replay: bool,
    atlas: &max_field::MaxAtlas,
) {
    let (x, y, width, height) = field_region(ink);
    let mask = svg.definition::<Mask>();
    let color_filter = svg.definition::<Filter>();
    let identity_rgb_opaque_alpha = ColorMatrix::matrix([
        1., 0., 0., 0., 0., 0., 1., 0., 0., 0., 0., 0., 1., 0., 0., 0., 0., 0., 0., 1.,
    ]);

    svg.scope(Definitions::new(), |svg| {
        svg.push(
            SvgFilter::new(&color_filter)
                .region(x, y, width, height)
                .add(identity_rgb_opaque_alpha),
        );
        atlas.emit(svg, ink, &mask, replay);
    });

    svg.scope(Group::new().masked(&mask), |svg| {
        svg.scope(Group::new().filtered(&color_filter), |svg| {
            for (index, [cx, cy, rx, ry, edge, coverage, red, green, blue]) in
                ink.records.iter().enumerate()
            {
                let color = ColorValue::rgb_components(
                    f64::from(*red),
                    f64::from(*green),
                    f64::from(*blue),
                )
                .expect("all per-stamp RGB channels were prevalidated");
                let id = svg.definition::<Gradient>();
                let opacity = f64::from(*coverage * ink.opacity as f32);
                let stop = (*edge * 2.0).clamp(0.0, 1.0);
                let gradient = RadialGradient::new(&id)
                    .cx(0)
                    .cy(0)
                    .r(1)
                    .add(Stop::new(0, color).opacity(opacity))
                    .add(Stop::new(stop, color).opacity(opacity))
                    .add(Stop::new(1, color).opacity(0));
                let matrix = Transform::matrix_unrounded([
                    f64::from(*rx),
                    0.0,
                    0.0,
                    f64::from(*ry),
                    f64::from(*cx),
                    f64::from(*cy),
                ]);
                svg.push(
                    Group::new()
                        .transformed(matrix)
                        .add(Definitions::new().add(gradient))
                        .replay_part(replay.then_some(ReplayPart(index + 1)))
                        .add(Circle::new().cx(0).cy(0).r(1).fill(Paint::Gradient(id))),
                );
            }
        });
    });
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
            per_stamp_rgb: false,
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

    fn colored_overlap(enhanced_antialias: bool, reverse: bool, alpha: u8) -> String {
        let mut records = vec![
            [32.5, 32.5, 8., 8., 0.3, 0.8, 1., 0., 0.],
            [32.5, 32.5, 8., 8., 0.3, 0.6, 0., 0., 1.],
        ];
        if reverse {
            records.reverse();
        }
        let ink = PreparedInkPen2V9 {
            per_stamp_rgb: true,
            enhanced_antialias,
            records,
            sample_ends: vec![0, 2],
            color: crate::Color {
                r: 51,
                g: 102,
                b: 153,
            },
            opacity: f64::from(f32::from(alpha) / 255.),
            bounds: crate::BoundingBox {
                x_min: 24.5,
                y_min: 24.5,
                x_max: 40.5,
                y_max: 40.5,
            },
        };
        let mut scene = Scene::new(Svg::new().width(64).height(64));
        render(&mut scene, &ink, Some(3));
        scene.finish()
    }

    fn colored_pixel(enhanced_antialias: bool, reverse: bool, alpha: u8) -> [u8; 4] {
        let svg = colored_overlap(enhanced_antialias, reverse, alpha);
        let tree = resvg::usvg::Tree::from_str(&svg, &Default::default()).unwrap();
        let mut pixels = resvg::tiny_skia::Pixmap::new(64, 64).unwrap();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::identity(),
            &mut pixels.as_mut(),
        );
        let pixel = pixels.pixel(32, 32).unwrap();
        [pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()]
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

    #[test]
    fn private_rgb_renderer_preserves_per_stamp_color_order_and_alpha() {
        // The center lies in both constant inner regions. The direct route
        // source-overs RGB and alpha per stamp; enhanced AA normalizes the
        // ordered RGB canvas and applies max(.8,.6) once as coverage.
        let a = 128.0_f64 / 255.0;
        let first = 0.8 * a;
        let second = 0.6 * a;
        let q = second + first * (1.0 - second);
        let premul = [first * (1.0 - second), 0.0, second];
        let direct_expected = [
            (premul[0] * 255.0).round() as i32,
            0,
            (premul[2] * 255.0).round() as i32,
            (q * 255.0).round() as i32,
        ];
        let mask_alpha = 0.8 * a;
        let enhanced_expected = [
            (premul[0] / q * mask_alpha * 255.0).round() as i32,
            0,
            (premul[2] / q * mask_alpha * 255.0).round() as i32,
            (mask_alpha * 255.0).round() as i32,
        ];
        for (enhanced, expected) in [(false, direct_expected), (true, enhanced_expected)] {
            let actual = colored_pixel(enhanced, false, 128);
            for (actual, expected) in actual.into_iter().zip(expected) {
                assert!(
                    (i32::from(actual) - expected).abs() <= 2,
                    "{actual} != {expected}; enhanced={enhanced}"
                );
            }
        }
        assert_ne!(
            colored_pixel(true, false, 128),
            colored_pixel(true, true, 128),
            "normalized color must retain source-over order"
        );
        assert_eq!(colored_pixel(true, false, 0), [0, 0, 0, 0]);
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn zero_alpha_rgb_renderer_does_not_register_a_pdf_field() {
        let ink = PreparedInkPen2V9 {
            per_stamp_rgb: true,
            enhanced_antialias: true,
            records: vec![[32., 32., 1., 1., 0.5, 0.8, 1., 0., 0.]],
            sample_ends: vec![0, 1],
            color: crate::Color { r: 0, g: 0, b: 0 },
            opacity: 0.0,
            bounds: crate::BoundingBox {
                x_min: 31.,
                y_min: 31.,
                x_max: 33.,
                y_max: 33.,
            },
        };
        let mut scene = Scene::new(Svg::new().width(64).height(64));
        scene.retain_text();
        render(&mut scene, &ink, None);
        assert_eq!(scene.take_native_ink().len(), 0);
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn empty_or_zero_coverage_rgb_renderer_is_a_retained_scene_noop_for_both_routes() {
        for enhanced_antialias in [false, true] {
            for records in [Vec::new(), vec![[32., 32., 1., 1., 0.25, 0., 1., 0., 0.]]] {
                let ink = PreparedInkPen2V9 {
                    per_stamp_rgb: true,
                    enhanced_antialias,
                    sample_ends: vec![0, records.len()],
                    records,
                    color: crate::Color { r: 0, g: 0, b: 0 },
                    opacity: 0.5,
                    bounds: crate::BoundingBox {
                        x_min: 31.,
                        y_min: 31.,
                        x_max: 33.,
                        y_max: 33.,
                    },
                };
                let mut scene = Scene::new(Svg::new().width(64).height(64));
                scene.retain_text();
                render(&mut scene, &ink, None);
                assert_eq!(scene.take_native_ink().len(), 0);
                assert!(scene.take_native_text_error().is_none());
                let output = scene.finish();
                assert!(!output.contains("sdocx-native-ink-"));
                assert!(!output.contains("data-replay-part"));
            }
        }
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn invalid_or_oversized_rgb_fields_reject_retained_scene_before_emitting_group() {
        let base = PreparedInkPen2V9 {
            per_stamp_rgb: true,
            enhanced_antialias: true,
            records: vec![[32., 32., 1., 1., 0.5, 0.8, 1., 0., 0.]],
            sample_ends: vec![0, 1],
            color: crate::Color { r: 0, g: 0, b: 0 },
            opacity: 0.5,
            bounds: crate::BoundingBox {
                x_min: 31.,
                y_min: 31.,
                x_max: 33.,
                y_max: 33.,
            },
        };
        let oversized = PreparedInkPen2V9 {
            records: vec![[32., 32., 1., 1., 0.25, 0.8, 1., 0., 0.]; 513],
            ..base.clone()
        };
        for ink in [base, oversized] {
            let mut scene = Scene::new(Svg::new().width(64).height(64));
            scene.retain_text();
            render(&mut scene, &ink, None);
            assert_eq!(scene.take_native_ink().len(), 0);
            assert!(scene.take_native_text_error().is_some());
            assert!(!scene.finish().contains("data-replay-part"));
        }
    }
}
