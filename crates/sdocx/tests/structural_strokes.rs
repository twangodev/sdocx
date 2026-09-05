mod support;

use support::{archive, object, page};

use sdocx::{Color, Error, ParseLimits, ParseOptions, Point};

// Small synthetic WDoc records built from the native serializer contract.
// These intentionally contain short strokes, unusual masks, children and
// multiple layers that the former fixed-offset walker could not traverse.
fn frame(kind: i16, properties: u16, fields: u32, fixed: &[u8], flexible: &[u8]) -> Vec<u8> {
    let offset = 18 + fixed.len();
    let mut data = Vec::new();
    data.extend_from_slice(&((offset + flexible.len()) as u32).to_le_bytes());
    data.extend_from_slice(&kind.to_le_bytes());
    data.extend_from_slice(&(offset as u32).to_le_bytes());
    data.push(2);
    data.extend_from_slice(&properties.to_le_bytes());
    data.push(4);
    data.extend_from_slice(&fields.to_le_bytes());
    data.extend_from_slice(fixed);
    data.extend_from_slice(flexible);
    data
}

fn base() -> Vec<u8> {
    let mut fixed = 5500_u32.to_le_bytes().to_vec();
    fixed.extend_from_slice(&36_u16.to_le_bytes());
    fixed.extend_from_slice(b"00000000-0000-0000-0000-000000000001");
    fixed.extend_from_slice(&1234_i64.to_le_bytes());
    for value in [10.0_f64, 17.0, 312.0, 21.0] {
        fixed.extend_from_slice(&value.to_le_bytes());
    }
    fixed.extend_from_slice(&0_i32.to_le_bytes());
    fixed.push(0);
    frame(0, 1 << 3, 0x6000, &fixed, &[0; 16])
}

fn stroke(properties: u16, count: u16, channels: &[u8], fields: u32, style: &[u8]) -> Vec<u8> {
    let mut fixed = count.to_le_bytes().to_vec();
    fixed.extend_from_slice(channels);
    fixed.extend_from_slice(&[1, 0]); // native tool/input type
    let mut data = base();
    data.extend(frame(1, properties, fields, &fixed, style));
    data
}

fn compressed(stylus: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&10.0_f64.to_le_bytes());
    bytes.extend_from_slice(&20.0_f64.to_le_bytes());
    bytes.extend_from_slice(&[0x90, 0x25, 0x48, 0x80, 0x20, 0, 0x40, 0]);
    bytes.extend_from_slice(&0.25_f32.to_le_bytes());
    bytes.extend_from_slice(&[0, 2, 0, 0x81]);
    bytes.extend_from_slice(&1000_i32.to_le_bytes());
    bytes.extend_from_slice(&[5, 0, 0xff, 0xff]);
    if stylus {
        bytes.extend_from_slice(&0.5_f32.to_le_bytes());
        bytes.extend_from_slice(&[0, 4, 0, 0x82]);
        bytes.extend_from_slice(&(-1.0_f32).to_le_bytes());
        bytes.extend_from_slice(&[0, 0x82, 0, 1]);
    }
    bytes
}

fn single(payload: &[u8]) -> Vec<u8> {
    archive(&page(&[vec![object(1, payload, &[])]], 0, &[]))
}

