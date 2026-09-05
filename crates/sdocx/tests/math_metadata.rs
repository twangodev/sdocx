mod support;

use sdocx::{
    DiagnosticCode, Error, MathAngleType, MathMetadata, ObjectType, ParseLimits, PlotMetadata,
};
use support::{archive, object, page};

fn frame(kind: i16, properties: &[u8], fields: &[u8], fixed: &[u8], flexible: &[u8]) -> Vec<u8> {
    let offset = 12 + properties.len() + fields.len() + fixed.len();
    let mut bytes = ((offset + flexible.len()) as u32).to_le_bytes().to_vec();
    bytes.extend(kind.to_le_bytes());
    bytes.extend((offset as u32).to_le_bytes());
    bytes.push(properties.len() as u8);
    bytes.extend(properties);
    bytes.push(fields.len() as u8);
    bytes.extend(fields);
    bytes.extend(fixed);
    bytes.extend(flexible);
    bytes
}

fn base() -> Vec<u8> {
    let mut fixed = 5500_u32.to_le_bytes().to_vec();
    fixed.extend(36_u16.to_le_bytes());
    fixed.extend(b"00000000-0000-0000-0000-000000000021");
    fixed.extend(9876_i64.to_le_bytes());
    for value in [10.0_f64, 20.0, 300.0, 400.0] {
        fixed.extend(value.to_le_bytes());
    }
    fixed.extend([0; 5]);
    frame(0, &[0], &[1], &fixed, &15.5_f32.to_le_bytes())
}

fn math(properties: &[u8], fields: &[u8], fixed: &[u8], flexible: &[u8]) -> Vec<u8> {
    [base(), frame(21, properties, fields, fixed, flexible)].concat()
}

fn decode(payload: &[u8]) -> sdocx::Result<MathMetadata> {
    decode_with_limits(payload, &ParseLimits::default())
}

fn decode_with_limits(payload: &[u8], limits: &ParseLimits) -> sdocx::Result<MathMetadata> {
    let bytes = page(&[vec![object(21, payload, &[])]], 0, &[]);
    let stored = sdocx::parse_stored_page_bytes(&bytes)?;
    stored.layers.layers[0].objects[0].math_metadata_with_limits(&bytes, limits)
}

fn formulas() -> Vec<Vec<u8>> {
    vec![
        [base(), frame(11, &[1], &[], &[0xa1, 0xa2], &[])].concat(),
        [base(), frame(11, &[], &[128], &[], &[0xb1, 0xb2, 0xb3])].concat(),
    ]
}

fn fields() -> Vec<Vec<u8>> {
    let mut objects = 2_u32.to_le_bytes().to_vec();
    for formula in formulas() {
        objects.extend((formula.len() as u32).to_le_bytes());
        objects.extend(formula);
    }
    let mut margins = Vec::new();
    for value in [1.25_f64, 2.5, 3.75, 4.0] {
        margins.extend(value.to_le_bytes());
    }
    let mut plots = 2_u32.to_le_bytes().to_vec();
    for uuid in [
        b"00000000-0000-0000-0000-000000000020",
        b"00000000-0000-0000-0000-000000000040",
    ] {
        plots.extend((uuid.len() as u16).to_le_bytes());
        plots.extend(uuid);
    }
    vec![objects, margins, 1_u32.to_le_bytes().to_vec(), plots]
}

