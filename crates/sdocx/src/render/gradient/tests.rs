use super::super::vector::{Rectangle, Styled, Svg};
use super::*;
use crate::{GradientStopSource, PaintFloat32};

const RECT: [f32; 4] = [10.25, 20.5, 310.25, 70.5];

fn source(positions: &[f32]) -> ColorPaintSource {
    ColorPaintSource {
        property_flags: 1,
        outline_color_type: None,
        solid_argb: 0,
        gradient_type: 0,
        linear_angle: 0,
        position: [PaintFloat32::from_bits(0x7fc12345); 2],
        stops: positions
            .iter()
            .enumerate()
            .map(|(index, p)| GradientStopSource {
                argb: 0x80123456 + index as u32,
                position: PaintFloat32::from_bits(p.to_bits()),
            })
            .collect(),
        trailing_data: Vec::new(),
    }
}

#[test]
fn linear_geometry_matches_hash_locked_native_helper_capture() {
    // linear.json d109f7d515f3bae10525501b9bd3dc8994425159aa0f6fc6c221b28427a371b6.
    // Full helper executes with hosted Linux libm; allow two f32 ULPs for
    // host transcendental rounding. Horizontal endpoints are deterministic.
    for (angle, expected) in [
        (0, [0x41240000, 0x42360000, 0x439b2000, 0x42360000]),
        (30, [0x42e9e5c0, 0x41a40000, 0x434b8d20, 0x428d0000]),
        (90, [0x43204000, 0x41a40000, 0x43204000, 0x428d0000]),
        (179, [0x439b2000, 0x422b86e5, 0x41240000, 0x4240791b]),
        (180, [0x439b2000, 0x42360000, 0x41240000, 0x42360000]),
        (270, [0x43204000, 0x428d0000, 0x43204000, 0x41a40000]),
        (359, [0x41240000, 0x4240791b, 0x439b2000, 0x422b86e5]),
        (360, [0x41240000, 0x42360000, 0x439b2000, 0x42360000]),
        (32767, [0x41240000, 0x41d8a895, 0x439b2000, 0x427fabb5]),
        (65535, [0x41240000, 0x40a9d806, 0x439b2000, 0x42ab6280]),
    ] {
        for (actual, expected) in linear_points(RECT, angle)
            .into_iter()
            .flatten()
            .zip(expected)
        {
            let tolerance = if matches!(angle, 0 | 180 | 360) { 0 } else { 2 };
            assert!(
                actual.to_bits().abs_diff(expected) <= tolerance,
                "angle {angle}: {actual} vs {}",
                f32::from_bits(expected)
            );
        }
    }
}

#[test]
fn radial_geometry_matches_reviewed_native_dispatcher_capture() {
    // gradient-reviewed.json 30c9be10e1657208277d7e6f2a3529e3b55003816896142de25ed6c8b670f222.
    // Native Model effect + Drawing radial dispatcher + Skia constructor,
    // supplied common rectangle/rotation, null theme, hosted Linux sincos.
    for (position, rotation, rotatable, center, radius, tolerance) in [
        (
            [0x3f000000, 0x3f000000],
            0.0,
            false,
            [1126187008, 1110835200],
            1125650862,
            0,
        ),
        (
            [0x3e800000, 0x3f400000],
            0.0,
            false,
            [1118470144, 1114112000],
            1134039470,
            0,
        ),
        (
            [0x3e800000, 0x3f400000],
            37.25,
            true,
            [1119483812, 1092671740],
            1134039470,
            2,
        ),
        (
            [0x3e800000, 0x3f400000],
            -37.25,
            true,
            [1121467240, 1120514484],
            1134039470,
            2,
        ),
        (
            [0x3e800000, 0x3f400000],
            37.25,
            false,
            [1118470144, 1114112000],
            1134039470,
            0,
        ),
        (
            [0x3f000000, 0x3f000000],
            37.25,
            true,
            [1126187008, 1110835200],
            1125650862,
            0,
        ),
        (
            [0x3f000001, 0x3f000000],
            0.0,
            false,
            [1126187009, 1110835200],
            1125650862,
            0,
        ),
        (
            [0x3f000002, 0x3f000000],
            0.0,
            false,
            [1126187010, 1110835200],
            1134039470,
            0,
        ),
    ] {
        let (actual, actual_radius) =
            radial_points(RECT, position.map(f32::from_bits), rotation, rotatable);
        assert_eq!(actual_radius.to_bits(), radius);
        for (actual, expected) in actual.into_iter().zip(center) {
            assert!(actual.to_bits().abs_diff(expected) <= tolerance);
        }
    }
}

