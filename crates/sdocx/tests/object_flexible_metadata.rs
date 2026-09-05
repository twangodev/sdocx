#[allow(dead_code)]
mod support;

use sdocx::{
    Error, ObjectBundleValue, ObjectFlexibleMetadata, ObjectLayoutType, ObjectMetadata,
    ParseLimits, Result, parse_stored_page_bytes,
};

fn utf8(value: &str) -> Vec<u8> {
    [
        (value.len() as u16).to_le_bytes().to_vec(),
        value.as_bytes().to_vec(),
    ]
    .concat()
}

fn utf16(value: &str) -> Vec<u8> {
    let units: Vec<_> = value.encode_utf16().collect();
    [
        (units.len() as u16).to_le_bytes().to_vec(),
        units.into_iter().flat_map(u16::to_le_bytes).collect(),
    ]
    .concat()
}

fn base(mask: &[u8], version: u32, flexible: &[u8]) -> ObjectMetadata {
    let mut fixed = version.to_le_bytes().to_vec();
    fixed.extend(utf8("object"));
    fixed.extend((-1_i64).to_le_bytes());
    fixed.extend(
        [0.0_f64, 0.0, 100.0, 200.0]
            .into_iter()
            .flat_map(f64::to_le_bytes),
    );
    fixed.extend([0; 5]);
    let offset = 13 + mask.len() + fixed.len();
    let mut payload = ((offset + flexible.len()) as u32).to_le_bytes().to_vec();
    payload.extend(0_i16.to_le_bytes());
    payload.extend((offset as u32).to_le_bytes());
    payload.extend([1, 8, mask.len() as u8]);
    payload.extend(mask);
    payload.extend(fixed);
    payload.extend(flexible);
    payload.extend([0; 256]);
    let bytes = support::page(&[vec![support::object(250, &payload, &[])]], 0, &[]);
    let page = parse_stored_page_bytes(&bytes).unwrap();
    page.layers.layers[0].objects[0]
        .base_metadata(&bytes)
        .unwrap()
}

fn metadata(mask: u32, flexible: &[u8]) -> Result<ObjectFlexibleMetadata> {
    base(&mask.to_le_bytes(), 5500, flexible).flexible_metadata()
}

fn integer_bundle(key: &str, value: i32) -> Vec<u8> {
    [vec![2, 1, 0], utf8(key), value.to_le_bytes().to_vec()].concat()
}

fn complete_bundle() -> Vec<u8> {
    let mut bytes = vec![15, 2, 0];
    bytes.extend(utf8("same"));
    bytes.extend((-2_i16).to_le_bytes());
    bytes.extend(utf8("same"));
    bytes.extend(utf16("值😀"));
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend(utf8("same"));
    bytes.extend((-9_i32).to_le_bytes());
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend(utf8("array"));
    bytes.extend(2_u16.to_le_bytes());
    bytes.extend(utf16(""));
    bytes.extend(utf16("𝑥"));
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend(utf8("SPEN_SDK_KEY_SYSTEM_RESERVED_EXTRA_DATA"));
    bytes.extend(3_u32.to_le_bytes());
    bytes.extend([1, 0, 2]);
    bytes
}

fn fields() -> Vec<(usize, Vec<u8>)> {
    vec![
        (1, [2_u16.to_le_bytes().to_vec(), vec![0x81; 32]].concat()),
        (2, utf16("SOR 😀")),
        (3, complete_bundle()),
        (4, utf16("com.example.sor")),
        (5, integer_bundle("extra", 123)),
        (6, (-1_i32).to_le_bytes().to_vec()),
        (
            7,
            [-2.0_f32, 5.0]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect(),
        ),
        (
            8,
            [300.0_f32, 400.0]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect(),
        ),
        (13, (-12345_i64).to_le_bytes().to_vec()),
        (
            14,
            [1080_i32, 1920]
                .into_iter()
                .flat_map(i32::to_le_bytes)
                .collect(),
        ),
        (15, vec![2]),
        (16, (0..20).collect()),
        (17, (-7_i32).to_le_bytes().to_vec()),
        (
            18,
            [12.25_f64, 34.5]
                .into_iter()
                .flat_map(f64::to_le_bytes)
                .collect(),
        ),
        (19, utf16("group-🖊")),
        (20, (-1_i32).to_le_bytes().to_vec()),
        (21, 42_i32.to_le_bytes().to_vec()),
    ]
}

