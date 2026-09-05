mod support;

use sdocx::{Error, ObjectType, ParseLimits, StrokeMetadata};
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

fn base(partials: Option<u16>) -> Vec<u8> {
    let mut fixed = 5500_u32.to_le_bytes().to_vec();
    fixed.extend(6_u16.to_le_bytes());
    fixed.extend(b"stroke");
    fixed.extend((-123_i64).to_le_bytes());
    for value in [0_f64, 0.0, 100.0, 200.0] {
        fixed.extend(value.to_le_bytes());
    }
    fixed.extend(7_i32.to_le_bytes());
    fixed.push(0);
    let mut flexible = 90_f32.to_le_bytes().to_vec();
    if let Some(count) = partials {
        flexible.extend(count.to_le_bytes());
        flexible.extend(vec![0x55; usize::from(count) * 16]);
    }
    frame(
        0,
        &[8],
        &[if partials.is_some() { 3 } else { 1 }],
        &fixed,
        &flexible,
    )
}

fn payload(properties: &[u8], fields: &[u8], flexible: &[u8], partials: Option<u16>) -> Vec<u8> {
    [
        base(partials),
        frame(1, properties, fields, &[0, 0, 0xff, 0xff], flexible),
    ]
    .concat()
}

fn inspect(payload: &[u8], limits: &ParseLimits) -> sdocx::Result<StrokeMetadata> {
    let raw = page(&[vec![object(1, payload, &[])]], 0, &[]);
    let parsed = sdocx::parse_stored_page_bytes(&raw)?;
    parsed.layers.layers[0].objects[0].stroke_metadata_with_limits(&raw, limits)
}

fn fields() -> Vec<(usize, Vec<u8>)> {
    vec![
        (0, (-101_i32).to_le_bytes().to_vec()),
        (1, 102_i32.to_le_bytes().to_vec()),
        (2, 0x12345678_u32.to_le_bytes().to_vec()),
        (3, 4.25_f32.to_le_bytes().to_vec()),
        (4, vec![255]),
        (5, vec![1, 2, 3, 4, 5, 6, 7, 8]),
        (7, (-1_i32).to_le_bytes().to_vec()),
        (8, 8.5_f32.to_le_bytes().to_vec()),
        (9, (-9_i32).to_le_bytes().to_vec()),
        (10, 1010_i32.to_le_bytes().to_vec()),
        (11, 11_i32.to_le_bytes().to_vec()),
        (12, 1200_i32.to_le_bytes().to_vec()),
        (13, 0.125_f32.to_le_bytes().to_vec()),
        (14, 14_u16.to_le_bytes().to_vec()),
        (15, (-15.5_f32).to_le_bytes().to_vec()),
        (16, 65535_u16.to_le_bytes().to_vec()),
        (17, 0.75_f32.to_le_bytes().to_vec()),
        (18, 1.25_f32.to_le_bytes().to_vec()),
        (19, (-19_i32).to_le_bytes().to_vec()),
        (20, 2.5_f32.to_le_bytes().to_vec()),
        (21, i32::MIN.to_le_bytes().to_vec()),
        (22, i32::MAX.to_le_bytes().to_vec()),
        (23, (-0.25_f32).to_le_bytes().to_vec()),
        (
            24,
            [
                2_u16.to_le_bytes().as_slice(),
                &0x00ff0000_u32.to_le_bytes(),
                &0x80ffffff_u32.to_le_bytes(),
            ]
            .concat(),
        ),
        (25, 255_u16.to_le_bytes().to_vec()),
    ]
}

