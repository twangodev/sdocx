//! Scale-independent V14 fountain shading using only SVG vector primitives.
use crate::PreparedStroke;
use std::fmt::Write;

pub(super) fn render(svg: &mut String, paint: &PreparedStroke<'_>) -> bool {
    let (Some(directions), Some(radii), Some(bounds)) =
        (&paint.dot_directions, &paint.dot_radii, paint.bounds)
    else {
        return false;
    };
    let id = svg.len();
    let x = bounds.x_min - 1.;
    let y = bounds.y_min - 1.;
    let width = bounds.x_max - bounds.x_min + 2.;
    let height = bounds.y_max - bounds.y_min + 2.;
    // Native texture y follows the tangent. Its middle half is opaque and
    // the two ends fall linearly to 1 - 0.93. Work in a unit circle so the
    // gradient and geometry scale together, independently of screen pixels.
    writeln!(svg, r#"<defs><linearGradient id="fg{id}" gradientUnits="userSpaceOnUse" x1="-1" x2="1" y1="0" y2="0"><stop offset="0" stop-color="rgb(7%,7%,7%)"/><stop offset=".25" stop-color="white"/><stop offset=".75" stop-color="white"/><stop offset="1" stop-color="rgb(7%,7%,7%)"/></linearGradient>"#).unwrap();
    writeln!(svg, r#"<mask id="fm{id}" maskUnits="userSpaceOnUse" x="{x:.4}" y="{y:.4}" width="{width:.4}" height="{height:.4}" style="mask-type:luminance"><g style="isolation:isolate"><path fill="black" d="M{x:.4},{y:.4}h{width:.4}v{height:.4}h-{width:.4}Z"/>"#).unwrap();
    // Opaque grayscale stamps over black, combined with Lighten, implement
    // maximum coverage. Ordinary alpha-over would darken overlapping stamps.
    // krilla-svg preserves these groups and gradients as PDF forms/shadings;
    // no SVG filter (which would trigger its bitmap fallback) is used.
    for ((point, radius), direction) in paint.points.iter().zip(radii).zip(directions) {
        let dx = direction.x * radius;
        let dy = direction.y * radius;
        writeln!(svg, r#"<g style="mix-blend-mode:lighten"><circle r="1" fill="url(#fg{id})" transform="matrix({dx:.7},{dy:.7},{:.7},{dx:.7},{:.4},{:.4})"/></g>"#, -dy, point.x, point.y).unwrap();
    }
    writeln!(svg, r#"</g></mask></defs><path fill="{}" fill-opacity="{:.6}" mask="url(#fm{id})" d="M{x:.4},{y:.4}h{width:.4}v{height:.4}h-{width:.4}Z"/>"#, paint.color, paint.opacity).unwrap();
    true
}

#[cfg(test)]
mod tests {
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
        let mut svg =
            String::from(r#"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64">"#);
        assert!(render(&mut svg, &paint));
        svg.push_str("</svg>");
        svg
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
        // Check the filled interior; edge antialiasing belongs to the SVG
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
}