#[test]
fn rotation_has_one_addition_and_strict_positive_gate() {
    let point = [85.25, 58.0];
    let center = [160.25, 45.5];
    for angle in [0.0, ROTATION_GATE, -360.0, -400.0] {
        assert_eq!(rotate_point(point, center, angle, true), point);
    }
    // The centered tolerance branch skips rotation even one f32 step off center.
    assert_eq!(
        radial_points(RECT, [f32::from_bits(0x3f000001), 0.5], 37.25, true),
        radial_points(RECT, [f32::from_bits(0x3f000001), 0.5], 0.0, false)
    );
}

#[test]
fn stops_keep_duplicates_quantize_and_pad_without_changing_source() {
    let source = source(&[0.100001, 0.5, 0.5, 0.9]);
    let original = source.clone();
    let plan = project_stops(&source).unwrap();
    assert_eq!(
        plan.iter().map(|s| s.offset).collect::<Vec<_>>(),
        [0.0, 6553.0 / 65536.0, 0.5, 0.5, 58982.0 / 65536.0, 1.0]
    );
    assert_eq!(
        plan.iter().map(|s| s.argb).collect::<Vec<_>>(),
        [
            0x80123456, 0x80123456, 0x80123457, 0x80123458, 0x80123459, 0x80123459
        ]
    );
    assert_eq!(source, original);
}

#[test]
fn first_ten_and_singleton_ignore_only_unconsumed_positions() {
    let singleton = source(&[f32::NAN]);
    assert_eq!(
        project_stops(&singleton).unwrap(),
        [
            ProjectedStop {
                offset: 0.0,
                argb: 0x80123456
            },
            ProjectedStop {
                offset: 1.0,
                argb: 0x80123456
            }
        ]
    );
    let mut many = source(&[0.0; 11]);
    many.stops[10].position = PaintFloat32::from_bits(0x7fc12345);
    let plan = project_stops(&many).unwrap();
    assert_eq!(plan.len(), 11); // Ten saved zeros followed by the copied endpoint.
    assert_eq!(plan.last().unwrap().argb, many.stops[9].argb);
    many.stops[9].position = PaintFloat32::from_bits(0x7fc12345);
    assert!(matches!(
        project_stops(&many),
        Err(PaintDiagnosticKind::InvalidGradient)
    ));
    for positions in [
        &[][..],
        &[0.8, 0.2],
        &[0.0, f32::INFINITY],
        &[-0.1, 1.0],
        &[0.0, 1.1],
    ] {
        assert!(matches!(
            project_stops(&source(positions)),
            Err(PaintDiagnosticKind::InvalidGradient)
        ));
    }
}