#[test]
fn complete_native_style_keeps_signed_ids_argb_and_following_records_aligned() {
    let fields = fields();
    let mask = fields
        .iter()
        .fold(0_u32, |mask, (bit, _)| mask | (1 << bit));
    let flexible: Vec<_> = fields.into_iter().flat_map(|(_, bytes)| bytes).collect();
    let extension = frame(99, &[], &[], &[], &[0xab, 0xcd]);
    let bytes = [
        payload(&[0x65], &mask.to_le_bytes(), &flexible, Some(2)),
        extension.clone(),
    ]
    .concat();
    let metadata = inspect(&bytes, &ParseLimits::default()).unwrap();
    assert_eq!(metadata.point_count, 0);
    assert_eq!(metadata.tool_type_raw, 65535);
    assert_eq!(metadata.base.uuid, "stroke");
    assert_eq!(metadata.base.rotation_degrees, Some(90.0));
    assert_eq!(metadata.property_mask, [0x65]);
    assert_eq!(metadata.field_mask, mask.to_le_bytes());
    assert_eq!(metadata.trailing_data, extension);
    let style = metadata.style;
    assert_eq!(style.legacy_advanced_pen_setting_id, Some(-101));
    assert_eq!(style.pen_name_id, Some(102));
    assert_eq!(style.color_argb, Some(0x12345678));
    assert_eq!(style.pen_size, Some(4.25));
    assert_eq!(style.field_4_raw, Some(255));
    assert_eq!(
        style.legacy_partial_rectangle_data,
        Some(vec![[1, 2, 3, 4], [5, 6, 7, 8]])
    );
    assert_eq!(style.advanced_pen_setting_id, Some(-1));
    assert_eq!(style.fixed_width, Some(8.5));
    assert_eq!(style.size_level, Some(-9));
    assert_eq!(style.particle_density, Some(1010));
    assert_eq!(style.rendering_level, Some(11));
    assert_eq!(style.original_width, Some(1200));
    assert_eq!(style.initial_tolerance, Some(0.125));
    assert_eq!(style.line_type_raw, Some(14));
    assert_eq!(style.dash_offset, Some(-15.5));
    assert_eq!(style.stroke_type_raw, Some(65535));
    assert_eq!(style.pen_repeat_distance, Some(0.75));
    assert_eq!(style.particle_size, Some(1.25));
    assert_eq!(style.pattern_index, Some(-19));
    assert_eq!(style.pattern_scale, Some(2.5));
    assert_eq!(style.particle_level, Some(i32::MIN));
    assert_eq!(style.rainbow_distance, Some(i32::MAX));
    assert_eq!(style.rainbow_offset, Some(-0.25));
    assert_eq!(
        style.gradient_colors_argb,
        Some(vec![0x00ff0000, 0x80ffffff])
    );
    assert_eq!(style.color_type_raw, Some(255));
    assert_eq!(style.first_unparsed_field, None);
    assert!(style.trailing_data.is_empty());
}

#[test]
fn native_property_polarity_and_future_mask_bytes_are_preserved() {
    for (mask, expected) in [
        (
            vec![],
            [
                false, false, false, false, false, false, false, false, true, true, false, false,
                false, false,
            ],
        ),
        (
            vec![0x25],
            [
                true, false, true, false, false, true, false, false, true, true, false, false,
                false, false,
            ],
        ),
        (
            vec![0x65, 4],
            [
                true, false, true, false, false, true, true, false, true, false, false, false,
                false, false,
            ],
        ),
        (
            vec![0xff, 0xff, 0, 0, 0x80],
            [
                true, true, true, true, true, true, true, true, false, false, true, true, true,
                true,
            ],
        ),
    ] {
        let metadata = inspect(&payload(&mask, &[], &[], None), &ParseLimits::default()).unwrap();
        assert_eq!(metadata.property_mask, mask);
        let p = metadata.properties;
        assert_eq!(
            [
                p.compressed,
                p.replay_only,
                p.stylus_channels,
                p.eraser,
                p.fixed_width,
                p.millisecond_timestamps,
                p.top_layer_pen,
                p.alpha_lock,
                p.binary_added,
                p.generated,
                p.fixed_opacity,
                p.rainbow_effect,
                p.straighten,
                p.reveal_mode
            ],
            expected
        );
    }
}

