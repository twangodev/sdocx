use super::PointMarkerTarget;
use crate::render::text::{TextSettings, finite_native_geometry};
use crate::render::vector::{
    Data, Group, LineCap, LineJoin, Paint, Path, Scene, Styled, Transform,
};

#[derive(Debug, Clone, Copy)]
pub(in crate::render) struct CheckboxMarker {
    pub reserved_width: f64,
    pub button_width: f64,
    pub radius: f64,
    pub asset_side: f64,
    checked: bool,
}

impl CheckboxMarker {
    pub fn measure(
        font_size: f64,
        settings: TextSettings,
        target: PointMarkerTarget,
        checked: bool,
    ) -> Option<Self> {
        let font_size = font_size as f32;
        let density = settings.scale;
        if !font_size.is_finite() || !density.is_finite() || density <= 0.0 {
            return None;
        }
        let minimum = match target {
            PointMarkerTarget::Mobile | PointMarkerTarget::Tablet => 5.5,
            PointMarkerTarget::Uwp => 4.0,
        } * density;
        let maximum = 20.0_f32 * density;
        let button_width = maximum;
        let reserved_width = button_width + 6.0_f32 * density;
        if !maximum.is_finite() || !reserved_width.is_finite() {
            return None;
        }
        let ink_size = (font_size * 68.0 / 100.0).clamp(minimum, maximum) as i32;
        let viewport_side = ((ink_size as f32 * 24.0) / 15.0).ceil() as i32;
        let radius = ((viewport_side as f32) / 2.0).ceil();
        let asset_side = 2.0_f32 * radius;
        Some(Self {
            reserved_width: f64::from(reserved_width),
            button_width: f64::from(button_width),
            radius: f64::from(radius),
            asset_side: f64::from(asset_side),
            checked,
        })
    }

    pub fn bounds(&self, x: f64, center_y: f64) -> Option<crate::BoundingBox> {
        finite_native_geometry(x)?;
        finite_native_geometry(center_y)?;
        let center_x = x + self.button_width / 2.0;
        let center_y = center_y - 1.0;
        super::marker_bounds(
            center_x - self.radius,
            center_y - self.radius,
            center_x + self.radius,
            center_y + self.radius,
        )
    }

    pub fn paint(&self, svg: &mut Scene, x: f64, center_y: f64, color: &str) -> Option<()> {
        let bounds = self.bounds(x, center_y)?;
        let scale = self.asset_side / 24.0;
        finite_native_geometry(scale)?;
        let paint = Paint::from_hex(color)?;
        svg.scope(
            Group::new().transformed(Transform::translate(bounds.x_min, bounds.y_min, 8)),
            |svg| {
                svg.scope(
                    Group::new().transformed(Transform::scale(scale, scale, 8)),
                    |svg| {
                        svg.push(
                            Path::new()
                                .data(checkbox_data(self.checked))
                                .fill(Paint::None)
                                .stroke(paint)
                                .stroke_width(1.5)
                                .stroke_opacity(if self.checked { 0.4 } else { 1.0 })
                                .line_cap(LineCap::Round)
                                .line_join(LineJoin::Round),
                        );
                    },
                );
            },
        );
        Some(())
    }
}