#[test]
fn math_envelopes_preserve_formulas_and_decode_margins_angles_and_plot_references() {
    let payload = math(&[1], &[15, 0], &[], &fields().concat());
    let bytes = page(&[vec![object(21, &payload, &[])]], 0, &[]);
    let parsed = sdocx::parse_bytes_detailed(&archive(&bytes)).unwrap();
    let object = &parsed.stored_pages[0].page.layers.layers[0].objects[0];
    let value = object.math_metadata(&bytes).unwrap();
    assert!(value.editable);
    assert_eq!(value.base.format_version, 5500);
    assert_eq!(value.base.uuid, "00000000-0000-0000-0000-000000000021");
    assert_eq!(value.base.modified_time_raw, 9876);
    assert_eq!(value.base.rotation_degrees, Some(15.5));
    assert_eq!(value.base.bbox.x_min, 10.0);
    assert_eq!(value.base.bbox.y_max, 400.0);
    assert_eq!(value.formula_objects, formulas());
    let margins = value.margins.unwrap();
    assert_eq!(
        [margins.left, margins.top, margins.right, margins.bottom],
        [1.25, 2.5, 3.75, 4.0]
    );
    assert_eq!(value.angle_type, Some(MathAngleType::Radian));
    assert_eq!(
        value.connected_plot_uuids,
        [
            "00000000-0000-0000-0000-000000000020",
            "00000000-0000-0000-0000-000000000040"
        ]
    );
    assert_eq!(value.property_mask, [1]);
    assert_eq!(value.field_mask, [15, 0]);
    assert!(value.fixed_trailing_data.is_empty());
    assert!(value.flexible_trailing_data.is_empty());
    assert!(value.trailing_data.is_empty());
    assert!(parsed.document.pages[0].elements.is_empty());
    assert!(
        parsed
            .report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::UnsupportedObjectType)
    );
}

#[test]
fn every_math_field_is_optional_and_truncated_fields_cannot_borrow_later_frames() {
    for (bit, data) in fields().into_iter().enumerate() {
        decode(&math(&[], &[1 << bit], &[], &data)).unwrap();
        for length in 0..data.len() {
            let payload = [
                math(&[], &[1 << bit], &[], &data[..length]),
                frame(99, &[], &[], &[0; 1024], &[]),
            ]
            .concat();
            assert!(decode(&payload).is_err(), "bit {bit}, length {length}");
        }
    }
}

#[test]
fn absent_math_fields_and_unknown_angle_values_remain_distinct() {
    let mut payload = math(&[0], &[0, 0], &[], &[]);
    let math_start = base().len();
    payload[math_start + 6..math_start + 10].copy_from_slice(&0_u32.to_le_bytes());
    let value = decode(&payload).unwrap();
    assert!(!value.editable);
    assert!(value.formula_objects.is_empty());
    assert!(value.margins.is_none());
    assert!(value.angle_type.is_none());
    assert!(value.connected_plot_uuids.is_empty());
    for (raw, expected) in [
        (0_u32, MathAngleType::Degree),
        (1, MathAngleType::Radian),
        (2, MathAngleType::All),
        (u32::MAX, MathAngleType::Other(u32::MAX)),
    ] {
        let value = decode(&math(&[2], &[4], &[], &raw.to_le_bytes())).unwrap();
        assert!(!value.editable);
        assert_eq!(value.angle_type, Some(expected));
    }
}

#[test]
fn future_math_masks_and_extensions_are_preserved_at_their_original_boundaries() {
    let mut payload = math(
        &[1, 0, 0, 0, 128],
        &[4, 0, 0, 0, 1],
        &[0xa1, 0xa2],
        &[0, 0, 0, 0, 0xb1, 0xb2, 0xb3],
    );
    payload.extend([0xc1, 0xc2, 0xc3, 0xc4]);
    let value = decode(&payload).unwrap();
    assert!(value.editable);
    assert_eq!(value.angle_type, Some(MathAngleType::Degree));
    assert_eq!(value.property_mask, [1, 0, 0, 0, 128]);
    assert_eq!(value.field_mask, [4, 0, 0, 0, 1]);
    assert_eq!(value.fixed_trailing_data, [0xa1, 0xa2]);
    assert_eq!(value.flexible_trailing_data, [0xb1, 0xb2, 0xb3]);
    assert_eq!(value.trailing_data, [0xc1, 0xc2, 0xc3, 0xc4]);
}