#[test]
fn every_style_field_is_bounded_before_the_following_frame_and_object_hash() {
    let next = frame(100, &[], &[], &[0; 100], &[]);
    for (bit, field) in fields() {
        for end in 0..field.len() {
            let bytes = [
                payload(&[], &(1_u32 << bit).to_le_bytes(), &field[..end], Some(2)),
                next.clone(),
            ]
            .concat();
            assert!(
                inspect(&bytes, &ParseLimits::default()).is_err(),
                "bit {bit}, prefix {end}"
            );
        }
    }
}

#[test]
fn unknown_fields_stop_without_shifting_later_known_styles() {
    for bit in [6, 26, 32, 39] {
        let mask = ((1_u64 << bit) | (1 << 2) | (1 << 25)).to_le_bytes();
        let mut flexible = 0_u32.to_le_bytes().to_vec();
        if bit > 25 {
            flexible.extend(23_u16.to_le_bytes());
        }
        let remainder = [0xaa, 0xbb, 0xcc, 0xdd];
        flexible.extend(remainder);
        let metadata = inspect(
            &payload(&[], &mask, &flexible, None),
            &ParseLimits::default(),
        )
        .unwrap();
        assert_eq!(metadata.style.color_argb, Some(0));
        assert_eq!(metadata.style.first_unparsed_field, Some(bit));
        assert_eq!(metadata.style.trailing_data, remainder);
        assert_eq!(
            metadata.style.color_type_raw,
            if bit > 25 { Some(23) } else { None }
        );
        assert_eq!(metadata.field_mask, mask);
    }
}

#[test]
fn absent_and_empty_lists_remain_distinct_and_partial_count_comes_from_base() {
    let absent = inspect(&payload(&[], &[], &[], None), &ParseLimits::default()).unwrap();
    assert_eq!(absent.style.gradient_colors_argb, None);
    assert_eq!(absent.style.legacy_partial_rectangle_data, None);
    for partials in [None, Some(0)] {
        let mask = (1_u32 << 5) | (1 << 7) | (1 << 24);
        let data = [77_i32.to_le_bytes().as_slice(), &[0, 0]].concat();
        let value = inspect(
            &payload(&[], &mask.to_le_bytes(), &data, partials),
            &ParseLimits::default(),
        )
        .unwrap();
        assert_eq!(value.style.legacy_partial_rectangle_data, Some(vec![]));
        assert_eq!(value.style.advanced_pen_setting_id, Some(77));
        assert_eq!(value.style.gradient_colors_argb, Some(vec![]));
    }
    let mut malformed = base(Some(2));
    malformed.truncate(malformed.len() - 1);
    let size = malformed.len() as u32;
    malformed[..4].copy_from_slice(&size.to_le_bytes());
    malformed.extend(frame(1, &[], &[32], &[0, 0, 1, 0], &[0; 8]));
    assert!(inspect(&malformed, &ParseLimits::default()).is_err());
}

#[test]
fn metadata_limits_apply_before_list_allocation_or_channel_access() {
    let mask = (1_u32 << 5) | (1 << 24);
    let data = [vec![0; 8], vec![2, 0], vec![0; 8]].concat();
    let bytes = payload(&[], &mask.to_le_bytes(), &data, Some(2));
    let mut limits = ParseLimits {
        max_object_metadata_entries: 4,
        ..ParseLimits::default()
    };
    inspect(&bytes, &limits).unwrap();
    limits.max_object_metadata_entries = 3;
    assert!(matches!(
        inspect(&bytes, &limits),
        Err(Error::LimitExceeded {
            resource: "stroke metadata entries",
            actual: 4,
            ..
        })
    ));
    limits.max_entry_size = bytes.len() as u64 - 1;
    assert!(matches!(
        inspect(&bytes, &limits),
        Err(Error::LimitExceeded {
            resource: "stroke payload size",
            ..
        })
    ));

    let bytes = [base(None), frame(1, &[5], &[], &[0xff, 0xff, 1, 0], &[])].concat();
    let limits = ParseLimits {
        max_points_per_stroke: 2,
        ..ParseLimits::default()
    };
    assert!(matches!(
        inspect(&bytes, &limits),
        Err(Error::LimitExceeded {
            resource: "points per stroke",
            actual: 65535,
            ..
        })
    ));
    let bytes = payload(&[], &(1_u32 << 24).to_le_bytes(), &[0xff, 0xff], None);
    assert!(matches!(
        inspect(&bytes, &ParseLimits::default()),
        Err(Error::LimitExceeded {
            resource: "stroke metadata entries",
            actual: 65535,
            ..
        })
    ));
}