#[test]
fn decodes_all_mapped_fields_and_keeps_bundle_records_in_wire_order() {
    let fields = fields();
    let mask = fields.iter().fold(1_u32, |mask, (bit, _)| mask | 1 << bit);
    let payload = [
        15.5_f32.to_le_bytes().to_vec(),
        fields.into_iter().flat_map(|(_, bytes)| bytes).collect(),
        vec![0xfe, 0xfd],
    ]
    .concat();
    let base = base(&mask.to_le_bytes(), 5500, &payload);
    let value = base.flexible_metadata().unwrap();
    assert_eq!(base.rotation_degrees, Some(15.5));
    assert_eq!(base.flexible_trailing_data, payload[4..]);
    assert_eq!(
        value.partial_rectangles.as_deref(),
        Some([[0x81; 16]; 2].as_slice())
    );
    assert_eq!(value.sor_info.as_deref(), Some("SOR 😀"));
    assert_eq!(value.sor_package_link.as_deref(), Some("com.example.sor"));
    let bundle = value.sor_data.unwrap();
    assert_eq!(bundle.category_mask, 15);
    assert_eq!(bundle.data, complete_bundle());
    assert_eq!(bundle.entries.len(), 5);
    assert_eq!(bundle.entries[0].key, "same");
    assert_eq!(bundle.entries[0].value, ObjectBundleValue::String(None));
    assert_eq!(bundle.entries[1].key, "same");
    assert_eq!(
        bundle.entries[1].value,
        ObjectBundleValue::String(Some("值😀".into()))
    );
    assert_eq!(bundle.entries[2].key, "same");
    assert_eq!(bundle.entries[2].value, ObjectBundleValue::Integer(-9));
    assert_eq!(
        bundle.entries[3].value,
        ObjectBundleValue::StringArray(vec!["".into(), "𝑥".into()])
    );
    assert_eq!(
        bundle.entries[4].key,
        "SPEN_SDK_KEY_SYSTEM_RESERVED_EXTRA_DATA"
    );
    assert_eq!(
        bundle.entries[4].value,
        ObjectBundleValue::Bytes(vec![1, 0, 2])
    );
    let extra = value.extra_data.unwrap();
    assert_eq!(extra.data, integer_bundle("extra", 123));
    assert_eq!(extra.entries[0].value, ObjectBundleValue::Integer(123));
    assert_eq!(value.attached_file_id, Some(-1));
    assert_eq!(value.min_size.unwrap().width, -2.0);
    assert_eq!(value.min_size.unwrap().height, 5.0);
    assert_eq!(value.max_size.unwrap().width, 300.0);
    assert_eq!(value.max_size.unwrap().height, 400.0);
    assert_eq!(value.append_time_raw, Some(-12345));
    assert_eq!(value.owner_page_size.unwrap().width, 1080);
    assert_eq!(value.owner_page_size.unwrap().height, 1920);
    assert_eq!(value.layout_type, Some(ObjectLayoutType::Block));
    assert_eq!(
        value.saved_span_data.unwrap(),
        std::array::from_fn(|i| i as u8)
    );
    assert_eq!(value.captured_thumbnail_media_id, Some(-7));
    assert_eq!(value.pivot, Some(sdocx::Point { x: 12.25, y: 34.5 }));
    assert_eq!(value.group_id.as_deref(), Some("group-🖊"));
    assert_eq!(value.page_index, Some(-1));
    assert_eq!(value.render_layer_id, Some(42));
    assert_eq!(value.first_unparsed_field, None);
    assert_eq!(value.trailing_data, [0xfe, 0xfd]);
}

#[test]
fn every_optional_field_is_bounded_by_its_base_frame() {
    for (bit, data) in fields() {
        metadata(1 << bit, &data).unwrap();
        for length in 0..data.len() {
            assert!(
                metadata(1 << bit, &data[..length]).is_err(),
                "bit {bit}, length {length}"
            );
        }
    }
}