#[test]
fn hidden_strokes_and_containers_keep_their_records_without_visible_content() {
    let visible = stroke(1, 3, &compressed(false), 0, &[]);
    let mut hidden = visible.clone();
    hidden[11] &= !(1 << 3);
    let visible_child = object(1, &visible, &[]);
    let hidden_child = object(1, &hidden, &[]);
    let raw = page(
        &[vec![
            hidden_child.clone(),
            object(4, &hidden, std::slice::from_ref(&visible_child)),
            object(4, &base(), &[hidden_child, visible_child]),
        ]],
        0,
        &[],
    );
    let bytes = archive(&raw);
    let parsed = sdocx::parse_bytes_detailed(&bytes).unwrap();
    assert_eq!(parsed.document.pages[0].strokes.len(), 1);
    let objects = &parsed.stored_pages[0].page.layers.layers[0].objects;
    assert_eq!(objects.len(), 3);
    assert_eq!(objects[0].payload(&raw).unwrap(), hidden);
    assert!(!objects[0].base_metadata(&raw).unwrap().visible);
    assert_eq!(objects[1].children.len(), 1);
    assert_eq!(objects[1].children[0].payload(&raw).unwrap(), visible);
    assert_eq!(objects[2].children.len(), 2);
    assert_eq!(
        parsed
            .report
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == sdocx::DiagnosticCode::UnsupportedObjectType)
            .count(),
        1
    );
    let options = ParseOptions {
        limits: ParseLimits {
            max_strokes_per_page: 3,
            ..ParseLimits::default()
        },
        ..Default::default()
    };
    assert!(matches!(
        sdocx::parse_bytes_with_options(&bytes, &options),
        Err(Error::LimitExceeded {
            resource: "strokes per page",
            limit: 3,
            actual: 4,
        })
    ));
}

#[test]
fn unknown_objects_do_not_infer_child_visibility_from_a_common_looking_payload() {
    let child = object(1, &stroke(1, 3, &compressed(false), 0, &[]), &[]);
    let mut hidden = base();
    hidden[11] = 0;
    let raw = page(&[vec![object(250, &hidden, &[child])]], 0, &[]);
    let parsed = sdocx::parse_bytes_detailed(&archive(&raw)).unwrap();
    assert_eq!(parsed.document.pages[0].strokes.len(), 1);
    assert!(
        parsed
            .report
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code == sdocx::DiagnosticCode::UnknownObjectType })
    );
}

#[test]
fn unreadable_base_metadata_still_reaches_the_supported_object_decoder() {
    for payload in [Vec::new(), vec![0; 32], frame(0, 0, 0, &[], &[])] {
        assert!(matches!(
            sdocx::parse_bytes(&single(&payload)),
            Err(Error::Format(_))
        ));
    }
}

#[test]
fn short_strokes_use_the_declared_count_and_property_selected_channels() {
    for properties in [0x25, 0x05, 0x65, 0x425] {
        let data = single(&stroke(properties, 3, &compressed(true), 0, &[]));
        let doc = sdocx::parse_bytes(&data).unwrap();
        let stroke = &doc.pages[0].strokes[0];
        assert_eq!(
            stroke.points,
            [
                Point { x: 10.0, y: 20.0 },
                Point { x: 310.5, y: 17.75 },
                Point { x: 311.5, y: 19.75 },
            ]
        );
        assert_eq!(stroke.pressures, [0.25, 0.375, 0.3125]);
        assert_eq!(stroke.timestamps, [1000, 1005, 66540]);
        assert_eq!(stroke.tilts, [0.5, 0.75, 0.625]);
        assert_eq!(stroke.orientations, [-1.0, -1.125, -1.0625]);
    }
}

#[test]
fn traverses_every_layer_and_child_without_scanning_unknown_payloads() {
    let payload = stroke(1, 3, &compressed(false), 0, &[]);
    let child = object(1, &payload, &[]);
    let tree = object(4, b"container payload", std::slice::from_ref(&child));
    // An unknown object's payload deliberately contains a valid-looking stroke.
    let unknown = object(250, &payload, &[]);
    let bytes = archive(&page(&[vec![unknown, tree], vec![child]], 0, &[]));
    let parsed = sdocx::parse_bytes_detailed(&bytes).unwrap();
    assert_eq!(parsed.stored_pages[0].page.layers.layers.len(), 2);
    assert_eq!(parsed.document.pages[0].strokes.len(), 2);
    assert_eq!(parsed.document.pages[0].strokes[1].points.len(), 3);
    assert!(parsed.document.pages[0].strokes[0].tilts.is_empty());
    assert!(
        parsed
            .report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == sdocx::DiagnosticCode::UnknownObjectType)
    );
}