#[test]
fn nonfinite_style_values_are_rejected_by_explicit_inspection() {
    for bit in [3, 8, 13, 15, 17, 18, 20, 23] {
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let bytes = payload(
                &[],
                &(1_u32 << bit).to_le_bytes(),
                &value.to_le_bytes(),
                None,
            );
            assert!(
                inspect(&bytes, &ParseLimits::default()).is_err(),
                "bit {bit}"
            );
        }
    }
}

#[test]
fn metadata_rejects_wrong_types_missing_frames_and_out_of_page_payloads() {
    let bytes = payload(&[], &[], &[], None);
    let raw = page(&[vec![object(1, &bytes, &[])]], 0, &[]);
    let stored = sdocx::parse_stored_page_bytes(&raw).unwrap();
    let mut record = stored.layers.layers[0].objects[0].clone();
    record.stroke_metadata(&raw).unwrap();
    record.object_type = ObjectType::Formula;
    assert!(record.stroke_metadata(&raw).is_err());
    record.object_type = ObjectType::Stroke;
    assert!(
        record
            .stroke_metadata(&raw[..record.payload_offset])
            .is_err()
    );
    record.payload_offset = usize::MAX;
    assert!(record.stroke_metadata(&raw).is_err());
    assert!(inspect(&base(None), &ParseLimits::default()).is_err());
    assert!(
        inspect(
            &[base(None), frame(2, &[], &[], &[0; 4], &[])].concat(),
            &ParseLimits::default()
        )
        .is_err()
    );
}

#[test]
fn metadata_and_visible_strokes_share_channel_boundaries_and_style_prefix() {
    for (properties, channels) in [
        (0_u8, vec![0; 2 * 24]),
        (4, vec![0; 2 * 32]),
        (1, vec![0; 16 + 4 + 2 * 6]),
        (5, vec![0; 16 + 4 + 4 * 6]),
    ] {
        let mut fixed = 2_u16.to_le_bytes().to_vec();
        fixed.extend(&channels);
        fixed.extend(3_u16.to_le_bytes());
        let style = [0x12456789_u32.to_le_bytes(), 2.5_f32.to_le_bytes()].concat();
        let bytes = [base(None), frame(1, &[properties], &[12], &fixed, &style)].concat();
        let metadata = inspect(&bytes, &ParseLimits::default()).unwrap();
        let raw = page(&[vec![object(1, &bytes, &[])]], 0, &[]);
        let parsed = sdocx::parse_bytes(&archive(&raw)).unwrap();
        let stroke = &parsed.pages[0].strokes[0];
        assert_eq!(usize::from(metadata.point_count), stroke.points.len());
        assert_eq!(metadata.tool_type_raw, 3);
        assert_eq!(stroke.pen_width, metadata.style.pen_size.unwrap());
        assert_eq!(metadata.style.color_argb, Some(0x12456789));
        assert_eq!(
            stroke.color,
            Some(sdocx::Color {
                r: 0x45,
                g: 0x67,
                b: 0x89
            })
        );
        for missing in 1..=3 {
            let malformed = [
                base(None),
                frame(
                    1,
                    &[properties],
                    &[12],
                    &fixed[..fixed.len() - missing],
                    &style,
                ),
            ]
            .concat();
            assert!(inspect(&malformed, &ParseLimits::default()).is_err());
        }
    }
}