fn checkbox_data(checked: bool) -> Data {
    let outline = Data::new()
        .move_to((17.0, 19.5))
        .line_to((7.0, 19.5))
        .cubic_curve_to((5.625, 19.5, 4.5, 18.375, 4.5, 17.0))
        .line_to((4.5, 7.0))
        .cubic_curve_to((4.5, 5.625, 5.625, 4.5, 7.0, 4.5))
        .line_to((17.0, 4.5))
        .cubic_curve_to((18.375, 4.5, 19.5, 5.625, 19.5, 7.0))
        .line_to((19.5, 17.0))
        .cubic_curve_to((19.5, 18.375, 18.375, 19.5, 17.0, 19.5))
        .close();
    if checked {
        outline
            .move_to((15.988, 9.686))
            .line_to((11.483, 14.205))
            .cubic_curve_to((11.336, 14.351, 11.098, 14.351, 10.952, 14.205))
            .line_to((8.013, 11.257))
    } else {
        outline
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::vector::Svg;

    fn marker(
        font_size: f64,
        density: f32,
        target: PointMarkerTarget,
        checked: bool,
    ) -> CheckboxMarker {
        CheckboxMarker::measure(
            font_size,
            TextSettings {
                scale: density,
                font_size_delta: 900.0,
                ..Default::default()
            },
            target,
            checked,
        )
        .unwrap()
    }

    #[test]
    fn native_sizing_truncates_percent_before_viewport_and_even_rounding() {
        for (font, density, target, side) in [
            (17.0, 1.0, PointMarkerTarget::Mobile, 18.0),
            (45.0, 3.0, PointMarkerTarget::Mobile, 48.0),
            (1.0, 1.0, PointMarkerTarget::Mobile, 8.0),
            (1.0, 1.0, PointMarkerTarget::Tablet, 8.0),
            (1.0, 1.0, PointMarkerTarget::Uwp, 8.0),
            (1.0, 3.0, PointMarkerTarget::Mobile, 26.0),
            (1.0, 3.0, PointMarkerTarget::Uwp, 20.0),
            (1000.0, 3.0, PointMarkerTarget::Mobile, 96.0),
            (13.2352933883667, 1.0, PointMarkerTarget::Mobile, 14.0),
            (26.4705867767334, 1.0, PointMarkerTarget::Mobile, 28.0),
        ] {
            let marker = marker(font, density, target, false);
            assert_eq!(marker.asset_side, side);
            assert_eq!(marker.radius * 2.0, side);
            assert_eq!(marker.button_width, f64::from(20.0_f32 * density));
            assert_eq!(marker.reserved_width, f64::from(26.0_f32 * density));
        }
    }

    #[test]
    fn compound_paths_match_pinned_native_resources_with_one_alpha_application() {
        let outline = "M17,19.5L7,19.5C5.625,19.5 4.5,18.375 4.5,17L4.5,7C4.5,5.625 5.625,4.5 7,4.5L17,4.5C18.375,4.5 19.5,5.625 19.5,7L19.5,17C19.5,18.375 18.375,19.5 17,19.5Z";
        let tick =
            "M15.988,9.686L11.483,14.205C11.336,14.351 11.098,14.351 10.952,14.205L8.013,11.257";
        for checked in [false, true] {
            let marker = marker(45.0, 3.0, PointMarkerTarget::Mobile, checked);
            let mut scene = Scene::new(Svg::new());
            marker.paint(&mut scene, 48.0, 401.625, "#123456").unwrap();
            let output = scene.finish();
            let document = roxmltree::Document::parse(&output).unwrap();
            let paths = document
                .descendants()
                .filter(|node| node.has_tag_name("path"))
                .collect::<Vec<_>>();
            assert_eq!(paths.len(), 1);
            let path = paths[0];
            let expected = if checked {
                format!("{outline}{tick}")
            } else {
                outline.to_owned()
            };
            let geometry = |path: &str| {
                svgtypes::PathParser::from(path)
                    .map(|segment| match segment.unwrap() {
                        svgtypes::PathSegment::ClosePath { .. } => {
                            svgtypes::PathSegment::ClosePath { abs: true }
                        }
                        segment => segment,
                    })
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                geometry(path.attribute("d").unwrap()),
                geometry(expected.as_str())
            );
            assert_eq!(path.attribute("fill"), Some("none"));
            assert_eq!(path.attribute("stroke"), Some("#123456"));
            assert_eq!(path.attribute("stroke-width"), Some("1.5"));
            assert_eq!(path.attribute("stroke-linecap"), Some("round"));
            assert_eq!(path.attribute("stroke-linejoin"), Some("round"));
            assert_eq!(
                path.attribute("stroke-opacity"),
                Some(if checked { "0.4" } else { "1" })
            );
            assert!(!output.contains("<image"));
            assert!(!output.contains("<text"));
        }
    }

    #[test]
    fn viewport_uses_button_center_and_native_unscaled_minus_one_y_offset() {
        for density in [1.0, 2.0, 3.0] {
            let marker = marker(17.0, density, PointMarkerTarget::Mobile, false);
            let mut scene = Scene::new(Svg::new());
            marker.paint(&mut scene, 48.0, 401.625, "#262626").unwrap();
            let output = scene.finish();
            let document = roxmltree::Document::parse(&output).unwrap();
            let transforms = document
                .descendants()
                .filter(|node| node.has_tag_name("g"))
                .map(|node| node.attribute("transform").unwrap())
                .collect::<Vec<_>>();
            assert_eq!(
                transforms[0],
                format!(
                    "translate({} {:.8})",
                    48.0 + marker.button_width / 2.0 - marker.radius,
                    400.625 - marker.radius
                )
            );
            assert_eq!(
                transforms[1],
                format!(
                    "scale({} {:.8})",
                    marker.asset_side / 24.0,
                    marker.asset_side / 24.0
                )
            );
        }
    }

    #[test]
    fn invalid_native_geometry_emits_nothing_without_guessing_minimum_artwork_size() {
        for (font, density) in [
            (f64::NAN, 1.0),
            (f64::INFINITY, 1.0),
            (1e308, 1.0),
            (17.0, 0.0),
            (17.0, -1.0),
            (17.0, f32::NAN),
            (17.0, f32::MAX),
        ] {
            assert!(
                CheckboxMarker::measure(
                    font,
                    TextSettings {
                        scale: density,
                        font_size_delta: 0.0,
                        ..Default::default()
                    },
                    PointMarkerTarget::Mobile,
                    false
                )
                .is_none()
            );
        }
        let tiny = marker(1.0, f32::from_bits(1), PointMarkerTarget::Mobile, false);
        assert_eq!(tiny.asset_side, 0.0);
        let mut scene = Scene::new(Svg::new());
        assert!(tiny.paint(&mut scene, 0.0, 0.0, "#262626").is_none());
        let normal = marker(17.0, 1.0, PointMarkerTarget::Mobile, false);
        for (x, center_y, color) in [
            (f64::NAN, 0.0, "#262626"),
            (0.0, 1e308, "#262626"),
            (0.0, 0.0, "invalid"),
        ] {
            assert!(normal.paint(&mut scene, x, center_y, color).is_none());
        }
        assert!(!scene.finish().contains("<path"));
    }
}
