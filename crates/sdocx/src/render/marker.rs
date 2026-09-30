use super::text::{TextSettings, finite_native_geometry};
use super::vector::{Circle, Paint, Rectangle, Scene, Styled, decimal};
use crate::BulletType;

/// Native point-marker image sizing for the rendering display.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum PointMarkerTarget {
    #[default]
    Mobile,
    Tablet,
    Uwp,
}

impl PointMarkerTarget {
    fn image_range(self) -> (i32, f32, f32) {
        match self {
            Self::Mobile => (8, 1.5, 5.0),
            Self::Tablet => (7, 1.35, 2.25),
            Self::Uwp => (7, 4.5, 9.0),
        }
    }
}

pub(super) fn marker_center_y(
    baseline: f64,
    post_line_cursor: f64,
    line_base_height: f64,
    pixel_spacing: f64,
    default_cap_height_ratio: Option<f64>,
) -> Option<f64> {
    if ![baseline, post_line_cursor, line_base_height, pixel_spacing]
        .into_iter()
        .all(|value| finite_native_geometry(value).is_some())
        || line_base_height < 0.0
    {
        return None;
    }
    let center = if pixel_spacing == 0.0 {
        post_line_cursor - finite_native_geometry(1.35 * line_base_height)? * 0.5
    } else {
        let ratio = default_cap_height_ratio
            .filter(|ratio| finite_native_geometry(*ratio).is_some() && *ratio > 0.0)?;
        baseline - finite_native_geometry(ratio * line_base_height)? * 0.5
    };
    finite_native_geometry(center)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PointMarker {
    SolidCircle,
    OpenCircle,
    SolidSquare,
    OpenSquare,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PointMarkerMetrics {
    pub reserved_width: f64,
    pub button_width: f64,
    pub radius: f64,
}

impl PointMarkerMetrics {
    pub fn measure(
        font_size: f64,
        settings: TextSettings,
        target: PointMarkerTarget,
    ) -> Option<Self> {
        let font_size = font_size as f32;
        let scale = settings.scale;
        if !font_size.is_finite() || !scale.is_finite() || scale <= 0.0 {
            return None;
        }
        let (max_font, min_pixels, max_pixels) = target.image_range();
        let min_pixels = min_pixels * scale;
        let max_pixels = max_pixels * scale;
        let button_width = 20.0_f32 * scale;
        let reserved_width = button_width + 6.0_f32 * scale;
        if !reserved_width.is_finite() || !max_pixels.is_finite() {
            return None;
        }
        let font = ((font_size / scale) as i32).max(1);
        let step = (max_pixels - min_pixels) / (max_font - 1) as f32;
        let mut pixels = max_pixels;
        for threshold in (1..=max_font).rev() {
            if threshold == 1 {
                pixels = min_pixels;
            }
            if font >= threshold {
                break;
            }
            pixels -= step;
        }
        let image_size = pixels.round() as i32;
        let radius = ((image_size as f32) / 2.0).ceil();
        Some(Self {
            reserved_width: f64::from(reserved_width),
            button_width: f64::from(button_width),
            radius: f64::from(radius),
        })
    }
}

impl PointMarker {
    pub fn from_bullet(kind: BulletType) -> Option<Self> {
        match kind {
            BulletType::Arrow | BulletType::Diamond | BulletType::SolidCircle => {
                Some(Self::SolidCircle)
            }
            BulletType::WhiteCircle => Some(Self::OpenCircle),
            BulletType::BlackSquare => Some(Self::SolidSquare),
            BulletType::WhiteSquare => Some(Self::OpenSquare),
            _ => None,
        }
    }

    pub fn paint(
        self,
        scene: &mut Scene,
        metrics: PointMarkerMetrics,
        left: f64,
        top: f64,
        color: &str,
    ) -> Option<()> {
        let radius = metrics.radius;
        let diameter = 2.0 * radius;
        let right = left + diameter;
        let bottom = top + diameter;
        if ![left, top, radius, diameter, right, bottom]
            .into_iter()
            .all(|value| finite_native_geometry(value).is_some())
            || radius <= 0.0
            || right <= left
            || bottom <= top
        {
            return None;
        }
        let paint = Paint::from_hex(color)?;
        let cx = left + radius;
        let cy = top + radius;
        match self {
            Self::SolidCircle => scene.push(
                Circle::new()
                    .cx(decimal(cx, 5))
                    .cy(decimal(cy, 5))
                    .r(decimal(radius, 5))
                    .fill(paint),
            ),
            Self::OpenCircle => scene.push(
                Circle::new()
                    .cx(decimal(cx, 5))
                    .cy(decimal(cy, 5))
                    .r(decimal(radius * 0.875, 5))
                    .fill(Paint::None)
                    .stroke(paint)
                    .stroke_width(decimal(diameter / 8.0, 5)),
            ),
            Self::SolidSquare => scene.push(
                Rectangle::new()
                    .x(decimal(left, 5))
                    .y(decimal(top, 5))
                    .width(decimal(diameter, 5))
                    .height(decimal(diameter, 5))
                    .fill(paint),
            ),
            Self::OpenSquare => scene.push(
                Rectangle::new()
                    .x(decimal(left + diameter / 16.0, 5))
                    .y(decimal(top + diameter / 16.0, 5))
                    .width(decimal(diameter * 0.875, 5))
                    .height(decimal(diameter * 0.875, 5))
                    .fill(Paint::None)
                    .stroke(paint)
                    .stroke_width(decimal(diameter / 8.0, 5)),
            ),
        }
        Some(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::vector::Svg;
    use super::*;

    fn metrics(font_size: f64, scale: f32, target: PointMarkerTarget) -> PointMarkerMetrics {
        PointMarkerMetrics::measure(
            font_size,
            TextSettings {
                scale,
                font_size_delta: 0.0,
            },
            target,
        )
        .unwrap()
    }

    #[test]
    fn zero_pixel_spacing_centers_on_native_post_line_cursor_and_base_height() {
        assert_eq!(
            marker_center_y(416.25, 432.0, 45.0, 0.0, None),
            Some(401.625)
        );
        assert_eq!(
            marker_center_y(484.25, 500.0, 100.0, 0.0, None),
            Some(432.5)
        );
        assert_eq!(marker_center_y(500.0, 500.0, 100.0, 0.0, None), Some(432.5));
    }

    #[test]
    fn pixel_spacing_centers_on_baseline_and_actual_default_face_cap_height() {
        let ratio = crate::fonts::FontBook::default()
            .resolve("sans-serif", false, false)
            .unwrap()
            .metrics
            .cap_height_ratio();
        let expected = Some(384.00390625);
        assert_eq!(marker_center_y(400.0, 420.0, 45.0, 6.0, ratio), expected);
        assert_eq!(marker_center_y(400.0, 500.0, 45.0, 90.0, ratio), expected);
        assert_eq!(marker_center_y(400.0, 420.0, 45.0, -6.0, ratio), expected);
        assert_eq!(
            marker_center_y(400.0, 420.0, 100.0, 6.0, ratio),
            Some(364.453125)
        );
        for ratio in [
            None,
            Some(0.0),
            Some(-1.0),
            Some(f64::NAN),
            Some(f64::INFINITY),
        ] {
            assert_eq!(marker_center_y(400.0, 420.0, 45.0, 6.0, ratio), None);
        }
    }

    #[test]
    fn marker_centers_reject_nonfinite_geometry_and_arithmetic_overflow() {
        let native_max = f64::from(f32::MAX);
        for (baseline, cursor, height, spacing) in [
            (f64::NAN, 420.0, 45.0, 0.0),
            (400.0, f64::INFINITY, 45.0, 0.0),
            (400.0, 420.0, f64::INFINITY, 0.0),
            (400.0, 420.0, -1.0, 0.0),
            (400.0, 420.0, 45.0, f64::NAN),
            (400.0, -f64::MAX, f64::MAX, 0.0),
            (-f64::MAX, 420.0, f64::MAX, 1.0),
            (native_max * 2.0, 420.0, 45.0, 0.0),
            (400.0, native_max * 2.0, 45.0, 0.0),
            (400.0, 420.0, native_max * 2.0, 0.0),
            (400.0, 420.0, 45.0, native_max * 2.0),
            (400.0, -native_max, native_max / 2.0, 0.0),
            (-native_max, 420.0, native_max / 2.0, 1.0),
            (400.0, 420.0, native_max, 0.0),
        ] {
            assert_eq!(
                marker_center_y(baseline, cursor, height, spacing, Some(1.0)),
                None
            );
        }
    }

    #[test]
    fn target_profiles_select_native_minimum_and_maximum_images() {
        for (target, min_radius, max_radius) in [
            (PointMarkerTarget::Mobile, 3.0, 8.0),
            (PointMarkerTarget::Tablet, 2.0, 4.0),
            (PointMarkerTarget::Uwp, 7.0, 14.0),
        ] {
            assert_eq!(metrics(0.0, 3.0, target).radius, min_radius);
            assert_eq!(metrics(-45.0, 3.0, target).radius, min_radius);
            assert_eq!(metrics(3.0, 3.0, target).radius, min_radius);
            assert_eq!(metrics(45.0, 3.0, target).radius, max_radius);
            assert_eq!(metrics(450.0, 3.0, target).radius, max_radius);
        }
    }

    #[test]
    fn interpolation_truncates_font_thresholds_and_rounds_image_sizes_away() {
        let mobile = PointMarkerTarget::Mobile;
        for (font, radius) in [
            (1.0, 3.0),
            (2.0, 3.0),
            (3.0, 4.0),
            (4.0, 5.0),
            (5.0, 6.0),
            (6.0, 6.0),
            (7.0, 7.0),
            (8.0, 8.0),
        ] {
            assert_eq!(metrics(font * 3.0, 3.0, mobile).radius, radius);
        }
        assert_eq!(metrics(11.999, 3.0, mobile).radius, 4.0);
        assert_eq!(metrics(12.0, 3.0, mobile).radius, 5.0);
    }

    #[test]
    fn density_changes_reserved_width_independently_of_font_and_rounded_artwork() {
        for (density, radius) in [(1.0, 3.0), (2.0, 5.0), (3.0, 8.0)] {
            let measured = metrics(
                15.0 * f64::from(density),
                density,
                PointMarkerTarget::Mobile,
            );
            assert_eq!(measured.reserved_width, 26.0 * f64::from(density));
            assert_eq!(measured.button_width, 20.0 * f64::from(density));
            assert_eq!(measured.radius, radius);
        }
    }

    #[test]
    fn all_native_point_resources_paint_typed_shapes_with_transparent_open_centers() {
        let measured = metrics(45.0, 3.0, PointMarkerTarget::Mobile);
        let mut scene = Scene::new(Svg::new());
        for marker in [
            PointMarker::SolidCircle,
            PointMarker::OpenCircle,
            PointMarker::SolidSquare,
            PointMarker::OpenSquare,
        ] {
            marker
                .paint(&mut scene, measured, 70.0, 393.625, "#262626")
                .unwrap();
        }
        let output = scene.finish();
        let xml = roxmltree::Document::parse(&output).unwrap();
        let shapes = xml
            .root_element()
            .children()
            .filter(roxmltree::Node::is_element)
            .collect::<Vec<_>>();
        assert_eq!(shapes.len(), 4);
        assert_eq!(shapes[0].tag_name().name(), "circle");
        assert_eq!(shapes[0].attribute("cx"), Some("78.00000"));
        assert_eq!(shapes[0].attribute("cy"), Some("401.62500"));
        assert_eq!(shapes[0].attribute("r"), Some("8.00000"));
        assert_eq!(shapes[0].attribute("fill"), Some("#262626"));
        assert_eq!(shapes[1].attribute("r"), Some("7.00000"));
        assert_eq!(shapes[1].attribute("fill"), Some("none"));
        assert_eq!(shapes[1].attribute("stroke"), Some("#262626"));
        assert_eq!(shapes[1].attribute("stroke-width"), Some("2.00000"));
        assert_eq!(shapes[2].tag_name().name(), "rect");
        assert_eq!(shapes[2].attribute("width"), Some("16.00000"));
        assert_eq!(shapes[3].attribute("x"), Some("71.00000"));
        assert_eq!(shapes[3].attribute("y"), Some("394.62500"));
        assert_eq!(shapes[3].attribute("width"), Some("14.00000"));
        assert_eq!(shapes[3].attribute("fill"), Some("none"));
        assert_eq!(shapes[3].attribute("stroke-width"), Some("2.00000"));
        assert!(!output.contains("<text"));
    }

    #[test]
    fn white_open_markers_keep_transparent_centers_and_none_tint_has_no_ink() {
        let measured = metrics(45.0, 3.0, PointMarkerTarget::Mobile);
        for marker in [PointMarker::OpenCircle, PointMarker::OpenSquare] {
            for color in ["#ffffff", "none"] {
                let mut scene = Scene::new(Svg::new().width(20).height(20));
                marker.paint(&mut scene, measured, 2.0, 2.0, color).unwrap();
                let tree =
                    resvg::usvg::Tree::from_str(&scene.finish(), &resvg::usvg::Options::default())
                        .unwrap();
                let mut image = resvg::tiny_skia::Pixmap::new(20, 20).unwrap();
                resvg::render(
                    &tree,
                    resvg::tiny_skia::Transform::identity(),
                    &mut image.as_mut(),
                );
                assert_eq!(image.pixel(10, 10).unwrap().alpha(), 0);
                if color == "none" {
                    assert!(image.pixels().iter().all(|pixel| pixel.alpha() == 0));
                } else {
                    let pixel = image.pixel(2, 10).unwrap();
                    assert!(pixel.alpha() > 0);
                    assert_eq!(pixel.red(), pixel.alpha());
                    assert_eq!(pixel.green(), pixel.alpha());
                    assert_eq!(pixel.blue(), pixel.alpha());
                }
            }
        }
    }

    #[test]
    fn invalid_metrics_or_paint_geometry_produce_no_svg_nodes() {
        for (font, scale) in [
            (f64::INFINITY, 3.0),
            (f64::NAN, 3.0),
            (45.0, f32::INFINITY),
            (45.0, 0.0),
            (45.0, -1.0),
            (45.0, f32::MAX),
        ] {
            assert!(
                PointMarkerMetrics::measure(
                    font,
                    TextSettings {
                        scale,
                        font_size_delta: 0.0
                    },
                    PointMarkerTarget::Mobile
                )
                .is_none()
            );
        }
        let measured = metrics(45.0, 3.0, PointMarkerTarget::Mobile);
        let mut scene = Scene::new(Svg::new());
        for (left, top, color) in [
            (f64::INFINITY, 0.0, "#262626"),
            (0.0, f64::NAN, "#262626"),
            (f64::MAX, f64::MAX, "#262626"),
            (f64::from(f32::MAX) * 2.0, 0.0, "#262626"),
            (0.0, f64::from(f32::MAX) * 2.0, "#262626"),
            (0.0, 0.0, "invalid"),
        ] {
            assert!(
                PointMarker::SolidCircle
                    .paint(&mut scene, measured, left, top, color)
                    .is_none()
            );
        }
        let output = scene.finish();
        let xml = roxmltree::Document::parse(&output).unwrap();
        assert!(!xml.root_element().children().any(|node| node.is_element()));
    }

    #[test]
    fn saved_bullet_types_select_native_resource_shapes() {
        assert_eq!(
            PointMarker::from_bullet(BulletType::Arrow),
            Some(PointMarker::SolidCircle)
        );
        assert_eq!(
            PointMarker::from_bullet(BulletType::Diamond),
            Some(PointMarker::SolidCircle)
        );
        assert_eq!(
            PointMarker::from_bullet(BulletType::WhiteCircle),
            Some(PointMarker::OpenCircle)
        );
        assert!(PointMarker::from_bullet(BulletType::Digit).is_none());
        assert!(PointMarker::from_bullet(BulletType::Checker).is_none());
    }

    #[test]
    #[cfg(feature = "serde")]
    fn target_configuration_uses_lowercase_names() {
        assert_eq!(
            serde_json::to_string(&PointMarkerTarget::Mobile).unwrap(),
            "\"mobile\""
        );
        assert_eq!(
            serde_json::from_str::<PointMarkerTarget>("\"tablet\"").unwrap(),
            PointMarkerTarget::Tablet
        );
        assert_eq!(
            serde_json::from_str::<PointMarkerTarget>("\"uwp\"").unwrap(),
            PointMarkerTarget::Uwp
        );
    }
}
