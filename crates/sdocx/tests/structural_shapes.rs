mod support;

use sdocx::{
    DiagnosticCode, Error, NativeLine, NativeShape, PageElement, ParseLimits, ParseOptions,
    ShapePaint,
};
use support::{archive, object, page};

// Different mask widths exercise the generic reader, independent of native
// fixed header offsets. Short identities deliberately defeat UUID scanning.
fn frame(kind: i16, mask: u32, fixed: &[u8], flexible: &[u8]) -> Vec<u8> {
    let offset = 18 + fixed.len();
    let mut bytes = ((offset + flexible.len()) as u32).to_le_bytes().to_vec();
    bytes.extend(kind.to_le_bytes());
    bytes.extend((offset as u32).to_le_bytes());
    bytes.extend([1, 0, 5]);
    bytes.extend(mask.to_le_bytes());
    bytes.push(0);
    bytes.extend(fixed);
    bytes.extend(flexible);
    bytes
}

fn numbers(values: &[f64]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}
fn sized(bytes: &[u8]) -> Vec<u8> {
    let mut result = (bytes.len() as u32).to_le_bytes().to_vec();
    result.extend(bytes);
    result
}
fn base(rotation: f32) -> Vec<u8> {
    let mut fixed = 5500_u32.to_le_bytes().to_vec();
    fixed.extend(2_u16.to_le_bytes());
    fixed.extend(b"sh");
    fixed.extend(1234_i64.to_le_bytes());
    fixed.extend(numbers(&[10.0, 20.0, 210.0, 220.0]));
    fixed.extend([0; 5]);
    frame(0, 1, &fixed, &rotation.to_le_bytes())
}
fn base_fixed() -> Vec<u8> {
    [0_u32, 4, 0]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .chain([0])
        .collect()
}
fn color(outline: bool, kind: u8, argb: u32) -> Vec<u8> {
    let mut bytes = vec![1, if outline { 0 } else { kind }];
    if outline {
        bytes.push(kind);
    }
    bytes.extend(argb.to_le_bytes());
    bytes.extend([0; 12]); // gradient type, angle, position and stop count
    bytes
}
fn style(width: f32) -> Vec<u8> {
    let mut bytes = width.to_le_bytes().to_vec();
    bytes.extend([0, 0, 1, 2, 0, 0, 0, 0]);
    bytes
}
fn outline() -> Vec<u8> {
    let mut fields = sized(&color(true, 0, 0x800000ff));
    fields.extend(sized(&style(3.5)));
    frame(6, 12, &base_fixed(), &fields)
}
fn shape_fixed(kind: u32, rotation: f32) -> Vec<u8> {
    let mut bytes = kind.to_le_bytes().to_vec();
    bytes.extend(numbers(&[-10.0, 0.0, 90.0, 60.0]));
    bytes.extend(rotation.to_le_bytes());
    bytes.extend([0; 5]); // empty path and control points
    bytes.extend(numbers(&[10.0, 20.0, 210.0, 220.0]));
    bytes
}
fn shape_fields() -> Vec<u8> {
    let effect = color(false, 0, 0x40ff0000);
    let mut fields = (effect.len() as u32).to_le_bytes().to_vec();
    fields.push(1);
    fields.extend(effect);
    fields
}
fn shape(kind: u32) -> Vec<u8> {
    let mut bytes = base(0.0);
    bytes.extend(outline());
    bytes.extend(frame(7, 32, &shape_fixed(kind, 30.0), &shape_fields()));
    bytes
}
fn line_fixed(kind: u8) -> Vec<u8> {
    let mut fixed = vec![kind, 0, 0];
    fixed.extend(numbers(&[90.0, 45.0, -10.0, 45.0]));
    fixed.extend(numbers(&[-10.0, 45.0, 90.0, 45.0]));
    fixed.extend(numbers(&[-10.0, 45.0, 90.0, 45.0]));
    fixed.extend([0; 4]);
    assert_eq!(fixed.len(), 103);
    fixed
}
fn line(kind: u8, fields: u32, flexible: &[u8]) -> Vec<u8> {
    let mut bytes = base(90.0);
    bytes.extend(outline());
    bytes.extend(frame(8, fields, &line_fixed(kind), flexible));
    bytes
}
fn single(kind: u8, payload: &[u8]) -> Vec<u8> {
    archive(&page(&[vec![object(kind, payload, &[])]], 0, &[]))
}
fn as_shape(element: &PageElement) -> &NativeShape {
    let PageElement::Shape(value) = element else {
        panic!("expected shape")
    };
    value
}
fn as_line(element: &PageElement) -> &NativeLine {
    let PageElement::Line(value) = element else {
        panic!("expected line")
    };
    value
}
fn has_shape_warning(parsed: &sdocx::ParsedDocument) -> bool {
    parsed
        .report
        .diagnostics
        .iter()
        .any(|d| d.code == DiagnosticCode::UnsupportedShapeFeature)
}
fn assert_format(kind: u8, payload: &[u8]) {
    let error = sdocx::parse_bytes(&single(kind, payload)).unwrap_err();
    assert!(
        matches!(&error, Error::Format(message) if message.contains("page page:") && message.contains("at 0x")),
        "{error}"
    );
}