#[test]
fn known_unsupported_objects_report_their_location_and_keep_decoded_children() {
    let kinds = [
        0, 4, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 100,
    ];
    let payload = stroke(1, 3, &compressed(false), 0, &[]);
    let child = object(1, &payload, &[]);
    let parents = kinds
        .iter()
        .map(|kind| object(*kind, &payload, std::slice::from_ref(&child)))
        .collect::<Vec<_>>();
    let page_bytes = page(
        &[
            parents,
            vec![object(250, &payload, std::slice::from_ref(&child))],
        ],
        0,
        &[],
    );
    let parsed = sdocx::parse_bytes_detailed(&archive(&page_bytes)).unwrap();
    assert_eq!(parsed.document.pages[0].strokes.len(), kinds.len() + 1);
    assert!(parsed.document.pages[0].elements.is_empty());
    let warnings = parsed
        .report
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == sdocx::DiagnosticCode::UnsupportedObjectType)
        .collect::<Vec<_>>();
    assert_eq!(warnings.len(), kinds.len());
    let stored = &parsed.stored_pages[0].page.layers.layers[0].objects;
    for ((kind, object), warning) in kinds.iter().zip(stored).zip(warnings) {
        assert_eq!(object.object_type.raw(), u32::from(*kind));
        assert_eq!(object.payload(&page_bytes).unwrap(), payload);
        assert_eq!(warning.archive_entry.as_deref(), Some("page.page"));
        assert!(warning.message.contains(&format!("type {kind}")));
        assert!(
            warning
                .message
                .contains(&format!("0x{:x}", object.payload_offset))
        );
    }
    assert_eq!(
        parsed
            .report
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == sdocx::DiagnosticCode::UnknownObjectType)
            .count(),
        1
    );
}

#[test]
fn uncompressed_strokes_store_complete_arrays_in_channel_order() {
    for stylus in [false, true] {
        let mut channels = Vec::new();
        for value in [1.25_f64, 5.5, 12.75, 9.0] {
            channels.extend_from_slice(&value.to_le_bytes());
        }
        for value in [0.25_f32, 0.875] {
            channels.extend_from_slice(&value.to_le_bytes());
        }
        for value in [-100_i32, 60000] {
            channels.extend_from_slice(&value.to_le_bytes());
        }
        if stylus {
            for value in [0.5_f32, 0.75, -1.0, -0.5] {
                channels.extend_from_slice(&value.to_le_bytes());
            }
        }
        let bytes = single(&stroke(if stylus { 4 } else { 0 }, 2, &channels, 0, &[]));
        let doc = sdocx::parse_bytes(&bytes).unwrap();
        let stroke = &doc.pages[0].strokes[0];
        assert_eq!(
            stroke.points,
            [Point { x: 1.25, y: 5.5 }, Point { x: 12.75, y: 9.0 }]
        );
        assert_eq!(stroke.pressures, [0.25, 0.875]);
        assert_eq!(stroke.timestamps, [-100, 60000]);
        assert_eq!(stroke.tilts, if stylus { vec![0.5, 0.75] } else { vec![] });
        assert_eq!(
            stroke.orientations,
            if stylus { vec![-1.0, -0.5] } else { vec![] }
        );
    }
}

#[test]
fn style_masks_control_color_and_width_without_marker_searches() {
    let mut style = 7_u32.to_le_bytes().to_vec(); // pen string-table ID
    style.extend_from_slice(&0xff47a114_u32.to_le_bytes());
    style.extend_from_slice(&5.5_f32.to_le_bytes());
    // Unknown later field contains a false legacy color marker and size.
    style.extend_from_slice(&[3, 0, 1, 0, 0, 0, 255, 0, 0, 255]);
    style.extend_from_slice(&99.0_f32.to_le_bytes());
    let bytes = single(&stroke(1, 3, &compressed(false), (1 << 31) | 14, &style));
    let doc = sdocx::parse_bytes(&bytes).unwrap();
    let stroke = &doc.pages[0].strokes[0];
    assert_eq!(
        stroke.color,
        Some(Color {
            r: 0x47,
            g: 0xa1,
            b: 0x14
        })
    );
    assert_eq!(stroke.pen_width, 5.5);
    assert!(stroke.tilts.is_empty());
    assert!(stroke.orientations.is_empty());
}

