//! Fountain V14/V16 coverage using only SVG vector primitives.
use super::vector::{
    Blend, Circle, ColorMatrix, ColorValue, Definitions, Filter, Gradient, Group, LinearGradient,
    Paint, RadialGradient, ReplayPart, Scene, Stop, Styled, SvgFilter, Transform,
};
use crate::PreparedStroke;
use std::collections::HashMap;

pub(super) fn render(
    svg: &mut Scene,
    paint: &PreparedStroke<'_>,
    replay: bool,
    slice: Option<crate::render::PageSlice>,
) -> bool {
    let culling_slice = slice.filter(|slice| slice.fountain_paint_culling_enabled());
    let Some(plan) = PaintPlan::prepare(paint, culling_slice.is_some()) else {
        return false;
    };
    if culling_slice.is_some_and(|slice| plan.outside(slice)) {
        return true;
    }
    plan.emit(svg, paint, replay);
    true
}

enum PaintPlan {
    V14(V14Plan),
    V16(V16Plan),
}

struct V14Plan {
    color: crate::Color,
    region: (f64, f64, f64, f64),
    stamps: Vec<V14Stamp>,
    y_bounds: Option<(f64, f64)>,
}

#[derive(Clone, Copy)]
struct V14Stamp {
    point: crate::Point,
    dx: f64,
    dy: f64,
}

struct V16Plan {
    color: crate::Color,
    stamps: Vec<V16Stamp>,
    y_bounds: Option<(f64, f64)>,
}

impl PaintPlan {
    fn prepare(paint: &PreparedStroke<'_>, collect_bounds: bool) -> Option<Self> {
        if paint.profile == Some("FountainPen")
            && paint.dot_radii.is_some()
            && paint.dot_directions.is_none()
        {
            Some(Self::V16(V16Plan::prepare(paint)?))
        } else {
            Some(Self::V14(V14Plan::prepare(paint, collect_bounds)?))
        }
    }

    fn outside(&self, slice: crate::render::PageSlice) -> bool {
        let bounds = match self {
            Self::V14(plan) => plan.y_bounds,
            Self::V16(plan) => plan.y_bounds,
        };
        let Some((min_y, max_y)) = bounds else {
            return false;
        };
        let slice_min = f64::from(slice.y());
        let slice_max = slice_min + f64::from(slice.height());
        max_y < slice_min || min_y > slice_max
    }

    fn emit(self, svg: &mut Scene, paint: &PreparedStroke<'_>, replay: bool) {
        match self {
            Self::V14(plan) => plan.emit(svg, paint, replay),
            Self::V16(plan) => plan.emit(svg, paint, replay),
        }
    }
}

impl V14Plan {
    fn prepare(paint: &PreparedStroke<'_>, collect_bounds: bool) -> Option<Self> {
        let (Some(directions), Some(radii), Some(bounds)) =
            (&paint.dot_directions, &paint.dot_radii, paint.bounds)
        else {
            return None;
        };
        let color = parse_rgb(&paint.color)?;
        let region = (
            bounds.x_min - 1.,
            bounds.y_min - 1.,
            bounds.x_max - bounds.x_min + 2.,
            bounds.y_max - bounds.y_min + 2.,
        );
        let mut stamps =
            Vec::with_capacity(paint.points.len().min(radii.len()).min(directions.len()));
        let mut y_bounds: Option<(f64, f64)> = None;
        let mut bounds_known = region.0.is_finite()
            && region.1.is_finite()
            && region.2.is_finite()
            && region.3.is_finite()
            && region.2 > 0.0
            && region.3 > 0.0;
        for ((point, radius), direction) in paint.points.iter().zip(radii).zip(directions) {
            let dx = direction.x * radius;
            let dy = direction.y * radius;
            stamps.push(V14Stamp {
                point: *point,
                dx,
                dy,
            });
            // Full-page output does not need serialized-transform culling bounds.
            if !collect_bounds {
                continue;
            }
            let (Some(_tx), Some(ty)) = (
                svg_number(point.x, 4).and_then(f32_number),
                svg_number(point.y, 4).and_then(f32_number),
            ) else {
                bounds_known = false;
                continue;
            };
            let (Some(a), Some(b), Some(c), Some(d)) = (
                svg_number(dx, 7).and_then(f32_number),
                svg_number(dy, 7).and_then(f32_number),
                svg_number(-dy, 7).and_then(f32_number),
                svg_number(dx, 7).and_then(f32_number),
            ) else {
                bounds_known = false;
                continue;
            };
            let extent = next_up(a.hypot(c).max(b.hypot(d)));
            let low = next_down(next_down(ty) - extent);
            let high = next_up(next_up(ty) + extent);
            y_bounds = Some(match y_bounds {
                Some((min_y, max_y)) => (min_y.min(f64::from(low)), max_y.max(f64::from(high))),
                None => (f64::from(low), f64::from(high)),
            });
        }
        if !bounds_known {
            y_bounds = None;
        }
        Some(Self {
            color,
            region,
            stamps,
            y_bounds,
        })
    }