#[test]
fn math_entry_limits_cover_formulas_and_plot_references_together() {
    let payload = math(&[], &[15], &[], &fields().concat());
    let mut limits = ParseLimits {
        max_objects_per_page: 4,
        ..Default::default()
    };
    decode_with_limits(&payload, &limits).unwrap();
    limits.max_objects_per_page = 3;
    assert!(matches!(
        decode_with_limits(&payload, &limits),
        Err(Error::LimitExceeded {
            resource: "math entries",
            limit: 3,
            actual: 4
        })
    ));
    for bit in [0, 3] {
        let payload = math(&[], &[1 << bit], &[], &u32::MAX.to_le_bytes());
        assert!(matches!(decode(&payload), Err(Error::LimitExceeded { .. })));
        let payload = math(&[], &[1 << bit], &[], &3_u32.to_le_bytes());
        assert!(matches!(decode(&payload), Err(Error::Format(_))));
    }
    limits.max_entry_size = payload.len() as u64 - 1;
    assert!(matches!(
        decode_with_limits(&payload, &limits),
        Err(Error::LimitExceeded {
            resource: "math payload size",
            ..
        })
    ));
}

#[test]
fn math_metadata_requires_the_declared_object_type_and_complete_frame_chain() {
    let payload = math(&[], &[], &[], &[]);
    let bytes = page(&[vec![object(21, &payload, &[])]], 0, &[]);
    let stored = sdocx::parse_stored_page_bytes(&bytes).unwrap();
    let mut object = stored.layers.layers[0].objects[0].clone();
    object.object_type = ObjectType::Formula;
    assert!(object.math_metadata(&bytes).is_err());
    object.object_type = ObjectType::Math;
    object.payload_offset = usize::MAX;
    assert!(object.math_metadata(&bytes).is_err());
    for length in 0..payload.len() {
        assert!(decode(&payload[..length]).is_err(), "length {length}");
    }
    let wrong_chain = [base(), frame(11, &[], &[], &[], &[])].concat();
    assert!(decode(&wrong_chain).is_err());
    let wrong_base = [frame(99, &[], &[], &[], &[]), frame(21, &[], &[], &[], &[])].concat();
    assert!(decode(&wrong_base).is_err());
}

#[test]
fn invalid_math_values_and_declared_lengths_fail_inside_their_field() {
    let mut margin = fields()[1].clone();
    margin[..8].copy_from_slice(&f64::INFINITY.to_le_bytes());
    assert!(decode(&math(&[], &[2], &[], &margin)).is_err());
    let plot = [1, 0, 0, 0, 1, 0, 0xff];
    assert!(decode(&math(&[], &[8], &[], &plot)).is_err());
    let plot = [1, 0, 0, 0, 0xff, 0xff];
    assert!(decode(&math(&[], &[8], &[], &plot)).is_err());
    let formula = [1, 0, 0, 0, 0xff, 0xff, 0xff, 0xff];
    assert!(decode(&math(&[], &[1], &[], &formula)).is_err());
}

fn plot(fields: &[u8], flexible: &[u8]) -> Vec<u8> {
    [base(), frame(20, &[0], fields, &[], flexible)].concat()
}

fn decode_plot(payload: &[u8], limits: &ParseLimits) -> sdocx::Result<PlotMetadata> {
    let bytes = page(&[vec![object(20, payload, &[])]], 0, &[]);
    let stored = sdocx::parse_stored_page_bytes(&bytes)?;
    stored.layers.layers[0].objects[0].plot_metadata_with_limits(&bytes, limits)
}

fn byte_string(value: &str) -> Vec<u8> {
    [
        (value.len() as u16).to_le_bytes().as_slice(),
        value.as_bytes(),
    ]
    .concat()
}

fn graph(latex: &str, color: u32, width: f32, visibility: u8, substitutions: &[&str]) -> Vec<u8> {
    let mut bytes = byte_string(latex);
    bytes.extend(color.to_le_bytes());
    bytes.extend(width.to_le_bytes());
    bytes.push(visibility);
    bytes.extend((substitutions.len() as u32).to_le_bytes());
    for substitution in substitutions {
        bytes.extend(byte_string(substitution));
    }
    bytes
}