#[test]
fn decodes_shape_geometry_rotation_and_independent_outline_and_fill() {
    let parsed = sdocx::parse_bytes_detailed(&single(7, &shape(4))).unwrap();
    assert!(!has_shape_warning(&parsed));
    let shape = as_shape(&parsed.document.pages[0].elements[0]);
    assert_eq!(shape.metadata.uuid, "sh");
    assert_eq!(shape.shape_type, 4);
    assert_eq!(shape.geometry_bbox.x_min, -10.0);
    assert_eq!(shape.metadata.bbox.x_min, 10.0);
    assert_eq!(shape.drawn_bbox.x_max, 210.0);
    assert_eq!(shape.rotation_degrees, 30.0);
    assert_eq!(shape.metadata.rotation_degrees, Some(0.0));
    assert_eq!(shape.style.width, 3.5);
    assert_eq!(shape.style.cap, 1);
    assert_eq!(shape.style.join, 2);
    assert!(matches!(shape.style.paint, ShapePaint::Solid(0x800000ff)));
    assert!(matches!(shape.fill, ShapePaint::Solid(0x40ff0000)));
    #[cfg(feature = "render")]
    {
        let svg = &sdocx::render_document_svg(&parsed.document, &Default::default())[0].svg;
        assert!(svg.contains("<rect x=\"-10.00\" y=\"0.00\" width=\"100.00\" height=\"60.00\""));
        assert!(svg.contains(
            "fill=\"#ff0000\" fill-opacity=\"0.2510\" stroke=\"#0000ff\" stroke-opacity=\"0.5020\""
        ));
        assert!(
            svg.contains(
                "stroke-width=\"3.50\" stroke-linecap=\"round\" stroke-linejoin=\"bevel\""
            )
        );
        assert!(svg.contains("rotate(30.00 40.00 30.00)"));
    }
}

#[test]
fn preserves_reversed_horizontal_line_without_rotating_it_twice() {
    let parsed = sdocx::parse_bytes_detailed(&single(8, &line(0, 0, &[]))).unwrap();
    assert!(!has_shape_warning(&parsed));
    let line = as_line(&parsed.document.pages[0].elements[0]);
    assert_eq!(line.begin, [90.0, 45.0]);
    assert_eq!(line.end, [-10.0, 45.0]);
    assert_eq!(line.metadata.rotation_degrees, Some(90.0));
    #[cfg(feature = "render")]
    {
        let svg = &sdocx::render_document_svg(&parsed.document, &Default::default())[0].svg;
        assert!(svg.contains("<line x1=\"90.00\" y1=\"45.00\" x2=\"-10.00\" y2=\"45.00\""));
        assert!(!svg.contains("rotate("));
        assert!(!svg.contains("/ >"));
    }
}

#[test]
fn missing_effects_use_native_defaults_and_unknown_templates_remain_shapes() {
    for kind in [1, 2, 3, 4, 8, 900] {
        let mut payload = base(0.0);
        payload.extend(frame(6, 0, &base_fixed(), &[]));
        payload.extend(frame(7, 0, &shape_fixed(kind, 0.0), &[]));
        let parsed = sdocx::parse_bytes_detailed(&single(7, &payload)).unwrap();
        let shape = as_shape(&parsed.document.pages[0].elements[0]);
        assert_eq!(shape.shape_type, kind);
        assert_eq!(shape.style.width, 2.0);
        assert!(matches!(shape.style.paint, ShapePaint::Solid(0xff000000)));
        assert!(matches!(shape.fill, ShapePaint::None));
        assert_eq!(has_shape_warning(&parsed), kind == 900);
        #[cfg(feature = "render")]
        if kind == 900 {
            let svg = &sdocx::render_document_svg(&parsed.document, &Default::default())[0].svg;
            assert!(!svg.contains("stroke-width=\"2.00\""));
        }
    }
}