    fn emit(self, svg: &mut Scene, paint: &PreparedStroke<'_>, replay: bool) {
        let (x, y, width, height) = self.region;
        let gradient = svg.definition::<Gradient>();
        let filter = svg.definition::<Filter>();
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
            f64::from(self.color.r) / 255.,
            0.,
            0.,
            0.,
            0.,
            f64::from(self.color.g) / 255.,
            0.,
            0.,
            0.,
            0.,
            f64::from(self.color.b) / 255.,
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
                    for (index, stamp) in self.stamps.iter().enumerate() {
                        svg.push(
                            Group::new()
                                .blend(Blend::Lighten)
                                .replay_part(replay.then_some(ReplayPart(index + 1)))
                                .add(
                                    Circle::new()
                                        .r(1)
                                        .fill(Paint::Gradient(gradient))
                                        .transformed(Transform::matrix(
                                            [
                                                stamp.dx,
                                                stamp.dy,
                                                -stamp.dy,
                                                stamp.dx,
                                                stamp.point.x,
                                                stamp.point.y,
                                            ],
                                            7,
                                            4,
                                        )),
                                ),
                        );
                    }
                });
            },
        );
    }
}

fn svg_number(value: f64, places: usize) -> Option<f64> {
    format!("{value:.places$}").parse().ok()
}

fn f32_number(value: f64) -> Option<f32> {
    let value = value as f32;
    value.is_finite().then_some(value)
}

fn next_up(value: f32) -> f32 {
    if value.is_nan() || value == f32::INFINITY {
        value
    } else if value == 0.0 {
        f32::from_bits(1)
    } else if value > 0.0 {
        f32::from_bits(value.to_bits() + 1)
    } else {
        f32::from_bits(value.to_bits() - 1)
    }
}

fn next_down(value: f32) -> f32 {
    -next_up(-value)
}

#[derive(Clone, Copy)]
struct V16Stamp {
    x: f32,
    y: f32,
    extent_x: f32,
    extent_y: f32,
    gradient_key: (u32, u32),
}

/// Retain the V16 RTV5 radial coverage as vector gradients at canonical unit
/// inverse scale. This is the saved shader equation, not a device-pixel match
/// for arbitrary SVG zoom or PDF viewing scale.
impl V16Plan {
    fn prepare(paint: &PreparedStroke<'_>) -> Option<Self> {
        let (Some(radii), Some(_bounds)) = (&paint.dot_radii, paint.bounds) else {
            return None;
        };
        if radii.len() != paint.points.len()
            || radii.is_empty()
            || !paint.opacity.is_finite()
            || !(0.0..=1.0).contains(&paint.opacity)
        {
            return None;
        }
        let color = parse_rgb(&paint.color)?;

        // RTV5::AddPoint float32 arithmetic at sx=sy=1, in native instruction
        // order: rx/ry, min and factor, width compensation, extents, then inner.
        // A gradient is shared only when both shader coverage parameters have
        // identical f32 bits.
        let mut stamps = Vec::with_capacity(radii.len());
        let mut y_bounds: Option<(f64, f64)> = None;
        for (point, radius) in paint.points.iter().zip(radii) {
            let radius = *radius as f32;
            if !radius.is_finite() || radius <= 0. {
                return None;
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
                return None;
            }
            let key = (alpha.to_bits(), inner.to_bits());
            y_bounds = Some(match y_bounds {
                Some((min_y, max_y)) => (
                    min_y.min(f64::from(next_down(y - extent_y))),
                    max_y.max(f64::from(next_up(y + extent_y))),
                ),
                None => (
                    f64::from(next_down(y - extent_y)),
                    f64::from(next_up(y + extent_y)),
                ),
            });
            stamps.push(V16Stamp {
                x,
                y,
                extent_x,
                extent_y,
                gradient_key: key,
            });
        }

        Some(Self {
            color,
            stamps,
            y_bounds,
        })
    }