#[test]
fn rejects_incomplete_or_non_finite_channels_instead_of_returning_partial_pages() {
    let good = compressed(true);
    for end in 0..good.len() {
        let bytes = single(&stroke(5, 3, &good[..end], 0, &[]));
        assert!(
            matches!(sdocx::parse_bytes(&bytes), Err(Error::Format(_))),
            "accepted {end} channel bytes"
        );
    }
    for offset in [0, 24, 40] {
        // first coordinate, pressure and tilt
        let mut channels = good.clone();
        if offset == 0 {
            channels[..8].copy_from_slice(&f64::INFINITY.to_le_bytes());
        } else {
            channels[offset..offset + 4].copy_from_slice(&f32::NAN.to_le_bytes());
        }
        assert!(sdocx::parse_bytes(&single(&stroke(5, 3, &channels, 0, &[]))).is_err());
    }
}

#[test]
fn applies_limits_to_declared_points_and_strokes_across_layers_and_children() {
    let bytes = single(&stroke(1, u16::MAX, &[], 0, &[]));
    let options = ParseOptions {
        limits: ParseLimits {
            max_points_per_stroke: 2,
            ..ParseLimits::default()
        },
        ..Default::default()
    };
    assert!(matches!(
        sdocx::parse_bytes_with_options(&bytes, &options),
        Err(Error::LimitExceeded {
            resource: "points per stroke",
            limit: 2,
            actual: 65535,
        })
    ));
    let child = object(1, &stroke(1, 3, &compressed(false), 0, &[]), &[]);
    let bytes = archive(&page(
        &[
            vec![object(4, &[], std::slice::from_ref(&child))],
            vec![child],
        ],
        0,
        &[],
    ));
    let options = ParseOptions {
        limits: ParseLimits {
            max_strokes_per_page: 1,
            ..ParseLimits::default()
        },
        ..Default::default()
    };
    assert!(matches!(
        sdocx::parse_bytes_with_options(&bytes, &options),
        Err(Error::LimitExceeded {
            resource: "strokes per page",
            limit: 1,
            actual: 2,
        })
    ));
}

#[test]
fn zero_point_strokes_have_no_channel_seed_values() {
    for properties in [0, 1, 4, 5] {
        let doc = sdocx::parse_bytes(&single(&stroke(properties, 0, &[], 0, &[]))).unwrap();
        assert!(doc.pages[0].strokes[0].points.is_empty());
        assert!(doc.pages[0].strokes[0].pressures.is_empty());
    }
}

#[test]
fn stored_objects_expose_shared_identity_and_placement_metadata() {
    let payload = stroke(1, 3, &compressed(false), 0, &[]);
    let raw = page(&[vec![object(1, &payload, &[])]], 0, &[]);
    let parsed = sdocx::parse_bytes_detailed(&archive(&raw)).unwrap();
    let stored = &parsed.stored_pages[0].page.layers.layers[0].objects[0];
    let metadata = stored.base_metadata(&raw).unwrap();
    assert_eq!(metadata.uuid, "00000000-0000-0000-0000-000000000001");
    assert_eq!(metadata.modified_time_raw, 1234);
    assert_eq!(metadata.format_version, 5500);
    assert_eq!(metadata.bbox, parsed.document.pages[0].strokes[0].bbox);
    assert_eq!(metadata.rotation_degrees, None);
    assert!(stored.base_metadata(&raw[..stored.payload_offset]).is_err());
}

