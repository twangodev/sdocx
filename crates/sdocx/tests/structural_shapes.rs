#[cfg(feature = "render")]
#[path = "support/svg.rs"]
mod svg_support;

mod support;

use sdocx::{
    DiagnosticCode, Error, NativeLine, NativeShape, PageElement, ParseLimits, ParseOptions,
    ShapePaint, TextAreaType,
};
use support::{archive, object, page, page_with_current_layer};

// Different mask widths exercise the generic reader, independent of native
// fixed header offsets. Short identities deliberately defeat UUID scanning.
fn frame(kind: i16, mask: u32, fixed: &[u8], flexible: &[u8]) -> Vec<u8> {
    let offset = 18 + fixed.len();
    let mut bytes = ((offset + flexible.len()) as u32).to_le_bytes().to_vec();
    bytes.extend(kind.to_le_bytes());
    bytes.extend((offset as u32).to_le_bytes());
    bytes.extend([1, u8::from(kind == 0) << 3, 5]);
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

#[test]
fn hidden_shapes_and_lines_are_retained_outside_the_visible_page() {
    for (kind, mut payload) in [(7, shape(1)), (8, line(0, 0, &[]))] {
        payload[11] &= !(1 << 3);
        let raw = page(&[vec![object(kind, &payload, &[])]], 0, &[]);
        let parsed = sdocx::parse_bytes_detailed(&archive(&raw)).unwrap();
        assert!(parsed.document.pages[0].elements().next().is_none());
        let stored = &parsed.stored_pages[0].page.layers.layers[0].objects[0];
        assert_eq!(stored.payload(&raw).unwrap(), payload);
        assert!(!stored.base_metadata(&raw).unwrap().visible);
        #[cfg(feature = "render")]
        assert!(
            sdocx::render_document_svg(&parsed.document, &Default::default())[0]
                .geometry_diagnostics
                .is_empty()
        );
    }
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
#[cfg(feature = "render")]
fn assert_geometry_issue(
    parsed: &sdocx::ParsedDocument,
    rendered: &sdocx::RenderedPage,
    kind: sdocx::GeometryDiagnosticKind,
) {
    assert_eq!(
        rendered.geometry_diagnostics,
        [sdocx::GeometryDiagnostic {
            source_offset: parsed.document.pages[0].objects[0].source_offset,
            object_uuid: "sh".into(),
            kind,
        }]
    );
}
fn assert_format(kind: u8, payload: &[u8]) {
    let error = sdocx::parse_bytes(&single(kind, payload)).unwrap_err();
    assert!(
        matches!(&error, Error::Format(message) if message.contains("page page:") && message.contains("at 0x")),
        "{error}"
    );
}

#[test]
fn shape_text_editability_is_metadata_and_unknown_properties_still_warn() {
    #[cfg(feature = "render")]
    let mut baseline = None;
    for mask in [0, 0x04, 0x40, 0x44] {
        let mut payload = base(0.0);
        payload.extend(outline());
        let mut geometry = frame(7, 32, &shape_fixed(4, 30.0), &shape_fields());
        geometry[11] = mask;
        payload.extend(geometry);
        let parsed = sdocx::parse_bytes_detailed(&single(7, &payload)).unwrap();
        assert_eq!(
            as_shape(parsed.document.pages[0].elements().next().unwrap()).text_editable,
            mask & 4 != 0
        );
        assert_eq!(has_shape_warning(&parsed), mask & 0x40 != 0);
        #[cfg(feature = "render")]
        {
            let svg = sdocx::render_document_svg(&parsed.document, &Default::default())[0]
                .svg
                .clone();
            if let Some(expected) = &baseline {
                assert_eq!(&svg, expected);
            } else {
                baseline = Some(svg);
            }
        }
    }
}

#[test]
fn decodes_shape_geometry_rotation_and_independent_outline_and_fill() {
    let parsed = sdocx::parse_bytes_detailed(&single(7, &shape(4))).unwrap();
    assert!(!has_shape_warning(&parsed));
    let shape = as_shape(parsed.document.pages[0].elements().next().unwrap());
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
        svg_support::assert_svg_element(
            svg,
            "rect",
            &[
                ("x", "-10.00"),
                ("y", "0.00"),
                ("width", "100.00"),
                ("height", "60.00"),
            ],
        );
        svg_support::assert_svg_element(
            svg,
            "g",
            &[
                ("fill", "#ff0000"),
                ("fill-opacity", "0.2510"),
                ("stroke", "#0000ff"),
                ("stroke-opacity", "0.5020"),
                ("stroke-width", "3.50"),
                ("stroke-linecap", "round"),
                ("stroke-linejoin", "bevel"),
            ],
        );
        assert!(svg.contains("rotate(30.00 40.00 30.00)"));
    }
}

#[test]
fn preserves_reversed_horizontal_line_without_rotating_it_twice() {
    let parsed = sdocx::parse_bytes_detailed(&single(8, &line(0, 0, &[]))).unwrap();
    assert!(!has_shape_warning(&parsed));
    let line = as_line(parsed.document.pages[0].elements().next().unwrap());
    assert_eq!(line.begin, [90.0, 45.0]);
    assert_eq!(line.end, [-10.0, 45.0]);
    assert_eq!(line.metadata.rotation_degrees, Some(90.0));
    #[cfg(feature = "render")]
    {
        let svg = &sdocx::render_document_svg(&parsed.document, &Default::default())[0].svg;
        let xml = roxmltree::Document::parse(svg).unwrap();
        let node = xml
            .descendants()
            .find(|node| node.has_tag_name("line"))
            .unwrap();
        let endpoints = ["x1", "y1", "x2", "y2"]
            .map(|attribute| node.attribute(attribute).unwrap().parse::<f64>().unwrap());
        assert_eq!(endpoints, [90.0, 45.0, -10.0, 45.0]);
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
        let shape = as_shape(parsed.document.pages[0].elements().next().unwrap());
        assert_eq!(shape.shape_type, kind);
        assert_eq!(shape.style.width, 2.0);
        assert!(matches!(shape.style.paint, ShapePaint::Solid(0xff000000)));
        assert!(matches!(shape.fill, ShapePaint::None));
        assert_eq!(has_shape_warning(&parsed), kind == 900);
        #[cfg(feature = "render")]
        if kind == 900 {
            let rendered = sdocx::render_document_svg(&parsed.document, &Default::default());
            assert!(!rendered[0].svg.contains("stroke-width=\"2.00\""));
            assert_geometry_issue(
                &parsed,
                &rendered[0],
                sdocx::GeometryDiagnosticKind::UnsupportedShapeTemplate,
            );
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
fn current_layer_shapes_keep_child_order_without_phantom_text() {
    let decoy = shape(4);
    let layers = [
        vec![object(
            4,
            &[],
            &[object(7, &shape(1), &[]), object(8, &line(0, 0, &[]), &[])],
        )],
        vec![object(7, &shape(8), &[]), object(200, &decoy, &[])],
    ];
    for current_layer_index in [0, 1] {
        let bytes = archive(&page_with_current_layer(
            &layers,
            current_layer_index,
            0,
            &[],
        ));
        let parsed = sdocx::parse_bytes_detailed(&bytes).unwrap();
        let elements: Vec<_> = parsed.document.pages[0].elements().collect();
        if current_layer_index == 0 {
            assert_eq!(elements.len(), 2);
            assert_eq!(as_shape(elements[0]).shape_type, 1);
            assert_eq!(as_line(elements[1]).line_type, 0);
        } else {
            assert_eq!(elements.len(), 1);
            assert_eq!(as_shape(elements[0]).shape_type, 8);
        }
        assert_eq!(parsed.stored_pages[0].page.layers.layers.len(), 2);
        let limits = ParseLimits {
            max_objects_per_page: 4,
            ..Default::default()
        };
        assert!(matches!(
            sdocx::parse_bytes_with_options(
                &bytes,
                &ParseOptions {
                    limits,
                    ..Default::default()
                }
            ),
            Err(Error::LimitExceeded {
                resource: "objects per page",
                limit: 4,
                actual: 5,
            })
        ));
    }
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
        as_line(parsed.document.pages[0].elements().next().unwrap()).path_data,
        path
    );
    assert!(has_shape_warning(&parsed));
    #[cfg(feature = "render")]
    {
        let svg = &sdocx::render_document_svg(&parsed.document, &Default::default())[0].svg;
        assert_svg_path(
            svg,
            "M 10.00 20.00 L 30.00 40.00 Q 50.00 60.00 70.00 80.00 C 90.00 10.00 110.00 30.00 120.00 40.00 Z",
        );
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
    for (index, payload) in [
        line(88, 0, &[]),
        line(1, 0, &[]),
        line(2, 8, &native_path(&[(99, &[])])),
    ]
    .into_iter()
    .enumerate()
    {
        let parsed = sdocx::parse_bytes_detailed(&single(8, &payload)).unwrap();
        assert!(has_shape_warning(&parsed));
        assert_eq!(parsed.document.pages[0].elements().count(), 1);
        #[cfg(feature = "render")]
        {
            let rendered = sdocx::render_document_svg(&parsed.document, &Default::default());
            assert!(!rendered[0].svg.contains("<line "));
            assert!(!rendered[0].svg.contains("<path "));
            assert_geometry_issue(
                &parsed,
                &rendered[0],
                [
                    sdocx::GeometryDiagnosticKind::UnsupportedLineType,
                    sdocx::GeometryDiagnosticKind::MissingLinePath,
                    sdocx::GeometryDiagnosticKind::UnsupportedPath,
                ][index],
            );
        }
        #[cfg(not(feature = "render"))]
        let _ = index;
    }
}

#[test]
fn native_shapes_and_lines_survive_document_parsing() {
    let bytes = archive(&page(
        &[vec![
            object(7, &shape(4), &[]),
            object(8, &line(0, 0, &[]), &[]),
        ]],
        0,
        &[],
    ));
    let document = sdocx::parse_bytes(&bytes).unwrap();
    assert_eq!(document.pages[0].elements().count(), 2);
}

fn text_common(text: &str) -> Vec<u8> {
    let mut bytes = (text.encode_utf16().count() as u32).to_le_bytes().to_vec();
    for unit in text.encode_utf16() {
        bytes.extend(unit.to_le_bytes());
    }
    bytes.extend(1_u32.to_le_bytes()); // one font-size span
    bytes.extend(20_u16.to_le_bytes());
    for value in [3_u32, 0, text.encode_utf16().count() as u32, 1] {
        bytes.extend(value.to_le_bytes());
    }
    bytes.extend(18.0_f32.to_le_bytes());
    bytes.extend([0; 4 + 16 + 1 + 2 + 8]); // paragraphs, margins, gravity, sections, object flags
    sized(&bytes)
}

#[test]
fn embedded_shape_text_preserves_unicode_and_keeps_fill_aligned() {
    let common = text_common("A日本語😀");
    let mut fields = common.clone();
    fields.push(1);
    fields.extend((-9_i32).to_le_bytes()); // pen name ID is before fill
    fields.extend(123456_i32.to_le_bytes()); // advanced pen settings ID
    fields.extend(shape_fields());
    let mut payload = base(0.0);
    payload.extend(outline());
    payload.extend(frame(7, 0x37, &shape_fixed(4, 30.0), &fields));
    let parsed = sdocx::parse_bytes_detailed(&single(7, &payload)).unwrap();
    let shape = as_shape(parsed.document.pages[0].elements().next().unwrap());
    let text = shape.text.as_ref().unwrap();
    assert_eq!(shape.text_area_type, Some(TextAreaType::Free));
    assert_eq!(text.text_area_type, shape.text_area_type);
    assert_eq!(text.text, "A日本語😀");
    assert_eq!(text.spans[0].end_utf16, 6);
    assert_eq!(text.bbox.x_min, -10.0);
    assert_eq!(text.rotation_degrees, Some(30.0));
    assert_eq!(shape.pen_name_id, Some(-9));
    assert_eq!(shape.pen_settings_id, Some(123456));
    assert!(matches!(shape.style.paint, ShapePaint::Solid(0x800000ff)));
    assert!(matches!(shape.fill, ShapePaint::Solid(0x40ff0000)));
    assert!(has_shape_warning(&parsed));
    for limits in [
        ParseLimits {
            max_text_characters: 5,
            ..Default::default()
        },
        ParseLimits {
            max_text_spans: 0,
            ..Default::default()
        },
        ParseLimits {
            max_object_nesting_depth: 0,
            ..Default::default()
        },
    ] {
        assert!(matches!(
            sdocx::parse_bytes_with_options(
                &single(7, &payload),
                &ParseOptions {
                    limits,
                    ..Default::default()
                }
            ),
            Err(Error::LimitExceeded { .. })
        ));
    }
    for end in 0..common.len() {
        let mut broken = base(0.0);
        broken.extend(outline());
        broken.extend(frame(7, 1, &shape_fixed(4, 0.0), &common[..end]));
        assert_format(7, &broken);
    }
    #[cfg(feature = "render")]
    {
        let pages = sdocx::render_document_svg(&parsed.document, &Default::default());
        let svg = roxmltree::Document::parse(&pages[0].svg).unwrap();
        let source: String = svg
            .descendants()
            .filter(|node| node.has_tag_name("tspan"))
            .filter_map(|node| node.text())
            .collect();
        assert_eq!(source, "A日本語😀");

        let mut document = parsed.document.clone();
        let PageElement::Shape(shape) = document.pages[0].elements_mut().next().unwrap() else {
            panic!("expected shape")
        };
        shape.path_data = native_path(&[(1, &[1.0, 2.0]), (99, &[])]);
        let rendered = sdocx::render_document_svg(&document, &Default::default());
        assert_geometry_issue(
            &parsed,
            &rendered[0],
            sdocx::GeometryDiagnosticKind::UnsupportedPath,
        );
        let svg = roxmltree::Document::parse(&rendered[0].svg).unwrap();
        let source: String = svg
            .descendants()
            .filter(|node| node.has_tag_name("tspan"))
            .filter_map(|node| node.text())
            .collect();
        assert_eq!(source, "A日本語😀");
    }
}

#[test]
fn text_area_modes_without_text_keep_shape_fill_aligned() {
    for raw in 0..=u8::MAX {
        let mut fields = vec![raw];
        fields.extend(shape_fields());
        let mut payload = base(0.0);
        payload.extend(outline());
        payload.extend(frame(7, 0x22, &shape_fixed(4, 30.0), &fields));
        let parsed = sdocx::parse_bytes_detailed(&single(7, &payload)).unwrap();
        let shape = as_shape(parsed.document.pages[0].elements().next().unwrap());
        let expected = match raw {
            0 => TextAreaType::Margin,
            1 => TextAreaType::Free,
            2 => TextAreaType::Path,
            raw => TextAreaType::Other(raw),
        };
        assert_eq!(shape.text_area_type, Some(expected));
        assert_eq!(shape.text_area_type.unwrap().raw(), raw);
        assert!(shape.text.is_none());
        assert!(matches!(shape.fill, ShapePaint::Solid(0x40ff0000)));
    }
    let parsed = sdocx::parse_bytes(&single(7, &shape(1))).unwrap();
    assert_eq!(
        as_shape(parsed.pages[0].elements().next().unwrap()).text_area_type,
        None
    );
}

#[test]
fn line_pen_settings_and_name_ids_precede_the_path_in_native_order() {
    let path = native_path(&[(1, &[90.0, 45.0]), (2, &[-10.0, 45.0])]);
    let mut fields = (-9_i32).to_le_bytes().to_vec(); // advanced settings, bit 1
    fields.extend(123456_i32.to_le_bytes()); // pen name, bit 2
    fields.extend(&path);
    let parsed = sdocx::parse_bytes_detailed(&single(8, &line(2, 14, &fields))).unwrap();
    let decoded = as_line(parsed.document.pages[0].elements().next().unwrap());
    assert_eq!(decoded.pen_settings_id, Some(-9));
    assert_eq!(decoded.pen_name_id, Some(123456));
    assert_eq!(decoded.path_data, path);
    assert!(matches!(decoded.style.paint, ShapePaint::Solid(0x800000ff)));
    assert!(has_shape_warning(&parsed));
    for end in 0..8 {
        assert_format(8, &line(0, 6, &fields[..end]));
    }
    #[cfg(feature = "render")]
    {
        let svg = &sdocx::render_document_svg(&parsed.document, &Default::default())[0].svg;
        assert_svg_path(svg, "M 90 45 L -10 45");
        assert!(svg.contains("stroke=\"#0000ff\""));
    }
}

#[test]
fn legacy_line_pen_source_keeps_modern_ids_and_vector_path_aligned() {
    let path = native_path(&[(1, &[90.0, 45.0]), (2, &[-10.0, 45.0])]);
    for (pen_name_id, modern_id) in [(-123456_i32, 123456_i32), (-123456, -1), (-1, -2)] {
        let mut fields = pen_name_id.to_le_bytes().to_vec();
        fields.extend([0xde, 0xad, 0, 0xff]);
        fields.extend((-9_i32).to_le_bytes());
        fields.extend(modern_id.to_le_bytes());
        fields.extend(&path);
        let parsed = sdocx::parse_bytes_detailed(&single(8, &line(2, 15, &fields))).unwrap();
        let decoded = as_line(parsed.document.pages[0].elements().next().unwrap());
        let legacy = decoded.legacy_pen_source.as_ref().unwrap();
        assert_eq!(legacy.pen_name_id, pen_name_id);
        assert_eq!(legacy.remainder, [0xde, 0xad, 0, 0xff]);
        assert_eq!(decoded.pen_settings_id, Some(-9));
        assert_eq!(decoded.pen_name_id, Some(modern_id));
        assert_eq!(decoded.path_data, path);
        assert!(has_shape_warning(&parsed));
        #[cfg(feature = "serde")]
        {
            let restored: NativeLine =
                serde_json::from_value(serde_json::to_value(decoded).unwrap()).unwrap();
            let legacy = restored.legacy_pen_source.as_ref().unwrap();
            assert_eq!(legacy.pen_name_id, pen_name_id);
            assert_eq!(legacy.remainder, [0xde, 0xad, 0, 0xff]);
            assert_eq!(restored.pen_name_id, Some(modern_id));
            assert_eq!(restored.path_data, path);
        }
        #[cfg(feature = "render")]
        assert_svg_path(
            &sdocx::render_document_svg(&parsed.document, &Default::default())[0].svg,
            "M 90 45 L -10 45",
        );
    }
}

#[test]
fn legacy_line_pen_source_distinguishes_zero_from_absent_and_old_json() {
    for present in [false, true] {
        let fields = if present { vec![0; 8] } else { Vec::new() };
        let parsed =
            sdocx::parse_bytes_detailed(&single(8, &line(0, u32::from(present), &fields))).unwrap();
        let decoded = as_line(parsed.document.pages[0].elements().next().unwrap());
        assert_eq!(decoded.legacy_pen_source.is_some(), present);
        if let Some(legacy) = &decoded.legacy_pen_source {
            assert_eq!(legacy.pen_name_id, 0);
            assert_eq!(legacy.remainder, [0; 4]);
        }
        assert!(decoded.pen_name_id.is_none());
        assert_eq!(has_shape_warning(&parsed), present);
        #[cfg(feature = "serde")]
        {
            let mut json = serde_json::to_value(decoded).unwrap();
            let restored: NativeLine = serde_json::from_value(json.clone()).unwrap();
            assert_eq!(restored.legacy_pen_source.is_some(), present);
            if let Some(legacy) = &restored.legacy_pen_source {
                assert_eq!(legacy.pen_name_id, 0);
                assert_eq!(legacy.remainder, [0; 4]);
            }
            json.as_object_mut().unwrap().remove("legacy_pen_source");
            let restored: NativeLine = serde_json::from_value(json).unwrap();
            assert!(restored.legacy_pen_source.is_none());
        }
    }
}

#[test]
fn short_legacy_line_pen_sources_cannot_consume_the_following_frame() {
    for length in 0..8 {
        let mut payload = line(0, 1, &[0xff; 7][..length]);
        payload.extend(frame(66, 0, b"future", &[]));
        assert_format(8, &payload);
    }
}

#[test]
fn effect_sizes_and_fixed_geometry_cannot_consume_adjacent_fields() {
    for (mask, effect) in [(4, color(true, 0, 0xff123456)), (8, style(4.0))] {
        for end in 0..effect.len() {
            let mut payload = base(0.0);
            payload.extend(frame(6, mask, &base_fixed(), &sized(&effect[..end])));
            payload.extend(frame(7, 32, &shape_fixed(4, 0.0), &shape_fields()));
            assert_format(7, &payload);
        }
    }
    for end in 0..shape_fixed(4, 0.0).len() {
        let mut payload = base(0.0);
        payload.extend(outline());
        payload.extend(frame(7, 32, &shape_fixed(4, 0.0)[..end], &shape_fields()));
        assert_format(7, &payload);
    }
    let fill = color(false, 0, 0xffff0000);
    for end in 0..fill.len() {
        let mut fields = (end as u32).to_le_bytes().to_vec();
        fields.push(1);
        fields.extend(&fill[..end]);
        let mut payload = base(0.0);
        payload.extend(outline());
        payload.extend(frame(7, 32, &shape_fixed(4, 0.0), &fields));
        assert_format(7, &payload);
    }
}

#[test]
fn shape_pen_slots_keep_fill_aligned_and_unknown_line_fields_still_warn() {
    let mut payload = base(0.0);
    payload.extend(outline());
    let mut fields = 77_i32.to_le_bytes().to_vec();
    fields.extend([0xde, 0xad, 0, 0xff]);
    fields.extend((-123456_i32).to_le_bytes());
    fields.extend(shape_fields());
    payload.extend(frame(7, 4 | 8 | 16 | 32, &shape_fixed(4, 0.0), &fields));
    let parsed = sdocx::parse_bytes_detailed(&single(7, &payload)).unwrap();
    let shape = as_shape(parsed.document.pages[0].elements().next().unwrap());
    assert_eq!(shape.pen_name_id, Some(77));
    assert_eq!(shape.pen_data_field_3_raw, Some([0xde, 0xad, 0, 0xff]));
    assert_eq!(shape.pen_settings_id, Some(-123456));
    assert!(matches!(shape.fill, ShapePaint::Solid(0x40ff0000)));
    let Some(sdocx::ShapePaintSource::Color(source)) = &shape.fill_source else {
        panic!("expected retained fill source")
    };
    assert_eq!(source.solid_argb, 0x40ff0000);
    assert!(has_shape_warning(&parsed));
    let parsed = sdocx::parse_bytes_detailed(&single(8, &line(0, 16, b"future"))).unwrap();
    assert!(
        as_line(parsed.document.pages[0].elements().next().unwrap())
            .path_data
            .is_empty()
    );
    assert!(has_shape_warning(&parsed));
}

#[test]
fn zero_pen_slot_is_distinct_from_absent_and_old_json_defaults_to_absent() {
    for present in [false, true] {
        let mut payload = base(0.0);
        payload.extend(outline());
        let mut fields = if present { vec![0; 4] } else { Vec::new() };
        fields.extend(shape_fields());
        payload.extend(frame(
            7,
            32 | if present { 8 } else { 0 },
            &shape_fixed(4, 0.0),
            &fields,
        ));
        let parsed = sdocx::parse_bytes_detailed(&single(7, &payload)).unwrap();
        let shape = as_shape(parsed.document.pages[0].elements().next().unwrap());
        assert_eq!(shape.pen_data_field_3_raw, present.then_some([0; 4]));
        assert!(matches!(shape.fill, ShapePaint::Solid(0x40ff0000)));
        assert!(shape.fill_source.is_some());
        assert_eq!(has_shape_warning(&parsed), present);
        #[cfg(feature = "serde")]
        {
            let mut json = serde_json::to_value(shape).unwrap();
            let restored: NativeShape = serde_json::from_value(json.clone()).unwrap();
            assert_eq!(restored.pen_data_field_3_raw, shape.pen_data_field_3_raw);
            json.as_object_mut().unwrap().remove("pen_data_field_3_raw");
            let restored: NativeShape = serde_json::from_value(json).unwrap();
            assert!(restored.pen_data_field_3_raw.is_none());
        }
    }
}

#[test]
fn short_pen_slots_cannot_consume_the_following_frame() {
    for length in 0..4 {
        let mut payload = base(0.0);
        payload.extend(outline());
        payload.extend(frame(7, 8, &shape_fixed(4, 0.0), &[0xff; 3][..length]));
        payload.extend(frame(66, 0, b"future", &[]));
        assert_format(7, &payload);
    }
}

#[test]
fn future_frames_masks_and_outline_settings_are_retained_or_reported() {
    let mut native_style = style(5.0);
    native_style[4..12].copy_from_slice(&[1, 3, 2, 1, 2, 1, 4, 2]);
    let mut payload = base(0.0);
    payload.extend(frame(6, 8, &base_fixed(), &sized(&native_style)));
    let mut shape_frame = frame(7, 32, &shape_fixed(4, 0.0), &shape_fields());
    shape_frame[17] = 1; // field 32, after the known fill
    payload.extend(shape_frame);
    payload.extend(frame(66, 0, b"future", &[]));
    let parsed = sdocx::parse_bytes_detailed(&single(7, &payload)).unwrap();
    let shape = as_shape(parsed.document.pages[0].elements().next().unwrap());
    assert_eq!(shape.style.compound, 1);
    assert_eq!(shape.style.dash, 3);
    assert_eq!(shape.style.begin_arrow, [2, 1]);
    assert_eq!(shape.style.end_arrow, [4, 2]);
    assert!(matches!(shape.fill, ShapePaint::Solid(0x40ff0000)));
    assert!(has_shape_warning(&parsed));
}

#[test]
fn no_outline_and_unsupported_gradient_are_distinct_from_solid_black() {
    for kind in [1, 2, 99] {
        let effect = color(true, kind, 0xff000000);
        let mut payload = base(0.0);
        payload.extend(frame(6, 4, &base_fixed(), &sized(&effect)));
        payload.extend(frame(7, 32, &shape_fixed(4, 0.0), &shape_fields()));
        let parsed = sdocx::parse_bytes_detailed(&single(7, &payload)).unwrap();
        let shape = as_shape(parsed.document.pages[0].elements().next().unwrap());
        if kind == 2 {
            assert!(matches!(shape.style.paint, ShapePaint::None));
        } else {
            assert!(
                matches!(&shape.style.paint, ShapePaint::Unsupported { data, .. } if data == &effect)
            );
        }
        assert_eq!(has_shape_warning(&parsed), kind != 2);
        #[cfg(feature = "render")]
        assert!(
            sdocx::render_document_svg(&parsed.document, &Default::default())[0]
                .svg
                .contains("stroke=\"none\"")
        );
    }
}

#[test]
fn shapes_and_lines_own_precise_magnetic_points_and_opaque_connections() {
    let points = [
        [1.0000000000000002, -0.0],
        [-2.0000000000000004, f64::from_bits(1)],
    ];
    let mut connections = 1_u32.to_le_bytes().to_vec();
    connections.extend(b"\xffunresolved");
    let mut fixed = (points.len() as u32).to_le_bytes().to_vec();
    for point in &points {
        fixed.extend(numbers(point));
    }
    fixed.extend(sized(&connections));
    fixed.push(0);
    let source_frame = frame(6, 0, &fixed, &[]);
    let objects = [7, 8].map(|kind| object(kind, &with_shape_base(kind, &source_frame), &[]));
    let parsed = sdocx::parse_bytes_detailed(&archive(&page(&[objects.to_vec()], 0, &[]))).unwrap();
    assert!(has_shape_warning(&parsed));
    let elements: Vec<_> = parsed.document.pages[0].elements().collect();
    assert_eq!(elements.len(), 2);
    for element in elements {
        let source = shape_base_source(element).unwrap();
        assert_eq!(source.connection_data, connections);
        assert_eq!(
            source
                .magnetic_points
                .iter()
                .map(|point| point.map(f64::to_bits))
                .collect::<Vec<_>>(),
            points.map(|point| point.map(f64::to_bits))
        );
    }
}

fn with_shape_base(kind: u8, source_frame: &[u8]) -> Vec<u8> {
    let mut payload = base(0.0);
    payload.extend(source_frame);
    payload.extend(match kind {
        7 => frame(7, 32, &shape_fixed(4, 0.0), &shape_fields()),
        8 => frame(8, 0, &line_fixed(0), &[]),
        _ => panic!("expected shape or line"),
    });
    payload
}

fn shape_base_source(element: &PageElement) -> Option<&sdocx::ShapeBaseSource> {
    match element {
        PageElement::Shape(shape) => shape.base_source.as_deref(),
        PageElement::Line(line) => line.base_source.as_deref(),
        _ => panic!("expected shape or line"),
    }
}

#[test]
fn parsed_empty_shape_base_is_present_and_older_json_can_omit_it() {
    for (kind, payload) in [(7, shape(4)), (8, line(0, 0, &[]))] {
        let parsed = sdocx::parse_bytes_detailed(&single(kind, &payload)).unwrap();
        assert!(!has_shape_warning(&parsed));
        let element = parsed.document.pages[0].elements().next().unwrap();
        let source = shape_base_source(element).unwrap();
        assert!(source.magnetic_points.is_empty());
        assert_eq!(source.connection_data, [0; 4]);
        #[cfg(feature = "serde")]
        {
            let mut json = serde_json::to_value(element).unwrap();
            let restored: PageElement = serde_json::from_value(json.clone()).unwrap();
            let source = shape_base_source(&restored).unwrap();
            assert!(source.magnetic_points.is_empty());
            assert_eq!(source.connection_data, [0; 4]);
            json.as_object_mut()
                .unwrap()
                .values_mut()
                .next()
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove("base_source");
            let restored: PageElement = serde_json::from_value(json).unwrap();
            assert!(shape_base_source(&restored).is_none());
        }
    }
}

#[test]
fn connection_counts_and_shape_paths_are_bounded_before_allocation() {
    for fixed in [
        u32::MAX.to_le_bytes().to_vec(),
        [0_u32, u32::MAX]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect(),
    ] {
        let mut payload = base(0.0);
        payload.extend(frame(6, 0, &fixed, &[]));
        payload.extend(frame(7, 0, &shape_fixed(4, 0.0), &[]));
        assert_format(7, &payload);
    }
    let mut fixed = shape_fixed(4, 0.0);
    fixed[40..44].copy_from_slice(&u32::MAX.to_le_bytes()); // path length
    let mut payload = base(0.0);
    payload.extend(outline());
    payload.extend(frame(7, 0, &fixed, &[]));
    assert_format(7, &payload);
}

#[test]
fn unsupported_arc_oval_and_missing_move_paths_remain_bounded() {
    for path in [
        native_path(&[(1, &[0.0, 0.0]), (5, &[0.0, 0.0, 40.0, 40.0, 0.0, 90.0])]),
        native_path(&[(7, &[0.0, 0.0, 40.0, 40.0])]),
        native_path(&[(2, &[30.0, 40.0])]),
        native_path(&[]),
    ] {
        let parsed = sdocx::parse_bytes_detailed(&single(8, &line(2, 8, &path))).unwrap();
        assert_eq!(
            as_line(parsed.document.pages[0].elements().next().unwrap()).path_data,
            path
        );
        assert!(has_shape_warning(&parsed));
        #[cfg(feature = "render")]
        {
            let svg = &sdocx::render_document_svg(&parsed.document, &Default::default())[0].svg;
            assert!(!svg.contains("<path "));
            assert!(!svg.contains("<line "));
        }
    }
}

fn shape_with_path(kind: u32, path: &[u8]) -> Vec<u8> {
    let mut fixed = shape_fixed(kind, 30.0);
    // Replace the empty sized path; retain one adjustment control point.
    fixed.splice(
        40..45,
        [sized(path), vec![1], numbers(&[75.0, 42.0])].concat(),
    );
    [base(0.0), outline(), frame(7, 32, &fixed, &shape_fields())].concat()
}

#[test]
fn native_shape_paths_override_templates_and_already_include_rotation() {
    let path = native_path(&[
        (1, &[10.0, 20.0]),
        (2, &[90.0, 25.0]),
        (3, &[110.0, 30.0, 80.0, 70.0]),
        (4, &[60.0, 90.0, 20.0, 60.0, 10.0, 20.0]),
        (6, &[]),
    ]);
    for kind in [1, 2, 4, 6, 11, 999] {
        let parsed =
            sdocx::parse_bytes_detailed(&single(7, &shape_with_path(kind, &path))).unwrap();
        assert!(!has_shape_warning(&parsed));
        let shape = as_shape(parsed.document.pages[0].elements().next().unwrap());
        assert_eq!(shape.path_data, path);
        assert_eq!(shape.control_points, [[75.0, 42.0]]);
        assert_eq!(shape.shape_type, kind);
        #[cfg(feature = "render")]
        {
            let svg = &sdocx::render_document_svg(&parsed.document, &Default::default())[0].svg;
            assert_svg_path(
                svg,
                "M 10.00 20.00 L 90.00 25.00 Q 110.00 30.00 80.00 70.00 C 60.00 90.00 20.00 60.00 10.00 20.00 Z",
            );
            assert!(svg.contains("fill=\"#ff0000\" fill-opacity=\"0.2510\""));
            assert!(!svg.contains("rotate("));
            assert!(!svg.contains("<ellipse "));
            assert!(!svg.contains("<polygon "));
        }
    }
}

#[cfg(feature = "render")]
#[test]
fn saved_vector_paths_keep_fractional_commands_and_source_bytes() {
    let a = 1025.0 / 1024.0;
    let b = 1026.0 / 1024.0;
    let path = native_path(&[
        (1, &[a, a]),
        (2, &[b, b]),
        (3, &[a, b, b, a]),
        (4, &[b, a, a, b, b, b]),
        (6, &[]),
    ]);
    for (kind, payload) in [(7, shape_with_path(4, &path)), (8, line(2, 8, &path))] {
        let parsed = sdocx::parse_bytes_detailed(&single(kind, &payload)).unwrap();
        let rendered = sdocx::render_document_svg(&parsed.document, &Default::default());
        assert!(rendered[0].geometry_diagnostics.is_empty());
        let svg = &rendered[0].svg;
        let xml = roxmltree::Document::parse(svg).unwrap();
        let paths: Vec<_> = xml
            .descendants()
            .filter(|node| node.has_tag_name("path"))
            .collect();
        assert_eq!(paths.len(), 1);
        let commands: Vec<_> = svgtypes::PathParser::from(paths[0].attribute("d").unwrap())
            .map(|command| {
                use svgtypes::PathSegment::*;
                let (verb, values) = match command.unwrap() {
                    MoveTo { abs: true, x, y } => ("M", vec![x, y]),
                    LineTo { abs: true, x, y } => ("L", vec![x, y]),
                    Quadratic {
                        abs: true,
                        x1,
                        y1,
                        x,
                        y,
                    } => ("Q", vec![x1, y1, x, y]),
                    CurveTo {
                        abs: true,
                        x1,
                        y1,
                        x2,
                        y2,
                        x,
                        y,
                    } => ("C", vec![x1, y1, x2, y2, x, y]),
                    ClosePath { .. } => ("Z", vec![]),
                    command => panic!("unexpected emitted command {command:?}"),
                };
                (
                    verb,
                    values
                        .into_iter()
                        .map(|value| (value as f32).to_bits())
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        let (a_bits, b_bits) = (0x3f80_2000, 0x3f80_4000);
        assert_eq!(
            commands,
            [
                ("M", vec![a_bits, a_bits]),
                ("L", vec![b_bits, b_bits]),
                ("Q", vec![a_bits, b_bits, b_bits, a_bits]),
                ("C", vec![b_bits, a_bits, a_bits, b_bits, b_bits, b_bits]),
                ("Z", vec![]),
            ]
        );
        let retained = match parsed.document.pages[0].elements().next().unwrap() {
            PageElement::Shape(shape) => &shape.path_data,
            PageElement::Line(line) => &line.path_data,
            _ => panic!("expected saved vector"),
        };
        assert_eq!(retained, &path);
    }
}

#[cfg(feature = "render")]
#[test]
fn saved_vector_straight_line_attributes_keep_f64_endpoints() {
    let bits = [
        0x3ff0_0400_0000_0001,
        0x4000_0400_0000_0001,
        0x3ff0_1000_0000_0001,
        0x4000_0600_0000_0001,
    ];
    let endpoints = bits.map(f64::from_bits);
    let mut fixed = line_fixed(0);
    fixed[3..35].copy_from_slice(&numbers(&endpoints));
    let payload = [base(90.0), outline(), frame(8, 0, &fixed, &[])].concat();
    let parsed = sdocx::parse_bytes_detailed(&single(8, &payload)).unwrap();
    let svg = &sdocx::render_document_svg(&parsed.document, &Default::default())[0].svg;
    let xml = roxmltree::Document::parse(svg).unwrap();
    let node = xml
        .descendants()
        .find(|node| node.has_tag_name("line"))
        .unwrap();
    let actual = ["x1", "y1", "x2", "y2"].map(|attribute| {
        node.attribute(attribute)
            .unwrap()
            .parse::<f64>()
            .unwrap()
            .to_bits()
    });
    assert_eq!(actual, bits);
    let retained = as_line(parsed.document.pages[0].elements().next().unwrap());
    assert_eq!(retained.begin.map(f64::to_bits), [bits[0], bits[1]]);
    assert_eq!(retained.end.map(f64::to_bits), [bits[2], bits[3]]);
    assert!(retained.path_data.is_empty());
}

#[test]
fn unsupported_shape_paths_do_not_fall_back_to_plausible_geometry() {
    let mut trailing = native_path(&[(1, &[1.0, 2.0]), (2, &[3.0, 4.0])]);
    trailing.push(0xff);
    for (index, path) in [
        native_path(&[(1, &[1.0, 2.0]), (99, &[])]),
        native_path(&[(1, &[1.0, 2.0]), (5, &[0.0; 6])]),
        native_path(&[(2, &[1.0, 2.0])]),
        trailing,
    ]
    .into_iter()
    .enumerate()
    {
        let parsed = sdocx::parse_bytes_detailed(&single(7, &shape_with_path(4, &path))).unwrap();
        assert!(has_shape_warning(&parsed));
        assert_eq!(
            as_shape(parsed.document.pages[0].elements().next().unwrap()).path_data,
            path
        );
        #[cfg(feature = "render")]
        {
            let rendered = sdocx::render_document_svg(&parsed.document, &Default::default());
            assert!(!rendered[0].svg.contains("<path "));
            assert!(!rendered[0].svg.contains("rotate("));
            assert_geometry_issue(
                &parsed,
                &rendered[0],
                if index == 3 {
                    sdocx::GeometryDiagnosticKind::InvalidPath
                } else {
                    sdocx::GeometryDiagnosticKind::UnsupportedPath
                },
            );
        }
        #[cfg(not(feature = "render"))]
        let _ = index;
    }
    let path = native_path(&[(1, &[1.0, 2.0]), (2, &[3.0, 4.0])]);
    for end in 1..path.len() {
        assert_format(7, &shape_with_path(11, &path[..end]));
    }
    assert_format(
        7,
        &shape_with_path(6, &native_path(&[(1, &[f64::NAN, 0.0])])),
    );
    #[cfg(feature = "render")]
    {
        let path = native_path(&[(1, &[1.0, 2.0]), (2, &[f64::MAX, 4.0])]);
        for (kind, payload) in [(7, shape_with_path(4, &path)), (8, line(0, 8, &path))] {
            let parsed = sdocx::parse_bytes_detailed(&single(kind, &payload)).unwrap();
            let rendered = sdocx::render_document_svg(&parsed.document, &Default::default());
            assert!(!rendered[0].svg.contains("<path "));
            assert!(!rendered[0].svg.contains("<line "));
            assert!(!rendered[0].svg.contains("stroke=\"#0000ff\""));
            assert_geometry_issue(
                &parsed,
                &rendered[0],
                sdocx::GeometryDiagnosticKind::UnrepresentablePath,
            );
        }
    }
}

#[cfg(feature = "render")]
#[test]
fn geometry_reports_follow_admitted_roots_and_reset_for_cached_replay() {
    use sdocx::{GeometryDiagnostic, GeometryDiagnosticKind, PageObject, PageObjectContent};

    let parsed = sdocx::parse_bytes_detailed(&single(7, &shape(900))).unwrap();
    let mut document = parsed.document;
    let mut child = document.pages[0].objects.remove(0);
    child.render_layer = sdocx::ObjectRenderLayer::Top;
    let mut excluded = child.clone();
    excluded.source_offset = None;
    let PageObjectContent::Element(PageElement::Shape(shape)) = &mut excluded.content else {
        panic!("expected shape")
    };
    shape.metadata.uuid = "excluded".into();
    document.pages[0].objects = vec![
        excluded,
        PageObject {
            render_layer: sdocx::ObjectRenderLayer::Base,
            source_offset: None,
            content: PageObjectContent::Container(vec![child.clone()]),
        },
    ];
    let layout = sdocx::layout_document(&document);
    let fonts = sdocx::fonts::FontBook::default();
    let mut cache = sdocx::DocumentTextCache::default();
    for replay in [false, true, false] {
        let rendered = if replay {
            cache.render_layout_page_replay_svg(&document, &layout, 0, &Default::default(), &fonts)
        } else {
            cache.render_layout_page_svg(&document, &layout, 0, &Default::default(), &fonts)
        }
        .unwrap();
        assert_eq!(
            rendered.geometry_diagnostics,
            [GeometryDiagnostic {
                source_offset: child.source_offset,
                object_uuid: "sh".into(),
                kind: GeometryDiagnosticKind::UnsupportedShapeTemplate,
            }]
        );
        assert!(rendered.object_diagnostics.is_empty());
    }
}

#[cfg(feature = "render")]
#[test]
fn mutated_geometry_reports_invalid_paths_and_bounds_without_changing_source() {
    use sdocx::GeometryDiagnosticKind;

    let parsed = sdocx::parse_bytes_detailed(&single(7, &shape(4))).unwrap();
    for (path, invalid_bounds, expected) in [
        (vec![1, 0], false, GeometryDiagnosticKind::InvalidPath),
        (
            native_path(&[(1, &[f64::NAN, 0.0])]),
            false,
            GeometryDiagnosticKind::InvalidPath,
        ),
        (vec![], true, GeometryDiagnosticKind::InvalidGeometry),
    ] {
        let mut document = parsed.document.clone();
        let PageElement::Shape(shape) = document.pages[0].elements_mut().next().unwrap() else {
            panic!("expected shape")
        };
        shape.path_data = path.clone();
        if invalid_bounds {
            shape.geometry_bbox.x_max = shape.geometry_bbox.x_min;
        }
        let rendered = sdocx::render_document_svg(&document, &Default::default());
        assert_geometry_issue(&parsed, &rendered[0], expected);
        assert_eq!(
            as_shape(document.pages[0].elements().next().unwrap()).path_data,
            path
        );
    }
}

#[cfg(all(feature = "render", feature = "serde"))]
#[test]
fn rendered_pages_without_geometry_reports_still_deserialize() {
    let parsed = sdocx::parse_bytes_detailed(&single(7, &shape(4))).unwrap();
    let rendered = sdocx::render_document_svg(&parsed.document, &Default::default()).remove(0);
    let mut old = serde_json::to_value(&rendered).unwrap();
    old.as_object_mut().unwrap().remove("geometry_diagnostics");
    assert_eq!(
        serde_json::from_value::<sdocx::RenderedPage>(old).unwrap(),
        rendered
    );
}

#[cfg(feature = "render")]
fn assert_svg_path(svg: &str, expected: &str) {
    let document = roxmltree::Document::parse(svg).unwrap();
    let commands = |data: &str| {
        svgtypes::PathParser::from(data)
            .map(|command| match command.unwrap() {
                svgtypes::PathSegment::ClosePath { .. } => {
                    svgtypes::PathSegment::ClosePath { abs: false }
                }
                command => command,
            })
            .collect::<Vec<_>>()
    };
    let expected = commands(expected);
    assert!(
        document
            .descendants()
            .filter_map(|node| node.attribute("d"))
            .any(|data| commands(data) == expected)
    );
}
