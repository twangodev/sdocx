use super::super::vector::{Rectangle, Styled, Svg};
use super::*;
use crate::{
    GradientStopSource, NativeLine, PaintFloat32, ShapePaint, ShapePaintSource, ShapeStyle,
};

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

fn line(reference_bbox: BoundingBox) -> NativeLine {
    let other_bbox = BoundingBox {
        x_min: reference_bbox.x_min - 6.44654846,
        y_min: reference_bbox.y_min - 6.44656372,
        x_max: reference_bbox.x_max + 6.44654846,
        y_max: reference_bbox.y_max + 6.44656372,
    };
    NativeLine {
        metadata: crate::ObjectMetadata {
            format_version: 5500,
            uuid: "type8-gradient".into(),
            modified_time_raw: 0,
            bbox: other_bbox,
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
        line_type: 0,
        routing: 0,
        control_points: vec![],
        begin: [reference_bbox.x_min, reference_bbox.y_max],
        end: [reference_bbox.x_max, reference_bbox.y_min],
        geometry_bbox: other_bbox,
        reference_bbox,
        raw_setting: 0,
        pen_name_id: None,
        legacy_pen_source: None,
        pen_settings_id: None,
        path_data: vec![],
        style: ShapeStyle {
            width: f32::from_bits(0x4111_de6a),
            ..ShapeStyle::default()
        },
    }
}

fn line_source() -> ColorPaintSource {
    let mut source = source(&[0.0, 1.0]);
    source.outline_color_type = Some(1);
    source.property_flags = 0;
    source
}

fn test_line() -> NativeLine {
    line(BoundingBox {
        x_min: f64::from(RECT[0]),
        y_min: f64::from(RECT[1]),
        x_max: f64::from(RECT[2]),
        y_max: f64::from(RECT[3]),
    })
}

fn radial_line_source() -> ColorPaintSource {
    let mut source = line_source();
    source.gradient_type = 1;
    source.position = [0.25_f32, 0.75_f32].map(|value| PaintFloat32::from_bits(value.to_bits()));
    source
}

fn captured_rotatable_radial_line_source() -> ColorPaintSource {
    let mut source = radial_line_source();
    source.property_flags = 1;
    source.stops = vec![
        GradientStopSource {
            argb: 0xffff_0000,
            position: PaintFloat32::from_bits(0),
        },
        GradientStopSource {
            argb: 0xff00_00ff,
            position: PaintFloat32::from_bits(1.0_f32.to_bits()),
        },
    ];
    source
}

pub(in crate::render) fn hf02_line_with_gradient() -> NativeLine {
    let mut line = line(BoundingBox {
        x_min: 97.64480590820312,
        y_min: 434.66650390625,
        x_max: 222.5447540283203,
        y_max: 572.0924682617188,
    });
    line.style.paint = ShapePaint::Gradient;
    line.style.paint_source = Some(ShapePaintSource::Color(line_source()));
    line
}

#[test]
fn line_linear_gradient_uses_native_reference_bbox_not_other_saved_bounds() {
    // V21's actual Skia CreateLinear arguments pin these endpoint words and
    // the HF02 reference RectF. Its paint is synthetic, so this locks frame
    // selection rather than saved-archive appearance or native pixel parity.
    let frame = BoundingBox {
        x_min: 97.64480590820312,
        y_min: 434.66650390625,
        x_max: 222.5447540283203,
        y_max: 572.0924682617188,
    };
    let line = line(frame);
    let plan = Plan::for_line(&line, &line_source()).unwrap();
    let Geometry::Linear(points) = plan.geometry else {
        panic!("type-8 outline must use linear geometry");
    };
    assert_eq!(
        points.map(|point| point.map(f32::to_bits)),
        [[0x42c34a24, 0x43fbb093], [0x435e8b75, 0x43fbb093]]
    );
    assert_eq!(plan.stops.len(), 2);
}

#[test]
fn line_linear_gradient_is_independent_of_endpoint_direction() {
    let frame = BoundingBox {
        x_min: 97.64480590820312,
        y_min: 434.66650390625,
        x_max: 222.5447540283203,
        y_max: 572.0924682617188,
    };
    let mut line = line(frame);
    let source = line_source();
    let Geometry::Linear(forward) = Plan::for_line(&line, &source).unwrap().geometry else {
        unreachable!();
    };
    std::mem::swap(&mut line.begin, &mut line.end);
    let Geometry::Linear(reversed) = Plan::for_line(&line, &source).unwrap().geometry else {
        unreachable!();
    };
    assert_eq!(
        forward.map(|point| point.map(f32::to_bits)),
        reversed.map(|point| point.map(f32::to_bits))
    );
}

#[test]
fn rotatable_linear_line_flag_matches_pinned_native_zero_rotation_frame() {
    // V22 native result SHA-256:
    // d30162b9379963344b37a58b18df6628dfdb0625737dd2d409d5c3f0f305ffd7
    // ApplyBinary changed only property byte 1 from V21's 0 to 1. The native
    // helper bool changed 0->1 while object rotation stayed +0; Skia received
    // the same four point words and the complete 1,638,400-byte frame matched.
    let frame = BoundingBox {
        x_min: f64::from(f32::from_bits(0x42c3_4a24)),
        y_min: f64::from(f32::from_bits(0x43d9_5550)),
        x_max: f64::from(f32::from_bits(0x435e_8b75)),
        y_max: f64::from(f32::from_bits(0x440f_05eb)),
    };
    let mut control = line_source();
    control.position = [0.5_f32; 2].map(|value| PaintFloat32::from_bits(value.to_bits()));
    control.stops = vec![
        GradientStopSource {
            argb: 0xffff_0000,
            position: PaintFloat32::from_bits(0),
        },
        GradientStopSource {
            argb: 0xff00_00ff,
            position: PaintFloat32::from_bits(1.0_f32.to_bits()),
        },
    ];
    let mut rotatable = control.clone();
    rotatable.property_flags = 1;

    let control_plan = Plan::for_line(&line(frame), &control).unwrap();
    let rotatable_plan = Plan::for_line(&line(frame), &rotatable).unwrap();
    let Geometry::Linear(control_points) = control_plan.geometry else {
        panic!("flag-0 control must use linear geometry");
    };
    let Geometry::Linear(points) = rotatable_plan.geometry else {
        panic!("rotatable type-0 outline must use linear geometry");
    };
    let expected = [[0x42c3_4a24, 0x43fb_b093], [0x435e_8b75, 0x43fb_b093]];
    assert_eq!(points.map(|point| point.map(f32::to_bits)), expected);
    assert_eq!(
        points.map(|point| point.map(f32::to_bits)),
        control_points.map(|point| point.map(f32::to_bits))
    );
    assert_eq!(rotatable_plan.stops, control_plan.stops);

    // The helper's angle is zero in this witness. Metadata rotation remains a
    // separate hard gate for both flag values.
    for (rotation, accepted) in [(None, true), (Some(0.0), true), (Some(1.0), false)] {
        let mut candidate = line(frame);
        candidate.metadata.rotation_degrees = rotation;
        let result = Plan::for_line(&candidate, &rotatable);
        if accepted {
            assert!(result.is_ok(), "rotation {rotation:?} should be admitted");
        } else {
            assert!(matches!(
                result,
                Err(PaintDiagnosticKind::UnsupportedGradientFrame)
            ));
        }
    }

    // Native evidence covers exactly property byte 1; retain fallback for
    // higher/other outline property bits on linear as well as radial routes.
    for flags in [2, 0x80, 0xff] {
        let mut unknown = rotatable.clone();
        unknown.property_flags = flags;
        assert!(matches!(
            Plan::for_line(&line(frame), &unknown),
            Err(PaintDiagnosticKind::UnsupportedGradientFrame)
        ));
    }
}

#[test]
fn line_radial_geometry_matches_centered_and_off_center_native_factory_words() {
    // V23 and V24 each enter the real Skia CreateRadial factory on HF02's
    // unchanged reference bbox. V23 uses the centered half-diagonal branch;
    // V24 uses position (.25,.75), the full diagonal radius, and an off-center
    // computed center. Their serialized effect is synthetic, not archive paint.
    let frame = BoundingBox {
        x_min: 97.64480590820312,
        y_min: 434.66650390625,
        x_max: 222.5447540283203,
        y_max: 572.0924682617188,
    };
    for (position, center_words, radius_word) in [
        ([0.5_f32, 0.5], [0x4320_1844, 0x43fb_b093], 0x42b9_b42b),
        ([0.25_f32, 0.75], [0x4300_deab, 0x4406_6f1a], 0x4339_b42b),
    ] {
        let mut source = line_source();
        source.gradient_type = 1;
        source.position = position.map(|value| PaintFloat32::from_bits(value.to_bits()));
        let plan = Plan::for_line(&line(frame), &source).unwrap();
        let Geometry::Radial { center, radius } = plan.geometry else {
            panic!("type-8 radial outline must use radial geometry");
        };
        assert_eq!(center.map(f32::to_bits), center_words);
        assert_eq!(radius.to_bits(), radius_word);
        assert_eq!(plan.stops.len(), 2);
    }
}

#[test]
fn rotatable_radial_line_flag_matches_pinned_native_zero_angle_geometry_and_stops() {
    // Native V2 capture result SHA-256:
    // a715e5e069e797908213b0133fe4654e9472ab7dada40529770d40809d91910d
    // Its exact effect bytes set outline property flag 1, type 1, angle 0,
    // position (.25,.75), red@0 and blue@1. Actual helper/factory geometry
    // and the flag-0 V24 RGBA frame are pinned by that capture.
    let frame = BoundingBox {
        x_min: f64::from(f32::from_bits(0x42c3_4a24)),
        y_min: f64::from(f32::from_bits(0x43d9_5550)),
        x_max: f64::from(f32::from_bits(0x435e_8b75)),
        y_max: f64::from(f32::from_bits(0x440f_05eb)),
    };
    let mut control = captured_rotatable_radial_line_source();
    control.property_flags = 0;
    let rotatable = captured_rotatable_radial_line_source();

    let control_plan = Plan::for_line(&line(frame), &control).unwrap();
    let rotatable_plan = Plan::for_line(&line(frame), &rotatable).unwrap();
    let Geometry::Radial {
        center: control_center,
        radius: control_radius,
    } = control_plan.geometry
    else {
        panic!("flag-0 control must use radial geometry");
    };
    let Geometry::Radial { center, radius } = rotatable_plan.geometry else {
        panic!("rotatable type-1 outline must use radial geometry");
    };
    assert_eq!(center.map(f32::to_bits), [0x4300_deab, 0x4406_6f1a]);
    assert_eq!(radius.to_bits(), 0x4339_b42b);
    assert_eq!(center.map(f32::to_bits), control_center.map(f32::to_bits));
    assert_eq!(radius.to_bits(), control_radius.to_bits());
    assert_eq!(rotatable_plan.stops, control_plan.stops);
    assert_eq!(
        rotatable_plan.stops,
        vec![
            ProjectedStop {
                offset: 0.0,
                argb: 0xffff_0000,
            },
            ProjectedStop {
                offset: 1.0,
                argb: 0xffff_0000,
            },
        ]
    );

    // The existing metadata-rotation gate remains active; a rotatable effect
    // does not authorize a rotated line frame.
    for (rotation, accepted) in [(None, true), (Some(0.0), true), (Some(1.0), false)] {
        let mut line = line(frame);
        line.metadata.rotation_degrees = rotation;
        let result = Plan::for_line(&line, &rotatable);
        if accepted {
            assert!(result.is_ok(), "rotation {rotation:?} should be admitted");
        } else {
            assert!(matches!(
                result,
                Err(PaintDiagnosticKind::UnsupportedGradientFrame)
            ));
        }
    }

    // Only canonical flag 1 was observed; preserve other raw outline bytes
    // without treating them as a native boolean contract.
    for flags in [2, 0x80, 0xff] {
        let mut unknown = rotatable.clone();
        unknown.property_flags = flags;
        assert!(matches!(
            Plan::for_line(&line(frame), &unknown),
            Err(PaintDiagnosticKind::UnsupportedGradientFrame)
        ));
    }
}

#[test]
fn line_radial_geometry_rejects_nonfinite_active_position_before_paint_allocation() {
    let frame = BoundingBox {
        x_min: 97.64480590820312,
        y_min: 434.66650390625,
        x_max: 222.5447540283203,
        y_max: 572.0924682617188,
    };
    let mut radial = radial_line_source();
    radial.position[0] = PaintFloat32::from_bits(f32::NAN.to_bits());
    assert!(matches!(
        Plan::for_line(&line(frame), &radial),
        Err(PaintDiagnosticKind::UnrepresentableGradient)
    ));

    // Position is dormant for a linear line gradient and must not become a
    // new rejection condition on the existing linear route.
    radial.gradient_type = 0;
    assert!(Plan::for_line(&line(frame), &radial).is_ok());
}

#[test]
fn line_gradient_keeps_unproven_frames_and_styles_on_fallback() {
    let frame = BoundingBox {
        x_min: 97.64480590820312,
        y_min: 434.66650390625,
        x_max: 222.5447540283203,
        y_max: 572.0924682617188,
    };
    let source = line_source();
    let mut line = line(frame);
    line.metadata.rotation_degrees = Some(1.0);
    assert!(matches!(
        Plan::for_line(&line, &source),
        Err(PaintDiagnosticKind::UnsupportedGradientFrame)
    ));
    line.metadata.rotation_degrees = None;
    let mut unknown_flags = source.clone();
    unknown_flags.property_flags = 2;
    assert!(matches!(
        Plan::for_line(&line, &unknown_flags),
        Err(PaintDiagnosticKind::UnsupportedGradientFrame)
    ));
    line.style.end_arrow = [1, 1];
    assert!(matches!(
        Plan::for_line(&line, &source),
        Err(PaintDiagnosticKind::UnsupportedGradientFrame)
    ));
    line.style.end_arrow = [0; 2];
    line.reference_bbox.x_max = line.reference_bbox.x_min;
    assert!(matches!(
        Plan::for_line(&line, &source),
        Err(PaintDiagnosticKind::UnsupportedGradientFrame)
    ));
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
fn outline_copy_sorts_all_stops_before_applying_the_native_ten_stop_cap() {
    let many = source(&[0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0]);
    let original = many.clone();
    let normalized = normalize_outline_stops(&many).unwrap();
    assert_eq!(
        normalized
            .iter()
            .map(|stop| stop.position.value())
            .collect::<Vec<_>>(),
        [1.0, 0.9, 0.8, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1, 0.0]
    );
    assert_eq!(normalized[0].argb, many.stops[10].argb);
    assert_eq!(many, original);

    let mut capped_positions = vec![0.0; 10];
    capped_positions.push(1.0);
    let mut capped = source(&capped_positions);
    for stop in &mut capped.stops[..10] {
        stop.argb = 0xff12_3456;
    }
    capped.stops[10].argb = 0xff65_4321;
    assert_eq!(
        project_outline_stops(&capped).unwrap(),
        [
            ProjectedStop {
                offset: 0.0,
                argb: 0xff12_3456,
            },
            ProjectedStop {
                offset: 1.0,
                argb: 0xff12_3456,
            },
        ]
    );
}

#[test]
fn outline_copy_rejects_only_conflicting_ties_that_reach_the_consumed_prefix() {
    let crossing = source(&[1.0, 0.9, 0.8, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.0, 0.0]);
    assert!(matches!(
        normalize_outline_stops(&crossing),
        Err(PaintDiagnosticKind::InvalidGradient)
    ));

    let beyond = source(&[1.0, 0.9, 0.8, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1, 0.0, 0.0]);
    assert!(normalize_outline_stops(&beyond).is_ok());

    let mut identical = source(&[0.0, 0.0, 1.0]);
    identical.stops[1].argb = identical.stops[0].argb;
    let normalized = normalize_outline_stops(&identical).unwrap();
    assert_eq!(normalized.len(), 3);
    assert_eq!(normalized[1].argb, normalized[2].argb);

    let mut signed_zero = source(&[-0.0, 0.0, 1.0]);
    signed_zero.stops[1].argb = signed_zero.stops[0].argb;
    let normalized = normalize_outline_stops(&signed_zero).unwrap();
    assert_eq!(normalized.len(), 3);
    assert_eq!(
        normalized[1].position.value().to_bits(),
        (-0.0_f32).to_bits()
    );
    assert_eq!(normalized[2].position.value().to_bits(), 0.0_f32.to_bits());
    signed_zero.stops[1].argb ^= 1;
    assert!(matches!(
        normalize_outline_stops(&signed_zero),
        Err(PaintDiagnosticKind::InvalidGradient)
    ));
}

#[test]
fn outline_copy_validates_every_wire_stop_and_preserves_singleton_behavior() {
    let mut invalid_tail = source(&[1.0, 0.9, 0.8, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1, f32::NAN]);
    assert!(matches!(
        normalize_outline_stops(&invalid_tail),
        Err(PaintDiagnosticKind::InvalidGradient)
    ));
    invalid_tail.stops.truncate(1);
    invalid_tail.stops[0].position = PaintFloat32::from_bits(f32::NAN.to_bits());
    assert_eq!(
        normalize_outline_stops(&invalid_tail).unwrap(),
        invalid_tail.stops
    );

    let too_many = source(&vec![0.5; 256]);
    assert!(matches!(
        normalize_outline_stops(&too_many),
        Err(PaintDiagnosticKind::InvalidGradient)
    ));
    assert!(matches!(
        normalize_outline_stops(&source(&[])),
        Err(PaintDiagnosticKind::InvalidGradient)
    ));
}

#[test]
fn ascending_line_source_projects_the_native_descending_copy_as_red() {
    let mut ascending = line_source();
    ascending.stops[0].argb = 0xffff_0000;
    ascending.stops[0].position = PaintFloat32::from_bits(0.0_f32.to_bits());
    ascending.stops[1].argb = 0xff00_00ff;
    ascending.stops[1].position = PaintFloat32::from_bits(1.0_f32.to_bits());
    let original = ascending.clone();
    let projected = project_outline_stops(&ascending).unwrap();
    assert_eq!(
        projected,
        [
            ProjectedStop {
                offset: 0.0,
                argb: 0xffff_0000
            },
            ProjectedStop {
                offset: 1.0,
                argb: 0xffff_0000
            },
        ]
    );
    assert_eq!(ascending, original);

    let mut line = line(BoundingBox {
        x_min: 97.64480590820312,
        y_min: 434.66650390625,
        x_max: 222.5447540283203,
        y_max: 572.0924682617188,
    });
    line.style.paint = ShapePaint::Gradient;
    line.style.paint_source = Some(ShapePaintSource::Color(ascending.clone()));
    assert_eq!(Plan::for_line(&line, &ascending).unwrap().stops, projected);
}

#[test]
fn radial_line_projects_the_opaque_descending_cache_step_without_mutating_source() {
    let mut radial = radial_line_source();
    radial.stops[0] = GradientStopSource {
        argb: 0xffff_0000,
        position: PaintFloat32::from_bits(0.5_f32.to_bits()),
    };
    radial.stops[1] = GradientStopSource {
        argb: 0xff00_00ff,
        position: PaintFloat32::from_bits(0.75_f32.to_bits()),
    };
    let original = radial.clone();

    let plan = Plan::for_line(&test_line(), &radial).unwrap();
    assert_eq!(radial, original);
    assert_eq!(
        plan.stops,
        [
            ProjectedStop {
                offset: 0.0,
                argb: 0xff00_00ff,
            },
            ProjectedStop {
                offset: 0.5,
                argb: 0xff00_00ff,
            },
            ProjectedStop {
                offset: 0.5,
                argb: 0xffff_0000,
            },
            ProjectedStop {
                offset: 1.0,
                argb: 0xffff_0000,
            },
        ]
    );
}

#[test]
fn radial_line_cache_step_uses_native_q16_cache_quantization() {
    let below_first_boundary = f32::from_bits((1.0_f32 / 256.0).to_bits() - 1);
    let just_below = f32::from_bits((129.0_f32 / 256.0).to_bits() - 1);
    let at_boundary = 129.0_f32 / 256.0;
    let cases = [
        (1.0_f32 / 256.0, 1),
        (below_first_boundary, 0),
        (0.5, 128),
        (just_below, 128),
        (at_boundary, 129),
        (1.0, 255),
    ];
    for (position, expected_index) in cases {
        assert_eq!(radial_cache_index(position), expected_index);
    }

    let mut radial = radial_line_source();
    radial.stops[0] = GradientStopSource {
        argb: 0xffff_0000,
        position: PaintFloat32::from_bits(just_below.to_bits()),
    };
    radial.stops[1] = GradientStopSource {
        argb: 0xff00_00ff,
        position: PaintFloat32::from_bits(0.75_f32.to_bits()),
    };
    let projected = Plan::for_line(&test_line(), &radial).unwrap().stops;
    assert_eq!(projected[1].offset, 0.5);
    assert_eq!(projected[2].offset, 0.5);
    assert_eq!(projected[1].argb, 0xff00_00ff);
    assert_eq!(projected[2].argb, 0xffff_0000);

    radial.stops[0].position = PaintFloat32::from_bits(at_boundary.to_bits());
    let projected = Plan::for_line(&test_line(), &radial).unwrap().stops;
    assert_eq!(projected[1].offset, f64::from(at_boundary));
    assert_eq!(projected[2].offset, f64::from(at_boundary));

    radial.stops[0].position = PaintFloat32::from_bits(below_first_boundary.to_bits());
    let original = radial.clone();
    let projected = Plan::for_line(&test_line(), &radial).unwrap().stops;
    assert_eq!(
        radial, original,
        "bucket-zero projection must not mutate source"
    );
    assert_eq!(
        projected,
        [
            ProjectedStop {
                offset: 0.0,
                argb: 0xffff_0000
            },
            ProjectedStop {
                offset: 1.0,
                argb: 0xffff_0000
            },
        ]
    );

    // The first nonzero bucket remains a step at exactly 1/256, distinct from
    // the V41-observed constant bucket-zero cache.
    radial.stops[0].position = PaintFloat32::from_bits((1.0_f32 / 256.0).to_bits());
    let projected = Plan::for_line(&test_line(), &radial).unwrap().stops;
    assert_eq!(
        projected,
        [
            ProjectedStop {
                offset: 0.0,
                argb: 0xff00_00ff
            },
            ProjectedStop {
                offset: 1.0 / 256.0,
                argb: 0xff00_00ff
            },
            ProjectedStop {
                offset: 1.0 / 256.0,
                argb: 0xffff_0000
            },
            ProjectedStop {
                offset: 1.0,
                argb: 0xffff_0000
            },
        ]
    );
}

#[test]
fn radial_line_step_preserves_partial_zero_alpha_and_nonprimary_argb() {
    // These four synthetic native controls use the same two descending cache
    // stops. V35/V36 cover partial alpha, V37 retains hidden RGB at alpha zero,
    // and V38 changes the partial-alpha stop to nonprimary RGB.
    let cases = [
        (0x80ff_0000, 0xff00_00ff),
        (0x80ff_0000, 0x4000_00ff),
        (0x00ff_0000, 0x4000_00ff),
        (0x8059_21cd, 0x4000_00ff),
    ];
    for (low_argb, high_argb) in cases {
        let mut radial = radial_line_source();
        radial.stops[0] = GradientStopSource {
            argb: low_argb,
            position: PaintFloat32::from_bits(0x3f00_c49c),
        };
        radial.stops[1] = GradientStopSource {
            argb: high_argb,
            position: PaintFloat32::from_bits(0x3f40_4189),
        };
        let original = radial.clone();
        let projected = Plan::for_line(&test_line(), &radial).unwrap().stops;
        assert_eq!(radial, original, "projection must not mutate source stops");
        assert_eq!(
            projected,
            [
                ProjectedStop {
                    offset: 0.0,
                    argb: high_argb,
                },
                ProjectedStop {
                    offset: 0.5,
                    argb: high_argb,
                },
                ProjectedStop {
                    offset: 0.5,
                    argb: low_argb,
                },
                ProjectedStop {
                    offset: 1.0,
                    argb: low_argb,
                },
            ]
        );
    }

    let mut radial = radial_line_source();
    radial.stops[0].position = PaintFloat32::from_bits(0.5_f32.to_bits());
    radial.stops[1].position = PaintFloat32::from_bits(0.75_f32.to_bits());
    radial.stops[0].argb = 0xffff_0000;
    radial.stops[1].argb = radial.stops[0].argb;
    assert!(matches!(
        Plan::for_line(&test_line(), &radial),
        Err(PaintDiagnosticKind::InvalidGradient)
    ));
    radial.stops[1].argb = 0xff00_00ff;
    radial.stops.push(GradientStopSource {
        argb: 0xff12_3456,
        position: PaintFloat32::from_bits(0.9_f32.to_bits()),
    });
    assert!(matches!(
        Plan::for_line(&test_line(), &radial),
        Err(PaintDiagnosticKind::InvalidGradient)
    ));

    // Preserve the existing full-span descending overwrite behavior.
    radial.stops.truncate(2);
    radial.stops[0].position = PaintFloat32::from_bits(0.0_f32.to_bits());
    radial.stops[1].position = PaintFloat32::from_bits(1.0_f32.to_bits());
    assert_eq!(
        Plan::for_line(&test_line(), &radial).unwrap().stops,
        [
            ProjectedStop {
                offset: 0.0,
                argb: 0xffff_0000,
            },
            ProjectedStop {
                offset: 1.0,
                argb: 0xffff_0000,
            },
        ]
    );
}

#[test]
fn radial_cache_step_does_not_extend_to_shape_outlines_or_linear_lines() {
    let mut source = radial_line_source();
    source.stops[0] = GradientStopSource {
        argb: 0xffff_0000,
        position: PaintFloat32::from_bits(0.5_f32.to_bits()),
    };
    source.stops[1] = GradientStopSource {
        argb: 0xff00_00ff,
        position: PaintFloat32::from_bits(0.75_f32.to_bits()),
    };
    assert!(matches!(
        Plan::for_shape(&shape(), &source, PaintRole::Outline, None),
        Err(PaintDiagnosticKind::InvalidGradient)
    ));

    source.gradient_type = 0;
    assert!(matches!(
        Plan::for_line(&test_line(), &source),
        Err(PaintDiagnosticKind::InvalidGradient)
    ));
}

#[test]
fn shape_outline_uses_the_shared_descending_copy_but_fill_keeps_wire_order() {
    let shape = shape();
    let mut fill = source(&[0.0, 1.0]);
    fill.stops[0].argb = 0xffff_0000;
    fill.stops[1].argb = 0xff00_00ff;
    let fill_plan = Plan::for_shape(&shape, &fill, PaintRole::Fill, None).unwrap();
    assert_eq!(
        fill_plan.stops,
        [
            ProjectedStop {
                offset: 0.0,
                argb: 0xffff_0000,
            },
            ProjectedStop {
                offset: 1.0,
                argb: 0xff00_00ff,
            },
        ]
    );

    let mut outline = fill.clone();
    outline.outline_color_type = Some(1);
    outline.property_flags = 0;
    let original = outline.clone();
    let outline_plan = Plan::for_shape(&shape, &outline, PaintRole::Outline, None).unwrap();
    assert_eq!(
        outline_plan.stops,
        [
            ProjectedStop {
                offset: 0.0,
                argb: 0xffff_0000,
            },
            ProjectedStop {
                offset: 1.0,
                argb: 0xffff_0000,
            },
        ]
    );
    assert_eq!(outline, original);
}

#[test]
fn descending_stops_project_only_when_last_positive_span_covers_full_range() {
    // V13 native synthetic-effect capture used blue@1 followed by red@0.
    // Endpoint extension yields blue@0, blue@1, red@0, red@1. The V13
    // synthetic native readback was solid red; broader cache behavior is not
    // asserted here. Colors are source ARGB.
    let mut reversed = source(&[1.0, 0.0]);
    reversed.stops[0].argb = 0xff00_00ff;
    reversed.stops[1].argb = 0xffff_0000;
    assert_eq!(
        project_stops(&reversed).unwrap(),
        [
            ProjectedStop {
                offset: 0.0,
                argb: 0xffff_0000,
            },
            ProjectedStop {
                offset: 1.0,
                argb: 0xffff_0000,
            },
        ]
    );

    // The final equal interval is skipped after a full-range overwrite.
    // This pair has equal colors intentionally.
    let mut equal = source(&[1.0, 0.5, 0.0, 1.0, 1.0]);
    equal.stops[2].argb = 0xff12_3456;
    equal.stops[3].argb = 0xff12_3456;
    assert_eq!(
        project_stops(&equal).unwrap(),
        [
            ProjectedStop {
                offset: 0.0,
                argb: 0xff12_3456,
            },
            ProjectedStop {
                offset: 1.0,
                argb: 0xff12_3456,
            },
        ]
    );

    // Full-range overwrite can occur after arbitrary earlier spans; preserve
    // the colors on that final positive interval rather than fixture constants.
    let mut late = source(&[0.2, 0.8, 0.0, 1.0, 1.0]);
    late.stops[2].argb = 0xff11_2233;
    late.stops[3].argb = 0xff44_5566;
    assert_eq!(
        project_stops(&late).unwrap(),
        [
            ProjectedStop {
                offset: 0.0,
                argb: 0xff11_2233,
            },
            ProjectedStop {
                offset: 1.0,
                argb: 0xff44_5566,
            },
        ]
    );
}

#[test]
fn descending_partial_overwrite_stays_rejected_and_consumption_stays_capped() {
    // Endpoint extension appends a positive 0.5-to-1 span, so an earlier
    // full-range span is insufficient to describe the final cache.
    assert!(matches!(
        project_stops(&source(&[0.0, 1.0, 0.5, 0.5])),
        Err(PaintDiagnosticKind::InvalidGradient)
    ));

    assert!(matches!(
        project_stops(&source(&[0.1, 0.8, 0.2, 0.9])),
        Err(PaintDiagnosticKind::InvalidGradient)
    ));

    let mut many = source(&[0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, f32::NAN]);
    many.stops[8].argb = 0xff01_0203;
    many.stops[9].argb = 0xff04_0506;
    assert_eq!(
        project_stops(&many).unwrap(),
        [
            ProjectedStop {
                offset: 0.0,
                argb: 0xff01_0203,
            },
            ProjectedStop {
                offset: 1.0,
                argb: 0xff04_0506,
            },
        ]
    );
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

#[test]
fn gradient_stops_share_shape_foreground_policy_and_keep_saved_alpha() {
    let mut source = source(&[0.0, 1.0]);
    source.stops[0].argb = 0x80000000;
    source.stops[1].argb = 0x40000000;
    let before = source.clone();
    for (dark, color) in [(false, "#000000"), (true, "#ffffff")] {
        let mut scene = Scene::new(Svg::new());
        Plan::for_shape(&shape(), &source, PaintRole::Fill, None)
            .unwrap()
            .paint(&mut scene, RenderTheme::for_canvas(dark));
        let svg = scene.finish();
        let xml = roxmltree::Document::parse(&svg).unwrap();
        let stops = xml
            .descendants()
            .filter(|node| node.has_tag_name("stop"))
            .collect::<Vec<_>>();
        assert_eq!(stops.len(), 2);
        for (stop, alpha) in stops.into_iter().zip([128.0 / 255.0, 64.0 / 255.0]) {
            assert_eq!(stop.attribute("stop-color"), Some(color));
            let opacity: f64 = stop.attribute("stop-opacity").unwrap().parse().unwrap();
            assert_eq!(opacity, alpha);
        }
        assert_eq!(source, before);
    }
}