#[test]
fn page_properties_follow_masks_with_variable_headers() {
    let mut properties = Vec::new();
    for value in [10.0_f64, 20.0, 300.0, 400.0] {
        properties.extend_from_slice(&value.to_le_bytes());
    }
    properties.extend_from_slice(&0xfff5dddd_u32.to_le_bytes());
    properties.extend_from_slice(&10_u32.to_le_bytes());
    let bytes = archive(&page(&[vec![]], 1 | 32 | 512, &properties));
    let doc = sdocx::parse_bytes(&bytes).unwrap();
    let page = &doc.pages[0];
    assert_eq!(page.width, 1080);
    assert_eq!(page.content_bbox.x_max, 300.0);
    assert_eq!(
        page.background_color,
        Some(Color {
            r: 0xf5,
            g: 0xdd,
            b: 0xdd
        })
    );
    assert_eq!(page.template.unwrap().id, 10);
}

#[test]
fn malformed_frames_cannot_consume_a_sibling_or_return_a_partial_page() {
    let valid = stroke(5, 3, &compressed(true), 0, &[]);
    let base_size = base().len();
    let mut mutations = Vec::new();
    // Inflated frame size would reach the outer hash or the next object.
    let mut bytes = valid.clone();
    let size = u32::from_le_bytes(bytes[base_size..base_size + 4].try_into().unwrap());
    bytes[base_size..base_size + 4].copy_from_slice(&(size + 32).to_le_bytes());
    mutations.push(bytes);
    // Fixed base fields cannot borrow from its flexible block or stroke frame.
    let mut bytes = valid.clone();
    bytes[6..10].copy_from_slice(&18_u32.to_le_bytes());
    mutations.push(bytes);
    // A supported outer type must contain the corresponding typed frame.
    let mut bytes = valid.clone();
    bytes[base_size + 4..base_size + 6].copy_from_slice(&3_i16.to_le_bytes());
    mutations.push(bytes);
    // A declared style must have enough bytes inside the flexible block.
    mutations.push(stroke(5, 3, &compressed(true), 8, &[]));
    for invalid in mutations {
        let bytes = archive(&page(
            &[vec![
                object(1, &valid, &[]),
                object(1, &invalid, &[]),
                object(1, &valid, &[]),
            ]],
            0,
            &[],
        ));
        let error = sdocx::parse_bytes(&bytes).unwrap_err();
        assert!(matches!(error, Error::Format(_)));
        assert!(error.to_string().contains("page page: stroke at 0x"));
    }
}

#[test]
fn unknown_late_style_fields_do_not_invent_color_width_or_stylus_channels() {
    let style = [3, 0, 1, 0, 0, 0, 0xff, 0, 0, 0xff, 0, 0, 0x40, 0x40];
    let bytes = single(&stroke(1, 3, &compressed(false), 1 << 31, &style));
    let doc = sdocx::parse_bytes(&bytes).unwrap();
    let stroke = &doc.pages[0].strokes[0];
    assert_eq!(stroke.color, None);
    assert_eq!(stroke.pen_width, 0.8);
    assert!(stroke.tilts.is_empty());
}

#[test]
fn pdf_template_index_follows_the_declared_record_instead_of_page_size() {
    let mut properties = Vec::new();
    properties.extend_from_slice(&1_u16.to_le_bytes());
    properties.extend_from_slice(&7_u32.to_le_bytes()); // media ID
    properties.extend_from_slice(&3_u32.to_le_bytes()); // PDF page index
    for value in [0_u32, 0, 1080, 1527] {
        properties.extend_from_slice(&value.to_le_bytes());
    }
    let doc = sdocx::parse_bytes(&archive(&page(&[vec![]], 1 << 8, &properties))).unwrap();
    assert_eq!(
        doc.pages[0].template.unwrap().source,
        sdocx::PageTemplateSource::CustomPdf { page_index: 3 }
    );
}

#[test]
fn invalid_page_offsets_and_truncated_masks_are_errors() {
    let valid = page(&[vec![]], 0, &[]);
    for (offset, value) in [(0, 0), (0, u32::MAX), (4, 4), (4, u32::MAX)] {
        let mut bytes = valid.clone();
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        assert!(sdocx::parse_bytes(&archive(&bytes)).is_err());
    }
    for end in 0..16 {
        assert!(sdocx::parse_stored_page_bytes(&valid[..end]).is_err());
    }
}