fn plot_fields() -> Vec<Vec<u8>> {
    vec![
        17_u32.to_le_bytes().to_vec(),
        [
            (-10.0_f64).to_le_bytes(),
            10.0_f64.to_le_bytes(),
            10.0_f64.to_le_bytes(),
            (-10.0_f64).to_le_bytes(),
        ]
        .concat(),
        0xff123456_u32.to_le_bytes().to_vec(),
        0x80112233_u32.to_le_bytes().to_vec(),
        [
            2_u32.to_le_bytes().to_vec(),
            graph("x^2", 0xff445566, 2.5, 1, &["y=2", "π"]),
            graph("x", 0xff778899, 1.5, 0, &[]),
        ]
        .concat(),
        1_u32.to_le_bytes().to_vec(),
    ]
}

#[test]
fn plot_records_expose_graph_expressions_and_styles_without_implying_render_support() {
    let payload = plot(&[63, 0], &plot_fields().concat());
    let bytes = page(&[vec![object(20, &payload, &[])]], 0, &[]);
    let parsed = sdocx::parse_bytes_detailed(&archive(&bytes)).unwrap();
    let value = parsed.stored_pages[0].page.layers.layers[0].objects[0]
        .plot_metadata(&bytes)
        .unwrap();
    assert_eq!(value.base.uuid, "00000000-0000-0000-0000-000000000021");
    assert_eq!(value.legacy_field_0, Some(17));
    let rect = value.coordinate_rect.unwrap();
    assert_eq!(
        [rect.x_min, rect.y_min, rect.x_max, rect.y_max],
        [-10.0, 10.0, 10.0, -10.0]
    );
    assert_eq!(value.coordinate_color, Some(0xff123456));
    assert_eq!(value.background_color, Some(0x80112233));
    assert_eq!(value.angle_type, Some(MathAngleType::Radian));
    assert_eq!(value.graphs.len(), 2);
    assert_eq!(value.graphs[0].latex, "x^2");
    assert_eq!(value.graphs[0].substitution_latex, ["y=2", "π"]);
    assert_eq!(value.graphs[0].color, 0xff445566);
    assert_eq!(value.graphs[0].line_width, 2.5);
    assert!(value.graphs[0].is_visible());
    assert_eq!(value.graphs[1].latex, "x");
    assert!(value.graphs[1].substitution_latex.is_empty());
    assert!(!value.graphs[1].is_visible());
    assert_eq!(value.field_mask, [63, 0]);
    assert!(value.flexible_trailing_data.is_empty());
    assert!(parsed.document.pages[0].elements.is_empty());
    assert!(parsed.report.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::UnsupportedObjectType
            && diagnostic.message.contains("type 20")
    }));
}

#[test]
fn plot_fields_reject_all_truncated_prefixes_even_with_a_following_frame() {
    let limits = ParseLimits::default();
    for (bit, data) in plot_fields().into_iter().enumerate() {
        decode_plot(&plot(&[1 << bit], &data), &limits).unwrap();
        for length in 0..data.len() {
            let payload = [
                plot(&[1 << bit], &data[..length]),
                frame(99, &[], &[], &[0; 512], &[]),
            ]
            .concat();
            assert!(
                decode_plot(&payload, &limits).is_err(),
                "bit {bit}, length {length}"
            );
        }
    }
}