#[test]
fn svg_stops_keep_hidden_rgb_alpha_and_inverse_element_rotation() {
    for radial in [false, true] {
        let geometry = if radial {
            Geometry::Radial {
                center: [160.25, 45.5],
                radius: 152.06906,
            }
        } else {
            Geometry::Linear([[10.25, 45.5], [310.25, 45.5]])
        };
        let plan = Plan {
            geometry,
            stops: vec![
                ProjectedStop {
                    offset: 0.0,
                    argb: 0x00123456,
                },
                ProjectedStop {
                    offset: 1.0,
                    argb: 0x80123456,
                },
            ],
            element_rotation: Some(ElementRotation {
                angle: 37.25,
                center: [160.2500000001, 45.5000000001],
            }),
        };
        let mut scene = Scene::new(Svg::new());
        let paint = plan.paint(&mut scene, RenderTheme::for_canvas(false));
        scene.push(
            Rectangle::new()
                .width(300.0)
                .height(50.0)
                .fill(paint)
                .fill_opacity(1.0),
        );
        let svg = scene.finish();
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let gradient = xml
            .descendants()
            .find(|n| {
                n.has_tag_name(if radial {
                    "radialGradient"
                } else {
                    "linearGradient"
                })
            })
            .unwrap();
        assert_eq!(gradient.attribute("gradientUnits"), Some("userSpaceOnUse"));
        assert_eq!(gradient.attribute("spreadMethod"), None); // SVG default pad matches tile0.
        assert_eq!(
            gradient.attribute("gradientTransform"),
            Some("rotate(-37.25 160.2500000001 45.5000000001)")
        );
        let stops = gradient
            .children()
            .filter(|n| n.has_tag_name("stop"))
            .collect::<Vec<_>>();
        assert_eq!(stops[0].attribute("stop-color"), Some("#123456"));
        assert_eq!(stops[0].attribute("stop-opacity"), Some("0"));
        assert_eq!(
            stops[1]
                .attribute("stop-opacity")
                .unwrap()
                .parse::<f64>()
                .unwrap(),
            128.0 / 255.0
        );
        assert!(
            xml.descendants()
                .any(|n| n.has_tag_name("rect") && n.attribute("fill-opacity") == Some("1"))
        );
    }
}

fn shape() -> NativeShape {
    let bbox = BoundingBox {
        x_min: 10.25,
        y_min: 20.5,
        x_max: 310.25,
        y_max: 70.5,
    };
    NativeShape {
        horizontal_flip: false,
        vertical_flip: false,
        text_editable: false,
        text_area_type: None,
        metadata: crate::ObjectMetadata {
            format_version: 5500,
            uuid: "gradient".into(),
            modified_time_raw: 0,
            bbox,
            replay_timestamp_raw: 0,
            resize_mode_raw: 0,
            rotatable: false,
            selectable: false,
            movable: false,
            visible: true,
            replayable: false,
            out_of_canvas_enabled: false,
            template: false,
            flip_enabled: false,
            float_drawn_rect: false,
            locked: false,
            removable: true,
            rotation_degrees: None,
            property_mask: vec![],
            field_mask: vec![],
            fixed_trailing_data: vec![],
            flexible_trailing_data: vec![],
        },
        base_source: None,
        shape_type: 4,
        geometry_bbox: bbox,
        drawn_bbox: bbox,
        rotation_degrees: 0.0,
        control_points: vec![],
        path_data: vec![],
        style: Default::default(),
        fill: crate::ShapePaint::Gradient,
        fill_source: None,
        pen_name_id: None,
        pen_data_field_3_raw: None,
        pen_settings_id: None,
        text: None,
    }
}