    fn emit(self, svg: &mut Scene, paint: &PreparedStroke<'_>, replay: bool) {
        let mut unique = Vec::<(u32, u32, super::vector::Definition<Gradient>)>::new();
        let mut indices = HashMap::<(u32, u32), usize>::new();
        for stamp in &self.stamps {
            let key = stamp.gradient_key;
            indices.entry(key).or_insert_with(|| {
                let index = unique.len();
                unique.push((key.0, key.1, svg.definition::<Gradient>()));
                index
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
                .add(Stop::new(0, ColorValue::Rgb(self.color)).opacity(source_alpha));
            if inner > 0. {
                gradient = gradient.add(
                    Stop::new(inner + inner, ColorValue::Rgb(self.color)).opacity(source_alpha),
                );
            }
            definitions =
                definitions.add(gradient.add(Stop::new(1, ColorValue::Rgb(self.color)).opacity(0)));
        }
        svg.scope(definitions, |_| {});
        svg.scope(Group::new(), |svg| {
            // RTV5 submits each instance with source-over blending. Keep the
            // per-instance ARGB alpha in its radial field so overlaps accumulate
            // in draw order; applying it to this parent group would apply it once
            // after the instances have already combined.
            for (index, stamp) in self.stamps.iter().enumerate() {
                let Some(&gradient_index) = indices.get(&stamp.gradient_key) else {
                    continue;
                };
                let Some((_, _, gradient)) = unique.get(gradient_index) else {
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
    }
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
        assert!(render(&mut svg, &paint, replay, None));
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
        assert!(render(&mut svg, &paint, replay, None));
        svg.finish()
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn v16_slice_bounds_include_shader_fringe_and_touching_edges() {
        use crate::{LayoutPage, Page};

        let reference: serde_json::Value =
            serde_json::from_str(include_str!("../../../../conformance/fountain-v14.json"))
                .unwrap();
        let stroke: Stroke = serde_json::from_value(reference["stroke"].clone()).unwrap();
        let mut paint = crate::prepare_stroke(&stroke, false);
        paint.profile = Some("FountainPen");
        paint.points = Cow::Owned(vec![Point { x: 10.0, y: 10.0 }]);
        paint.dot_radii = Some(vec![0.25]);
        paint.dot_directions = None;
        paint.bounds = Some(BoundingBox {
            x_min: 0.0,
            y_min: 0.0,
            x_max: 20.0,
            y_max: 20.0,
        });
        let page = LayoutPage {
            page: Page {
                uuid: "slice-test".to_owned(),
                width: 100,
                height: 100,
                content_bbox: BoundingBox::default(),
                background_color: None,
                template: None,
                background: Default::default(),
                objects: Vec::new(),
            },
            source_page_index: 0,
            body_text: None,
        };
        let touching = crate::render::PageSlice::new_for_page(&page, 11, 10, false).unwrap();
        let separated = crate::render::PageSlice::new_for_page(&page, 12, 10, false).unwrap();
        let plan = PaintPlan::prepare(&paint, true).unwrap();
        assert!(!plan.outside(touching), "the V16 +0.5 extent touches y=11");
        assert!(plan.outside(separated), "the V16 fringe ends before y=12");

        // Compare identical viewports with culling on/off, including a visible
        // fringe and the adjacent empty slice. This is transport equivalence,
        // not a native pixel oracle.
        for y in [10, 11, 12] {
            let request = crate::render::PageSceneRequest::sliced(&page, y, 10, false).unwrap();
            let images = [request, request.with_fountain_paint_culling(false)].map(|request| {
                let mut scene = Scene::new(Svg::new().width(64).height(10).view_box(
                    super::super::vector::ViewBox::unrounded(0., f64::from(y), 64., 10.),
                ));
                assert!(render(&mut scene, &paint, false, request.slice()));
                preview(&scene.finish())
            });
            assert!(images[0].data() == images[1].data(), "slice y={y}");
            if y == 10 {
                assert!(images[0].pixels().iter().any(|pixel| pixel.alpha() > 0));
            }
        }
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
            background_diagnostics: Vec::new(),
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
            background_diagnostics: Vec::new(),
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
                None,
                None,
                None,
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
