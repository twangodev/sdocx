//! Scale-independent V14 fountain shading using only Svg vector primitives.
use super::vector::{
    Blend, Circle, ColorValue, Data, Definitions, Gradient, Group, LinearGradient, Mask, Paint,
    Path, ReplayPart, Scene, Stop, Styled, SvgMask, Transform, coordinate, decimal,
};
use crate::PreparedStroke;

pub(super) fn render(svg: &mut Scene, paint: &PreparedStroke<'_>, replay: bool) -> bool {
    let (Some(directions), Some(radii), Some(bounds)) =
        (&paint.dot_directions, &paint.dot_radii, paint.bounds)
    else {
        return false;
    };
    let gradient = svg.definition::<Gradient>();
    let mask = svg.definition::<Mask>();
    let x = bounds.x_min - 1.;
    let y = bounds.y_min - 1.;
    let width = bounds.x_max - bounds.x_min + 2.;
    let height = bounds.y_max - bounds.y_min + 2.;
    let rectangle = Data::new()
        .move_to((coordinate(x, 4), coordinate(y, 4)))
        .horizontal_line_by(coordinate(width, 4))
        .vertical_line_by(coordinate(height, 4))
        .horizontal_line_by(-coordinate(width, 4))
        .close();
    let ramp = LinearGradient::new(&gradient)
        .x1(-1)
        .x2(1)
        .y1(0)
        .y2(0)
        .add(Stop::new(0, ColorValue::gray(0.07)))
        .add(Stop::new(0.25, ColorValue::WHITE))
        .add(Stop::new(0.75, ColorValue::WHITE))
        .add(Stop::new(1, ColorValue::gray(0.07)));
    svg.scope(Definitions::new().add(ramp), |svg| {
        svg.scope(
            SvgMask::luminance(&mask)
                .x(decimal(x, 4))
                .y(decimal(y, 4))
                .width(decimal(width, 4))
                .height(decimal(height, 4)),
            |svg| {
                svg.scope(Group::new().isolated(), |svg| {
                    svg.push(Path::new().fill(Paint::BLACK).data(rectangle.clone()));
                    // Opaque Lighten stamps implement maximum coverage without alpha accumulation.
                    for (index, ((point, radius), direction)) in
                        paint.points.iter().zip(radii).zip(directions).enumerate()
                    {
                        let dx = direction.x * radius;
                        let dy = direction.y * radius;
                        svg.push(
                            Group::new()
                                .blend(Blend::Lighten)
                                .replay_part(replay.then_some(ReplayPart(index + 1)))
                                .add(
                                    Circle::new()
                                        .r(1)
                                        .fill(Paint::Gradient(gradient))
                                        .transformed(Transform::matrix(
                                            [dx, dy, -dy, dx, point.x, point.y],
                                            7,
                                            4,
                                        )),
                                ),
                        );
                    }
                });
            },
        );
    });
    svg.push(
        Path::new()
            .fill(Paint::from_hex(&paint.color))
            .fill_opacity(decimal(paint.opacity, 6))
            .masked(&mask)
            .data(rectangle),
    );
    true
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::super::vector::Svg;
    use super::*;
    use crate::{BoundingBox, Point, Stroke};
    use std::borrow::Cow;

    fn shaded_stamp(direction: Point, count: usize) -> String {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../../../../conformance/fountain-v14.json"))
                .unwrap();
        let stroke: Stroke = serde_json::from_value(reference["stroke"].clone()).unwrap();
        let mut paint = crate::prepare_stroke(&stroke, false);
        paint.points = Cow::Owned(vec![Point { x: 32., y: 32. }; count]);
        paint.dot_radii = Some(vec![20.; count]);
        paint.dot_directions = Some(vec![direction; count]);
        paint.bounds = Some(BoundingBox {
            x_min: 12.,
            y_min: 12.,
            x_max: 52.,
            y_max: 52.,
        });
        paint.color = "#000000".into();
        paint.opacity = 0.6;
        let mut svg = Scene::new(Svg::new().width(64).height(64));
        assert!(render(&mut svg, &paint, false));
        svg.finish()
    }

    fn preview(svg: &str) -> resvg::tiny_skia::Pixmap {
        let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default()).unwrap();
        let mut pixmap = resvg::tiny_skia::Pixmap::new(64, 64).unwrap();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::identity(),
            &mut pixmap.as_mut(),
        );
        pixmap
    }

    #[test]
    fn fountain_gradient_follows_tangent_and_applies_opacity_once() {
        let horizontal = shaded_stamp(Point { x: 1., y: 0. }, 1);
        let image = preview(&horizontal);
        // At pixel center x=48.5, normalized tangent distance is 0.825.
        let expected = 255. * 0.6 * (1. - 1.86 * (0.825 - 0.5));
        assert!((image.pixel(48, 32).unwrap().alpha() as f64 - expected).abs() < 2.);
        assert!((image.pixel(32, 48).unwrap().alpha() as i32 - 153).abs() <= 1);
        let repeated = preview(&shaded_stamp(Point { x: 1., y: 0. }, 2));
        // Check the filled interior; edge antialiasing belongs to the Svg
        // consumer and can differ when coincident boundaries are drawn twice.
        for y in 14..50 {
            for x in 14..50 {
                if (x as f64 + 0.5 - 32.).hypot(y as f64 + 0.5 - 32.) < 18. {
                    assert_eq!(
                        image.pixel(x, y),
                        repeated.pixel(x, y),
                        "overlap at {x},{y}"
                    );
                }
            }
        }
        let vertical = preview(&shaded_stamp(Point { x: 0., y: 1. }, 1));
        assert_eq!(image.pixel(48, 32), vertical.pixel(32, 48));
        assert!(!horizontal.contains("<image"));
        assert!(!horizontal.contains("<filter"));
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn fountain_pdf_retains_vector_shading_and_blending() {
        let page = crate::RenderedPage {
            text_diagnostics: Vec::new(),
            object_diagnostics: Vec::new(),
            geometry_diagnostics: Vec::new(),
            paint_diagnostics: Vec::new(),
            source_page_index: 0,
            width: 64,
            height: 64,
            svg: shaded_stamp(Point { x: 1., y: 0. }, 2),
        };
        let bytes = crate::render_svg_pages_pdf(&[page], &crate::PdfOptions::default()).unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        let mut shading = false;
        let mut lighten = false;
        for object in pdf.objects.values() {
            let dict = match object {
                lopdf::Object::Dictionary(dict) => dict,
                lopdf::Object::Stream(stream) => &stream.dict,
                _ => continue,
            };
            assert_ne!(
                dict.get(b"Subtype").and_then(lopdf::Object::as_name).ok(),
                Some(b"Image".as_slice())
            );
            shading |= dict.has(b"ShadingType");
            lighten |= dict.get(b"BM").and_then(lopdf::Object::as_name).ok()
                == Some(b"Lighten".as_slice());
        }
        assert!(shading, "gradient must remain a PDF shading");
        assert!(
            lighten,
            "maximum coverage must retain its vector blend mode"
        );
    }

    #[test]
    fn saved_fountain_alpha_applies_once_to_overlapping_stamps() {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../../../../conformance/fountain-v14.json"))
                .unwrap();
        let mut stroke: Stroke = serde_json::from_value(reference["stroke"].clone()).unwrap();
        stroke.points = vec![
            Point { x: 20., y: 20. },
            Point { x: 35., y: 20. },
            Point { x: 50., y: 20. },
        ];
        stroke.pressures = vec![0.7; 3];
        stroke.timestamps = vec![0, 10, 20];
        let render_stroke = |stroke: &Stroke| {
            let mut svg = Scene::new(Svg::new().width(64).height(64));
            crate::render::render_stroke(
                &mut svg,
                stroke,
                crate::RenderTheme::for_canvas(false),
                None,
            );
            let svg = svg.finish();
            assert!(!svg.contains("<image"));
            assert!(!svg.contains("<filter"));
            preview(&svg)
        };
        for settings in ["14;", "18;0;100;"] {
            stroke.rendering.as_mut().unwrap().advanced_settings = Some(settings.into());
            stroke.rendering.as_mut().unwrap().style.color_argb = Some(0xff000000);
            let opaque = render_stroke(&stroke);
            assert!(opaque.pixels().iter().any(|p| p.alpha() == 255));
            for alpha in [0, 1, 64, 128, 254, 255] {
                stroke.rendering.as_mut().unwrap().style.color_argb = Some(alpha << 24);
                let paint = crate::prepare_stroke(&stroke, false);
                assert!(paint.dot_radii.is_some());
                assert_eq!(paint.opacity, (alpha as f32 / 255.) as f64);
                let image = render_stroke(&stroke);
                for (actual, full) in image.pixels().iter().zip(opaque.pixels()) {
                    let expected = full.alpha() as f64 * paint.opacity;
                    assert!(
                        (actual.alpha() as f64 - expected).abs() <= 2.,
                        "{settings}, alpha {alpha}"
                    );
                }
            }
        }
    }
}