#[test]
fn plot_future_masks_unknown_enums_and_visibility_bytes_are_preserved() {
    let limits = ParseLimits::default();
    let payload = [
        base(),
        frame(
            20,
            &[0, 0, 0, 0, 1],
            &[32, 0, 0, 0, 2],
            &[0xa1],
            &[99, 0, 0, 0, 0xb1, 0xb2],
        ),
        vec![0xc1, 0xc2],
    ]
    .concat();
    let value = decode_plot(&payload, &limits).unwrap();
    assert_eq!(value.angle_type, Some(MathAngleType::Other(99)));
    assert_eq!(value.property_mask, [0, 0, 0, 0, 1]);
    assert_eq!(value.field_mask, [32, 0, 0, 0, 2]);
    assert_eq!(value.fixed_trailing_data, [0xa1]);
    assert_eq!(value.flexible_trailing_data, [0xb1, 0xb2]);
    assert_eq!(value.trailing_data, [0xc1, 0xc2]);
    for raw in [0, 1, 2, 255] {
        let data = [1_u32.to_le_bytes().to_vec(), graph("", 0, 0.0, raw, &[])].concat();
        let value = decode_plot(&plot(&[16], &data), &limits).unwrap();
        assert_eq!(value.graphs[0].visibility_raw, raw);
        assert_eq!(value.graphs[0].is_visible(), raw == 1);
    }
    let mut payload = plot(&[0, 0], &[]);
    let start = base().len();
    payload[start + 6..start + 10].copy_from_slice(&0_u32.to_le_bytes());
    let value = decode_plot(&payload, &limits).unwrap();
    assert!(value.graphs.is_empty());
    assert!(value.coordinate_rect.is_none());
    assert!(value.angle_type.is_none());
}

#[test]
fn plot_limits_include_substitutions_and_count_utf8_text_in_utf16_units() {
    let payload = plot(&[16], &plot_fields()[4]);
    let mut limits = ParseLimits {
        max_objects_per_page: 4,
        ..Default::default()
    };
    decode_plot(&payload, &limits).unwrap();
    limits.max_objects_per_page = 3;
    assert!(matches!(
        decode_plot(&payload, &limits),
        Err(Error::LimitExceeded {
            resource: "math entries",
            limit: 3,
            actual: 4,
        })
    ));
    limits.max_objects_per_page = 4;
    limits.max_text_characters = 2;
    for (latex, substitutions) in [("𝑥", vec![]), ("", vec!["𝑥"])] {
        let data = [
            1_u32.to_le_bytes().to_vec(),
            graph(latex, 0, 1.0, 1, &substitutions),
        ]
        .concat();
        let payload = plot(&[16], &data);
        decode_plot(&payload, &limits).unwrap();
        let strict = ParseLimits {
            max_text_characters: 1,
            ..limits
        };
        assert!(matches!(
            decode_plot(&payload, &strict),
            Err(Error::LimitExceeded {
                resource: "text characters",
                limit: 1,
                actual: 2,
            })
        ));
    }
    let excessive_count = plot(&[16], &u32::MAX.to_le_bytes());
    assert!(matches!(
        decode_plot(&excessive_count, &limits),
        Err(Error::LimitExceeded { .. })
    ));
    let impossible_count = plot(&[16], &3_u32.to_le_bytes());
    assert!(matches!(
        decode_plot(&impossible_count, &limits),
        Err(Error::Format(_))
    ));
}

#[test]
fn plot_inspection_checks_outer_type_payload_size_and_utf8() {
    let payload = plot(&[0], &[]);
    let bytes = page(&[vec![object(21, &payload, &[])]], 0, &[]);
    let stored = sdocx::parse_stored_page_bytes(&bytes).unwrap();
    assert!(
        stored.layers.layers[0].objects[0]
            .plot_metadata(&bytes)
            .is_err()
    );
    let mut record = stored.layers.layers[0].objects[0].clone();
    record.object_type = ObjectType::Plot;
    record.payload_offset = usize::MAX;
    assert!(record.plot_metadata(&bytes).is_err());
    let limits = ParseLimits {
        max_entry_size: payload.len() as u64 - 1,
        ..Default::default()
    };
    assert!(matches!(
        decode_plot(&payload, &limits),
        Err(Error::LimitExceeded { .. })
    ));
    let mut invalid = [1_u32.to_le_bytes().to_vec(), graph("x", 0, 1.0, 1, &[])].concat();
    invalid[6] = 255;
    assert!(decode_plot(&plot(&[16], &invalid), &ParseLimits::default()).is_err());
    let wrong_frame = [base(), frame(21, &[], &[], &[], &[])].concat();
    assert!(decode_plot(&wrong_frame, &ParseLimits::default()).is_err());
}