#[test]
fn plan_admits_only_ordinary_finite_frame_and_validates_before_allocating_ids() {
    let mut shape = shape();
    let source = source(&[0.0, 1.0]);
    shape.metadata.bbox.x_min += 1e-10; // Source restoration compares f32.
    assert!(Plan::for_shape(&shape, &source, PaintRole::Fill, None).is_ok());
    shape.metadata.bbox.x_min = 11.0;
    assert!(matches!(
        Plan::for_shape(&shape, &source, PaintRole::Fill, None),
        Err(PaintDiagnosticKind::UnsupportedGradientFrame)
    ));
    shape.metadata.bbox = shape.drawn_bbox;
    shape.metadata.rotation_degrees = Some(1.0);
    assert!(matches!(
        Plan::for_shape(&shape, &source, PaintRole::Fill, None),
        Err(PaintDiagnosticKind::UnsupportedGradientFrame)
    ));
    shape.metadata.rotation_degrees = Some(0.0);
    for bbox in [
        BoundingBox {
            x_max: f64::MAX,
            ..shape.geometry_bbox
        },
        BoundingBox {
            x_max: shape.geometry_bbox.x_min,
            ..shape.geometry_bbox
        },
    ] {
        shape.geometry_bbox = bbox;
        assert!(matches!(
            Plan::for_shape(&shape, &source, PaintRole::Fill, None),
            Err(PaintDiagnosticKind::UnrepresentableGradient)
        ));
    }
    shape.geometry_bbox = shape.drawn_bbox;
    assert!(matches!(
        Plan::for_shape(
            &shape,
            &source,
            PaintRole::Fill,
            Some(ElementRotation {
                angle: f64::NAN,
                center: [0.0, 0.0]
            })
        ),
        Err(PaintDiagnosticKind::UnrepresentableGradient)
    ));
    let mut radial = source.clone();
    radial.gradient_type = 1; // The dormant NaN coordinates become active.
    assert!(matches!(
        Plan::for_shape(&shape, &radial, PaintRole::Fill, None),
        Err(PaintDiagnosticKind::UnrepresentableGradient)
    ));
    let mut scene = Scene::new(Svg::new());
    let paint = Plan::for_shape(&shape, &source, PaintRole::Fill, None)
        .unwrap()
        .paint(&mut scene, RenderTheme::for_canvas(false));
    scene.push(Rectangle::new().width(1.0).height(1.0).fill(paint));
    assert!(scene.finish().contains("fill=\"url(#sdocx-def-0)\""));
}

#[test]
fn plan_checks_active_source_role_but_preserves_proven_flag_semantics() {
    let shape = shape();
    let mut source = source(&[0.0, 1.0]);
    source.property_flags = 0x83;
    assert!(Plan::for_shape(&shape, &source, PaintRole::Fill, None).is_ok());
    assert!(matches!(
        Plan::for_shape(&shape, &source, PaintRole::Outline, None),
        Err(PaintDiagnosticKind::UnsupportedPaint)
    ));
    source.outline_color_type = Some(1);
    assert!(Plan::for_shape(&shape, &source, PaintRole::Outline, None).is_ok());
    assert!(matches!(
        Plan::for_shape(&shape, &source, PaintRole::Fill, None),
        Err(PaintDiagnosticKind::UnsupportedPaint)
    ));
    source.trailing_data.push(0);
    assert!(matches!(
        Plan::for_shape(&shape, &source, PaintRole::Outline, None),
        Err(PaintDiagnosticKind::UnsupportedPaint)
    ));
}

#[test]
fn unconsumed_overflow_center_does_not_reject_finite_paint() {
    let mut shape = shape();
    shape.geometry_bbox.x_min = 2e38;
    shape.geometry_bbox.x_max = 3e38;
    let source = source(&[0.0, 1.0]);
    assert!(Plan::for_shape(&shape, &source, PaintRole::Fill, None).is_ok());
    shape.rotation_degrees = 90.0;
    let mut rotated = source;
    rotated.property_flags = 3;
    assert!(matches!(
        Plan::for_shape(&shape, &rotated, PaintRole::Fill, None),
        Err(PaintDiagnosticKind::UnrepresentableGradient)
    ));
}

#[test]
fn collapsed_rotated_axis_has_no_admitted_svg_fallback() {
    // Constructed exporter-capability regression, not a native capture.
    let mut shape = shape();
    shape.geometry_bbox = BoundingBox {
        x_min: 1073741824.0,
        y_min: 0.0,
        x_max: 1073741952.0,
        y_max: 1.0,
    };
    let mut source = source(&[0.0, 1.0]);
    source.linear_angle = 90;
    assert!(Plan::for_shape(&shape, &source, PaintRole::Fill, None).is_ok());
    shape.rotation_degrees = 90.0;
    source.property_flags = 3;
    assert!(matches!(
        Plan::for_shape(&shape, &source, PaintRole::Fill, None),
        Err(PaintDiagnosticKind::UnrepresentableGradient)
    ));
}