#[test]
fn absent_fields_and_present_empty_values_remain_distinct() {
    assert_eq!(metadata(0, &[]).unwrap(), ObjectFlexibleMetadata::default());
    let value = metadata(
        (1 << 1) | (1 << 2) | (1 << 3) | (1 << 4) | (1 << 5) | (1 << 19),
        &[0, 0, 0, 0, 0, 0, 0, 15, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    )
    .unwrap();
    assert_eq!(value.partial_rectangles, Some(vec![]));
    assert_eq!(value.sor_info.as_deref(), Some(""));
    assert_eq!(value.sor_package_link.as_deref(), Some(""));
    assert_eq!(value.group_id.as_deref(), Some(""));
    assert_eq!(value.sor_data.unwrap().data, [0]);
    let bundle = value.extra_data.unwrap();
    assert_eq!(bundle.category_mask, 15);
    assert!(bundle.entries.is_empty());
    assert!(value.trailing_data.is_empty());
}

#[test]
fn unknown_fields_preserve_the_tail_without_using_the_static_extraction_layout() {
    for bit in [9, 10, 11, 12, 22, 31, 32, 39] {
        let mut mask = [4, 0, 0, 0, 0];
        mask[bit / 8] |= 1 << (bit % 8);
        mask[2] |= 1 << 5;
        let mut data = utf16("known");
        if bit > 21 {
            data.extend(42_i32.to_le_bytes());
        }
        let tail_start = data.len();
        data.extend([0xaa, 0xbb]);
        if bit < 21 {
            data.extend(42_i32.to_le_bytes());
        }
        let value = base(&mask, 5500, &data).flexible_metadata().unwrap();
        assert_eq!(value.sor_info.as_deref(), Some("known"));
        assert_eq!(value.render_layer_id, (bit > 21).then_some(42));
        assert_eq!(value.first_unparsed_field, Some(bit));
        assert_eq!(value.trailing_data, data[tail_start..]);
    }
    let base = base(&[0, 0, 0x20, 0, 2], 5500, &[42, 0, 0, 0, 0x81, 0x82]);
    let value = base.flexible_metadata().unwrap();
    assert_eq!(value.render_layer_id, Some(42));
    assert_eq!(value.first_unparsed_field, Some(33));
    assert_eq!(value.trailing_data, [0x81, 0x82]);
}

#[test]
fn unknown_bundle_categories_stop_before_consuming_the_bundle_or_later_fields() {
    for bit in [3, 5] {
        let payload = [
            utf16("before"),
            vec![0x1f, 0xff, 0xff, 0xaa],
            utf16("after"),
        ]
        .concat();
        let value = metadata((1 << 2) | (1 << bit) | (1 << 19), &payload).unwrap();
        assert_eq!(value.sor_info.as_deref(), Some("before"));
        assert_eq!(value.sor_data, None);
        assert_eq!(value.extra_data, None);
        assert_eq!(value.group_id, None);
        assert_eq!(value.first_unparsed_field, Some(bit));
        assert_eq!(value.trailing_data, payload[utf16("before").len()..]);
    }
}

#[test]
fn version_gated_dimensions_and_attachment_fields_do_not_shift_append_time() {
    for version in [0, 5, 6, 8, 9, 12, 13, 5500, u32::MAX] {
        let mut data = Vec::new();
        if version >= 6 {
            data.extend((-1_i32).to_le_bytes());
        }
        if version >= 9 {
            data.extend([1.5_f32, 2.5].into_iter().flat_map(f32::to_le_bytes));
        }
        if version >= 13 {
            data.extend([30.5_f32, 40.5].into_iter().flat_map(f32::to_le_bytes));
        }
        data.extend(123_i64.to_le_bytes());
        let mask = ((1_u32 << 6) | (1 << 7) | (1 << 8) | (1 << 13)).to_le_bytes();
        let value = base(&mask, version, &data).flexible_metadata().unwrap();
        assert_eq!(value.attached_file_id, (version >= 6).then_some(-1));
        assert_eq!(
            value.min_size.map(|size| size.width),
            (version >= 9).then_some(1.5)
        );
        assert_eq!(
            value.max_size.map(|size| size.height),
            (version >= 13).then_some(40.5)
        );
        assert_eq!(value.append_time_raw, Some(123));
        assert!(value.trailing_data.is_empty());
    }
}

#[test]
fn scalar_string_nulls_keep_their_raw_encoding_and_following_categories_aligned() {
    for length in [i16::MIN, -2, -1, 0, 1] {
        let mut bundle = vec![3, 1, 0];
        bundle.extend(utf8(""));
        bundle.extend(length.to_le_bytes());
        if length == 1 {
            bundle.extend(0x0078_u16.to_le_bytes());
        }
        bundle.extend(1_u16.to_le_bytes());
        bundle.extend(utf8("i"));
        bundle.extend(42_i32.to_le_bytes());
        let value = metadata(1 << 3, &bundle).unwrap().sor_data.unwrap();
        assert_eq!(value.data, bundle);
        let expected = match length {
            0 => Some("".into()),
            1 => Some("x".into()),
            _ => None,
        };
        assert_eq!(value.entries[0].value, ObjectBundleValue::String(expected));
        assert_eq!(value.entries[1].value, ObjectBundleValue::Integer(42));
    }
}

#[test]
fn string_arrays_and_common_strings_accept_the_full_unsigned_length_range() {
    let long = "x".repeat(usize::from(u16::MAX));
    let bundle = [vec![4, 1, 0], utf8(""), vec![1, 0], utf16(&long)].concat();
    let payload = [utf16(&long), bundle, utf16(&long)].concat();
    let value = metadata((1 << 2) | (1 << 3) | (1 << 19), &payload).unwrap();
    assert_eq!(value.sor_info.as_deref(), Some(long.as_str()));
    assert_eq!(value.group_id.as_deref(), Some(long.as_str()));
    assert_eq!(
        value.sor_data.unwrap().entries[0].value,
        ObjectBundleValue::StringArray(vec![long])
    );
}

#[test]
fn byte_array_lengths_use_four_bytes_and_do_not_consume_the_next_field() {
    let blob = vec![0x55; 70_000];
    let bundle = [
        vec![8, 1, 0],
        utf8("b"),
        (blob.len() as u32).to_le_bytes().to_vec(),
        blob.clone(),
    ]
    .concat();
    let payload = [bundle.clone(), 42_i32.to_le_bytes().to_vec()].concat();
    let value = metadata((1 << 5) | (1 << 21), &payload).unwrap();
    let extra = value.extra_data.unwrap();
    assert_eq!(extra.data, bundle);
    assert_eq!(extra.entries[0].value, ObjectBundleValue::Bytes(blob));
    assert_eq!(value.render_layer_id, Some(42));
    assert!(value.trailing_data.is_empty());
}

#[test]
fn metadata_limits_cover_both_bundles_nested_arrays_partial_rectangles_and_text() {
    let data = [
        vec![2, 0],
        vec![0; 32],
        complete_bundle(),
        integer_bundle("extra", 1),
    ]
    .concat();
    let base = base(
        &((1_u32 << 1) | (1 << 3) | (1 << 5)).to_le_bytes(),
        5500,
        &data,
    );
    let mut limits = ParseLimits {
        max_object_metadata_entries: 10,
        ..ParseLimits::default()
    };
    base.flexible_metadata_with_limits(&limits).unwrap();
    limits.max_object_metadata_entries = 9;
    assert!(matches!(
        base.flexible_metadata_with_limits(&limits),
        Err(Error::LimitExceeded {
            resource: "object metadata entries",
            actual: 10,
            ..
        })
    ));
    limits.max_object_metadata_entries = 10;
    limits.max_entry_size = (data.len() - 1) as u64;
    assert!(matches!(
        base.flexible_metadata_with_limits(&limits),
        Err(Error::LimitExceeded {
            resource: "object metadata size",
            ..
        })
    ));

    let bundle = [vec![1, 1, 0], utf8("é"), utf16("𝑥")].concat();
    let data = [utf16("A😀"), bundle, integer_bundle("z", 0), utf16("q")].concat();
    let base = self::base(
        &((1_u32 << 2) | (1 << 3) | (1 << 5) | (1 << 19)).to_le_bytes(),
        5500,
        &data,
    );
    let mut limits = ParseLimits {
        max_text_characters: 8,
        ..ParseLimits::default()
    };
    base.flexible_metadata_with_limits(&limits).unwrap();
    limits.max_text_characters = 7;
    assert!(matches!(
        base.flexible_metadata_with_limits(&limits),
        Err(Error::LimitExceeded {
            resource: "text characters",
            ..
        })
    ));
}

#[test]
fn malformed_bundle_lengths_and_encodings_fail_within_the_bounded_frame() {
    let bad = [
        vec![1, 1, 0, 1, 0, 0xff, 0, 0],
        vec![1, 1, 0, 0, 0, 1, 0, 0, 0xd8],
        vec![4, 1, 0, 0, 0, 1, 0, 1, 0, 0, 0xd8],
        vec![8, 1, 0, 0, 0, 0xff, 0xff, 0xff, 0xff],
        vec![2, 0xff, 0xff],
        vec![4, 1, 0, 0, 0, 0xff, 0xff],
    ];
    for data in bad {
        assert!(metadata(1 << 3, &data).is_err(), "{data:?}");
    }
}

#[test]
fn layout_types_and_unknown_values_preserve_the_stored_byte() {
    for (raw, expected) in [
        (0, ObjectLayoutType::Normal),
        (1, ObjectLayoutType::Flow),
        (2, ObjectLayoutType::Block),
        (3, ObjectLayoutType::Undefined),
        (4, ObjectLayoutType::Other(4)),
        (255, ObjectLayoutType::Other(255)),
    ] {
        assert_eq!(
            metadata(1 << 15, &[raw]).unwrap().layout_type,
            Some(expected)
        );
    }
}

#[test]
fn saved_span_snapshot_uses_five_floats_and_preserves_the_original_data() {
    let data: Vec<_> = [-12.5_f32, 2.25, 123.75, 400.5, -45.0]
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect();
    let value = metadata(1 << 16, &data).unwrap();
    let snapshot = value.saved_span_snapshot().unwrap();
    assert_eq!(snapshot.bbox.x_min, -12.5);
    assert_eq!(snapshot.bbox.y_min, 2.25);
    assert_eq!(snapshot.bbox.x_max, 123.75);
    assert_eq!(snapshot.bbox.y_max, 400.5);
    assert_eq!(snapshot.rotation_degrees, -45.0);
    assert_eq!(value.saved_span_data.unwrap().as_slice(), data);
    assert_eq!(metadata(0, &[]).unwrap().saved_span_snapshot(), None);
}
