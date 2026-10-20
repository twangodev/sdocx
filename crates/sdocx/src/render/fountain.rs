//! Fountain V14/V16 coverage using only SVG vector primitives.
use super::vector::{
    Blend, Circle, ColorMatrix, ColorValue, Definitions, Filter, Gradient, Group, LinearGradient,
    Paint, RadialGradient, ReplayPart, Scene, Stop, Styled, SvgFilter, Transform,
};
use crate::PreparedStroke;
use std::collections::HashMap;

pub(super) fn render(svg: &mut Scene, paint: &PreparedStroke<'_>, replay: bool) -> bool {
    if paint.profile == Some("FountainPen")
        && paint.dot_radii.is_some()
        && paint.dot_directions.is_none()
    {
        return render_v16(svg, paint, replay);
    }
    let (Some(directions), Some(radii), Some(bounds)) =
        (&paint.dot_directions, &paint.dot_radii, paint.bounds)
    else {
        return false;
    };
    let Some(color) = parse_rgb(&paint.color) else {
        return false;
    };
    let gradient = svg.definition::<Gradient>();
    let filter = svg.definition::<Filter>();
    let x = bounds.x_min - 1.;
    let y = bounds.y_min - 1.;
    let width = bounds.x_max - bounds.x_min + 2.;
    let height = bounds.y_max - bounds.y_min + 2.;
    let ramp = LinearGradient::new(&gradient)
        .x1(-1)
        .x2(1)
        .y1(0)
        .y2(0)
        .add(Stop::new(0, ColorValue::gray(0.07)))
        .add(Stop::new(0.25, ColorValue::WHITE))
        .add(Stop::new(0.75, ColorValue::WHITE))
        .add(Stop::new(1, ColorValue::gray(0.07)));
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
    svg.scope(
        Definitions::new().add(ramp).add(
            SvgFilter::new(&filter)
                .region(x, y, width, height)
                .add(matrix),
        ),
        |_| {},
    );
    svg.scope(
        Group::new().filtered(&filter).opacity(paint.opacity),
        |svg| {
            svg.scope(Group::new().isolated(), |svg| {
                svg.push(
                    super::vector::Rectangle::new()
                        .x(x)
                        .y(y)
                        .width(width)
                        .height(height)
                        .fill(Paint::BLACK),
                );
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
    true
}

#[derive(Clone, Copy)]
struct V16Stamp {
    x: f32,
    y: f32,
    extent_x: f32,
    extent_y: f32,
    gradient: usize,
}

/// Retain the V16 RTV5 radial coverage as vector gradients at canonical unit
/// inverse scale. This is the saved shader equation, not a device-pixel match
/// for arbitrary SVG zoom or PDF viewing scale.
fn render_v16(svg: &mut Scene, paint: &PreparedStroke<'_>, replay: bool) -> bool {
    let (Some(radii), Some(_bounds)) = (&paint.dot_radii, paint.bounds) else {
        return false;
    };
    if radii.len() != paint.points.len()
        || radii.is_empty()
        || !paint.opacity.is_finite()
        || !(0.0..=1.0).contains(&paint.opacity)
    {
        return false;
    }
    let Some(color) = parse_rgb(&paint.color) else {
        return false;
    };

    // RTV5::AddPoint float32 arithmetic at sx=sy=1, in native instruction
    // order: rx/ry, min and factor, width compensation, extents, then inner.
    // A gradient is shared only when both shader coverage parameters have
    // identical f32 bits.
    let mut unique = Vec::<(u32, u32, super::vector::Definition<Gradient>)>::new();
    let mut indices = HashMap::<(u32, u32), usize>::new();
    let mut stamps = Vec::with_capacity(radii.len());
    for (point, radius) in paint.points.iter().zip(radii) {
        let radius = *radius as f32;
        if !radius.is_finite() || radius <= 0. {
            return false;
        }
        let rx = radius / 1.0_f32;
        let ry = radius / 1.0_f32;
        let minimum = rx.min(ry);
        let factor = if minimum < 0.5 { 0.5 / minimum } else { 1.0 };
        let alpha = 1.0 / factor;
        let extent_x = rx * factor.max(1.0) + 0.5;
        let extent_y = ry * factor.max(1.0) + 0.5;
        let maximum = extent_x.max(extent_y);
        let inner = 0.5 * (maximum - 1.0) / maximum;
        let x = point.x as f32;
        let y = point.y as f32;
        if !x.is_finite()
            || !y.is_finite()
            || !extent_x.is_finite()
            || !extent_y.is_finite()
            || !inner.is_finite()
        {
            return false;
        }
        let key = (alpha.to_bits(), inner.to_bits());
        let gradient = if let Some(&index) = indices.get(&key) {
            index
        } else {
            let index = unique.len();
            unique.push((key.0, key.1, svg.definition::<Gradient>()));
            indices.insert(key, index);
            index
        };
        stamps.push(V16Stamp {
            x,
            y,
            extent_x,
            extent_y,
            gradient,
        });
    }

    let mut definitions = Definitions::new();
    for (alpha_bits, inner_bits, id) in &unique {
        let alpha = f32::from_bits(*alpha_bits);
        let inner = f32::from_bits(*inner_bits);
        let source_alpha = alpha * paint.opacity as f32;
        let mut gradient = RadialGradient::new(id)
            .cx(0)
            .cy(0)
            .r(1)
            .add(Stop::new(0, ColorValue::Rgb(color)).opacity(source_alpha));
        if inner > 0. {
            gradient = gradient
                .add(Stop::new(inner + inner, ColorValue::Rgb(color)).opacity(source_alpha));
        }
        definitions =
            definitions.add(gradient.add(Stop::new(1, ColorValue::Rgb(color)).opacity(0)));
    }
    svg.scope(definitions, |_| {});
    svg.scope(Group::new(), |svg| {
        // RTV5 submits each instance with source-over blending. Keep the
        // per-instance ARGB alpha in its radial field so overlaps accumulate
        // in draw order; applying it to this parent group would apply it once
        // after the instances have already combined.
        for (index, stamp) in stamps.iter().enumerate() {
            let Some((_, _, gradient)) = unique.get(stamp.gradient) else {
                continue;
            };
            let transform = Transform::matrix_unrounded([
                stamp.extent_x as f64,
                0.,
                0.,
                stamp.extent_y as f64,
                stamp.x as f64,
                stamp.y as f64,
            ]);
            svg.push(
                Group::new()
                    .replay_part(replay.then_some(ReplayPart(index + 1)))
                    .transformed(transform)
                    .add(Circle::new().r(1).fill(Paint::Gradient(*gradient))),
            );
        }
    });
    true
}

fn parse_rgb(value: &str) -> Option<crate::Color> {
    let hex = value.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    Some(crate::Color {
        r: u8::from_str_radix(&hex[0..2], 16).ok()?,
        g: u8::from_str_radix(&hex[2..4], 16).ok()?,
        b: u8::from_str_radix(&hex[4..6], 16).ok()?,
    })
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::super::vector::Svg;
    use super::*;
    use crate::{BoundingBox, Point, Stroke};
    use std::borrow::Cow;

    fn shaded_stamp(direction: Point, count: usize) -> String {
        shaded_stamp_with_color(direction, count, "#000000", false)
    }

    fn shaded_stamp_with_color(
        direction: Point,
        count: usize,
        color: &str,
        replay: bool,
    ) -> String {
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
        paint.color = color.into();
        paint.opacity = 0.6;
        let mut svg = Scene::new(Svg::new().width(64).height(64));
        assert!(render(&mut svg, &paint, replay));
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

    fn v16_scene(stamps: &[(f64, f64, f64)], opacity: f64, replay: bool) -> String {
        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../../../../conformance/fountain-v14.json"))
                .unwrap();
        let stroke: Stroke = serde_json::from_value(reference["stroke"].clone()).unwrap();
        let mut paint = crate::prepare_stroke(&stroke, false);
        paint.profile = Some("FountainPen");
        paint.points = Cow::Owned(
            stamps
                .iter()
                .map(|(x, y, _)| Point { x: *x, y: *y })
                .collect(),
        );
        paint.dot_radii = Some(stamps.iter().map(|(_, _, r)| *r).collect());
        paint.dot_directions = None;
        paint.bounds = Some(BoundingBox {
            x_min: 0.,
            y_min: 0.,
            x_max: 64.,
            y_max: 64.,
        });
        paint.color = "#1a1a1a".into();
        paint.opacity = opacity;
        let mut svg = Scene::new(Svg::new().width(64).height(64));
        assert!(render(&mut svg, &paint, replay));
        svg.finish()
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
        assert_eq!(horizontal.matches("<filter ").count(), 1);
        assert_eq!(horizontal.matches("<feColorMatrix").count(), 1);
    }

    #[test]
    fn v14_colorizer_keeps_tint_opacity_and_replay_parts() {
        let svg = shaded_stamp_with_color(Point { x: 1., y: 0. }, 2, "#336699", false);
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let filter = xml
            .descendants()
            .find(|node| node.has_tag_name("filter"))
            .unwrap();
        assert_eq!(filter.attribute("filterUnits"), Some("userSpaceOnUse"));
        assert_eq!(
            filter.attribute("color-interpolation-filters"),
            Some("sRGB")
        );
        let matrix = xml
            .descendants()
            .find(|node| node.has_tag_name("feColorMatrix"))
            .unwrap();
        let values = svgtypes::NumberListParser::from(matrix.attribute("values").unwrap())
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(values.len(), 20);
        assert_eq!(values[4], 0x33 as f64 / 255.);
        assert_eq!(values[9], 0x66 as f64 / 255.);
        assert_eq!(values[14], 0x99 as f64 / 255.);
        assert_eq!(&values[15..], &[1., 0., 0., 0., 0.]);
        assert_eq!(
            xml.descendants()
                .filter(|node| node.attribute("filter").is_some())
                .count(),
            1
        );
        assert_eq!(
            xml.descendants()
                .filter(|node| node.attribute("opacity").is_some())
                .count(),
            1,
            "saved opacity is applied once on the filtered source"
        );
        let tinted = preview(&svg);
        let pixel = tinted.pixel(48, 32).unwrap();
        let expected_alpha = 255. * 0.6 * (1. - 1.86 * (0.825 - 0.5));
        assert!((pixel.alpha() as f64 - expected_alpha).abs() <= 2.);
        let expected_channel = |channel: u8| (channel as f64 * expected_alpha / 255.).round() as u8;
        for (actual, expected) in [
            (pixel.red(), expected_channel(0x33)),
            (pixel.green(), expected_channel(0x66)),
            (pixel.blue(), expected_channel(0x99)),
        ] {
            assert!((actual as i16 - expected as i16).abs() <= 1);
        }
        assert!(!svg.contains("<image"));
        assert!(!svg.contains("<feImage"));

        let replay = shaded_stamp_with_color(Point { x: 1., y: 0. }, 2, "#336699", true);
        let replay_xml = roxmltree::Document::parse(&replay).unwrap();
        let parts = replay_xml
            .descendants()
            .filter_map(|node| node.attribute("data-replay-part"))
            .collect::<Vec<_>>();
        assert_eq!(parts, ["1", "2"]);
    }

    #[test]
    fn v16_svg_fields_match_native_unit_scale_records() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/fountain-v16-addpoint-native.json"
        ))
        .unwrap();
        let mut checked = 0;
        for case in fixture["cases"].as_array().unwrap() {
            if case["inverse_xy"] != serde_json::json!([1.0, 1.0])
                || case["radius"].as_f64().unwrap() <= 0.0
            {
                continue;
            }
            let native: Vec<f32> = case["record_f32_le_hex_words"]
                .as_array()
                .unwrap()
                .iter()
                .map(|word| {
                    f32::from_bits(
                        u32::from_str_radix(word.as_str().unwrap(), 16)
                            .unwrap()
                            .swap_bytes(),
                    )
                })
                .collect();
            let svg = v16_scene(
                &[(32.25, 32.25, case["radius"].as_f64().unwrap())],
                1.,
                false,
            );
            let xml = roxmltree::Document::parse(&svg).unwrap();
            let transform = xml
                .descendants()
                .filter_map(|node| node.attribute("transform"))
                .find(|value| value.starts_with("matrix("))
                .unwrap();
            let matrix: Vec<f32> = transform
                .strip_prefix("matrix(")
                .unwrap()
                .strip_suffix(')')
                .unwrap()
                .split(|c: char| c == ',' || c.is_ascii_whitespace())
                .filter(|part| !part.is_empty())
                .map(|part| part.parse().unwrap())
                .collect();
            assert_eq!(matrix, [native[2], 0., 0., native[3], native[0], native[1]]);
            let stops: Vec<(f32, f32)> = xml
                .descendants()
                .filter(|node| node.has_tag_name("stop"))
                .map(|node| {
                    (
                        node.attribute("offset").unwrap().parse().unwrap(),
                        node.attribute("stop-opacity").unwrap().parse().unwrap(),
                    )
                })
                .collect();
            let mut expected = vec![(0., native[6])];
            if native[5] > 0. {
                expected.push((native[5] + native[5], native[6]));
            }
            expected.push((1., 0.));
            assert_eq!(stops, expected, "{}", case["case_id"]);
            checked += 1;
        }
        assert_eq!(checked, 6);
    }

    #[test]
    fn v16_unit_scale_radial_stops_match_rtv5_numeric_witnesses() {
        // Independent RTV5 AddPoint + fragment-shader witnesses at sx=sy=1:
        // r=.1 => extent 1, A=.2, inner=0; center=.2, u=.5 => .1.
        // r=.5 => extent 1, A=1, inner=0.
        // r=2 => extent 2.5, A=1, inner=.3 (u plateau ends at .6).
        let svg = v16_scene(
            &[
                (12.5, 12.5, 0.1),
                (24.5, 24.5, 0.499),
                (36.5, 36.5, 0.5),
                (48.5, 48.5, 0.501),
                (55.5, 55.5, 2.0),
            ],
            1.,
            false,
        );
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let gradients = xml
            .descendants()
            .filter(|node| node.has_tag_name("radialGradient"))
            .collect::<Vec<_>>();
        // .1 and .499 have the same inner stop but distinct width alpha.
        // .5 and .501 are distinct at float32, while repeat parameters share.
        assert_eq!(gradients.len(), 5);
        let stops = |gradient: roxmltree::Node<'_, '_>| {
            gradient
                .children()
                .filter(|node| node.has_tag_name("stop"))
                .map(|node| {
                    (
                        node.attribute("offset").unwrap().parse::<f32>().unwrap(),
                        node.attribute("stop-opacity")
                            .unwrap()
                            .parse::<f32>()
                            .unwrap(),
                    )
                })
                .collect::<Vec<_>>()
        };
        let tiny = gradients
            .iter()
            .find(|gradient| (stops(**gradient)[0].1 - 0.2).abs() < 1e-6)
            .unwrap();
        let tiny_stops = stops(*tiny);
        assert_eq!(tiny_stops, [(0., 0.2), (1., 0.)]);
        assert!((0.2_f32 * (1.0_f32 - 0.5_f32) - 0.1_f32).abs() < f32::EPSILON);
        let large = gradients
            .iter()
            .find(|gradient| {
                let values = stops(**gradient);
                values.len() == 3 && values[1].0 > 0.5
            })
            .unwrap();
        let large_stops = stops(*large);
        assert_eq!(large_stops[0], (0., 1.));
        assert!((large_stops[1].0 - 0.6).abs() < 1e-6);
        assert_eq!(large_stops[1].1, 1.);
        assert_eq!(large_stops[2], (1., 0.));
        assert!(!svg.contains("<image"));
        assert!(!svg.contains("<filter "));
        assert!(!svg.contains("<feColorMatrix"));
    }

    #[test]
    fn v16_instances_source_over_with_argb_alpha_per_stamp() {
        let one_svg = v16_scene(&[(32.5, 32.5, 0.1)], 1., false);
        let xml = roxmltree::Document::parse(&one_svg).unwrap();
        assert_eq!(
            xml.descendants()
                .filter(|node| node.has_tag_name("filter"))
                .count(),
            0
        );
        assert_eq!(
            xml.descendants()
                .filter(|node| node.attribute("opacity").is_some())
                .count(),
            0,
            "native ARGB alpha belongs to each V16 instance"
        );
        assert!(
            !xml.descendants()
                .any(|node| node.has_tag_name("rect") && node.attribute("fill") == Some("#000000"))
        );
        let gradient = xml
            .descendants()
            .find(|node| node.has_tag_name("radialGradient"))
            .unwrap();
        assert_eq!(
            gradient
                .children()
                .find(|node| node.has_tag_name("stop"))
                .unwrap()
                .attribute("stop-color"),
            Some("#1a1a1a")
        );
        assert!(!one_svg.contains("<image"));
        let one = preview(&one_svg);
        // Pixel center coincides with the stamp center: native shader A=.2.
        assert!(
            (one.pixel(32, 32).unwrap().alpha() as i32 - 51).abs() <= 1,
            "one center alpha {}",
            one.pixel(32, 32).unwrap().alpha()
        );
        let alpha = preview(&v16_scene(&[(32.5, 32.5, 0.1)], 0.5, false));
        // One instance has shader coverage .2 and source ARGB alpha .5.
        assert!((alpha.pixel(32, 32).unwrap().alpha() as i32 - 26).abs() <= 1);

        let overlap = preview(&v16_scene(
            &[(32.5, 32.5, 0.1), (32.75, 32.5, 0.15)],
            1.,
            false,
        ));
        // At the first center, the instance alphas are .2 and .225. V16 draws
        // with source-over, so .2 + .225 * (1-.2) = .38.
        assert!(
            (overlap.pixel(32, 32).unwrap().alpha() as i32 - 97).abs() <= 1,
            "overlap alpha was {}",
            overlap.pixel(32, 32).unwrap().alpha()
        );
        let partial_overlap = preview(&v16_scene(
            &[(32.5, 32.5, 0.1), (32.75, 32.5, 0.15)],
            0.5,
            false,
        ));
        // The source alpha multiplies each instance before blending: .1 +
        // .1125 * (1-.1) = .20125 (51/255 after rasterization).
        assert!(
            (partial_overlap.pixel(32, 32).unwrap().alpha() as i32 - 51).abs() <= 1,
            "partial-alpha overlap was {}",
            partial_overlap.pixel(32, 32).unwrap().alpha()
        );
        let replay = v16_scene(&[(32.5, 32.5, 0.1), (32.75, 32.5, 0.15)], 1., true);
        let xml = roxmltree::Document::parse(&replay).unwrap();
        assert!(
            xml.descendants()
                .any(|node| { node.attribute("transform") == Some("matrix(1,0,0,1,32.5,32.5)") }),
            "native fractional stamp centers must survive transform serialization"
        );
        let parts = xml
            .descendants()
            .filter_map(|node| node.attribute("data-replay-part"))
            .collect::<Vec<_>>();
        assert_eq!(parts, ["1", "2"]);
        assert!(!replay.contains("<image"));
        assert!(!replay.contains("<filter "));
        assert!(!replay.contains("mix-blend-mode:lighten"));
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

    #[cfg(feature = "pdf")]
    #[test]
    fn v16_pdf_retains_radial_opacity_shading_and_source_over_without_images() {
        let page = crate::RenderedPage {
            text_diagnostics: Vec::new(),
            object_diagnostics: Vec::new(),
            geometry_diagnostics: Vec::new(),
            paint_diagnostics: Vec::new(),
            source_page_index: 0,
            width: 64,
            height: 64,
            svg: v16_scene(&[(32.5, 32.5, 2.)], 0.5, false),
        };
        let bytes = crate::render_svg_pages_pdf(&[page], &crate::PdfOptions::default()).unwrap();
        let pdf = lopdf::Document::load_mem(&bytes).unwrap();
        let mut radial_shading = false;
        let mut non_normal_blend = false;
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
            if dict.has(b"ShadingType") {
                radial_shading |= dict
                    .get(b"ShadingType")
                    .and_then(lopdf::Object::as_i64)
                    .is_ok_and(|kind| kind == 3);
            }
            non_normal_blend |= dict.get(b"BM").and_then(lopdf::Object::as_name).ok()
                == Some(b"Lighten".as_slice());
        }
        assert!(
            radial_shading,
            "V16 radial opacity must remain a PDF shading"
        );
        assert!(
            !non_normal_blend,
            "V16 instances use source-over compositing"
        );
    }

    #[test]
    fn saved_fountain_argb_alpha_matches_each_renderer_path() {
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
        let render_stroke = |stroke: &Stroke, expect_filter: bool| {
            let mut svg = Scene::new(Svg::new().width(64).height(64));
            let mut calligraphy_session = crate::ink::calligraphy::CalligraphySession::new();
            crate::render::render_stroke(
                &mut svg,
                stroke,
                crate::RenderTheme::for_canvas(false),
                None,
                &mut calligraphy_session,
                true,
                [64, 64],
                None,
            );
            let svg = svg.finish();
            assert!(!svg.contains("<image"));
            if expect_filter {
                assert_eq!(svg.matches("<filter ").count(), 1);
            } else {
                assert!(!svg.contains("<filter"));
            }
            preview(&svg)
        };
        for settings in ["14;", "18;0;100;"] {
            let v16 = settings == "18;0;100;";
            stroke.rendering.as_mut().unwrap().advanced_settings = Some(settings.into());
            stroke.rendering.as_mut().unwrap().style.color_argb = Some(0xff000000);
            let opaque = render_stroke(&stroke, !v16);
            assert!(opaque.pixels().iter().any(|p| p.alpha() == 255));
            for alpha in [0, 1, 64, 128, 254, 255] {
                stroke.rendering.as_mut().unwrap().style.color_argb = Some(alpha << 24);
                let paint = crate::prepare_stroke(&stroke, false);
                assert!(paint.dot_radii.is_some());
                assert_eq!(paint.opacity, (alpha as f32 / 255.) as f64);
                let image = render_stroke(&stroke, !v16);
                for (actual, full) in image.pixels().iter().zip(opaque.pixels()) {
                    if v16 {
                        // V16 multiplies the source alpha into every instance,
                        // then blends those instances source-over. This is
                        // intentionally nonlinear where stamps overlap.
                        if alpha == 0 {
                            assert_eq!(actual.alpha(), 0, "V16 alpha-zero source");
                        } else {
                            assert!(actual.alpha() <= full.alpha());
                            if alpha == 255 {
                                assert_eq!(actual.alpha(), full.alpha());
                            }
                        }
                    } else {
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
}