#[test]
fn shape_and_line_payloads_cannot_borrow_from_the_next_object() {
    for (kind, payload) in [(7, shape(4)), (8, line(0, 0, &[]))] {
        for end in 0..payload.len() {
            assert_format(kind, &payload[..end]);
        }
        let mut wrong_kind = payload.clone();
        wrong_kind[4..6].copy_from_slice(&9_i16.to_le_bytes());
        assert_format(kind, &wrong_kind);
        let mut bad_size = payload;
        bad_size[..4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_format(kind, &bad_size);
    }
}

#[test]
fn rejects_nonfinite_geometry_and_negative_or_nonfinite_widths() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut fixed = line_fixed(0);
        fixed[3..11].copy_from_slice(&value.to_le_bytes());
        let mut payload = base(0.0);
        payload.extend(outline());
        payload.extend(frame(8, 0, &fixed, &[]));
        assert_format(8, &payload);
    }
    for value in [-1.0_f32, f32::NAN, f32::INFINITY] {
        let mut payload = base(0.0);
        payload.extend(frame(6, 8, &base_fixed(), &sized(&style(value))));
        payload.extend(frame(7, 0, &shape_fixed(4, 0.0), &[]));
        assert_format(7, &payload);
    }
}

#[test]
fn recursive_objects_and_layers_use_structural_order_without_phantom_text() {
    let decoy = shape(4);
    let bytes = archive(&page(
        &[
            vec![object(
                4,
                &[],
                &[object(7, &shape(1), &[]), object(8, &line(0, 0, &[]), &[])],
            )],
            vec![object(7, &shape(8), &[]), object(200, &decoy, &[])],
        ],
        0,
        &[],
    ));
    let parsed = sdocx::parse_bytes(&bytes).unwrap();
    let elements = &parsed.pages[0].elements;
    assert_eq!(elements.len(), 3);
    assert_eq!(as_shape(&elements[0]).shape_type, 1);
    assert_eq!(as_line(&elements[1]).line_type, 0);
    assert_eq!(as_shape(&elements[2]).shape_type, 8);
    let limits = ParseLimits {
        max_objects_per_page: 2,
        ..Default::default()
    };
    assert!(matches!(
        sdocx::parse_bytes_with_options(&bytes, &ParseOptions { limits }),
        Err(Error::LimitExceeded { .. })
    ));
}

fn native_path(commands: &[(u8, &[f64])]) -> Vec<u8> {
    let mut path = (commands.len() as u32).to_le_bytes().to_vec();
    for (verb, values) in commands {
        path.push(*verb);
        path.extend(numbers(values));
    }
    path
}

#[test]
fn line_paths_preserve_curves_and_stop_before_future_fields() {
    let path = native_path(&[
        (1, &[10.0, 20.0]),
        (2, &[30.0, 40.0]),
        (3, &[50.0, 60.0, 70.0, 80.0]),
        (4, &[90.0, 10.0, 110.0, 30.0, 120.0, 40.0]),
        (6, &[]),
    ]);
    let mut fields = path.clone();
    fields.extend(b"future");
    let parsed = sdocx::parse_bytes_detailed(&single(8, &line(2, 24, &fields))).unwrap();
    assert_eq!(
        as_line(&parsed.document.pages[0].elements[0]).path_data,
        path
    );
    assert!(has_shape_warning(&parsed));
    #[cfg(feature = "render")]
    {
        let svg = &sdocx::render_document_svg(&parsed.document, &Default::default())[0].svg;
        assert!(svg.contains("M 10.00 20.00 L 30.00 40.00 Q 50.00 60.00 70.00 80.00 C 90.00 10.00 110.00 30.00 120.00 40.00 Z"));
        assert!(!svg.contains("<line "));
        assert!(!svg.contains("rotate("));
    }
}

#[test]
fn path_counts_coordinates_and_truncation_are_bounded() {
    let path = native_path(&[
        (1, &[10.0, 20.0]),
        (4, &[30.0, 40.0, 50.0, 60.0, 70.0, 80.0]),
    ]);
    for end in 0..path.len() {
        assert_format(8, &line(2, 8, &path[..end]));
    }
    assert_format(8, &line(2, 8, &u32::MAX.to_le_bytes()));
    assert_format(8, &line(2, 8, &native_path(&[(1, &[f64::NAN, 0.0])])));
}

#[test]
fn unknown_line_types_or_path_verbs_do_not_become_straight_lines() {
    for payload in [
        line(88, 0, &[]),
        line(1, 0, &[]),
        line(2, 8, &native_path(&[(99, &[])])),
    ] {
        let parsed = sdocx::parse_bytes_detailed(&single(8, &payload)).unwrap();
        assert!(has_shape_warning(&parsed));
        assert_eq!(parsed.document.pages[0].elements.len(), 1);
        #[cfg(feature = "render")]
        {
            let svg = &sdocx::render_document_svg(&parsed.document, &Default::default())[0].svg;
            assert!(!svg.contains("<line "));
            assert!(!svg.contains("<path "));
        }
    }
}
