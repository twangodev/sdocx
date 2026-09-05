#[allow(dead_code)]
mod support;

use sdocx::{Error, ParseLimits, parse_stored_page_bytes};

fn string(value: &str) -> Vec<u8> {
    let units: Vec<_> = value.encode_utf16().collect();
    let mut bytes = (units.len() as u16).to_le_bytes().to_vec();
    bytes.extend(units.iter().flat_map(|unit| unit.to_le_bytes()));
    bytes
}

fn header(
    offset: usize,
    number: u32,
    properties: &[u8],
    fields: &[u8],
    fixed_extra: &[u8],
    flexible: &[u8],
) -> Vec<u8> {
    let mut bytes = vec![0; 8];
    bytes.push(properties.len() as u8);
    bytes.extend(properties);
    bytes.push(fields.len() as u8);
    bytes.extend(fields);
    bytes.extend(number.to_le_bytes());
    bytes.extend(fixed_extra);
    let flexible_offset = (offset + bytes.len()) as u32;
    bytes[4..8].copy_from_slice(&flexible_offset.to_le_bytes());
    bytes.extend(flexible);
    let size = bytes.len() as u32;
    bytes[..4].copy_from_slice(&size.to_le_bytes());
    bytes
}

fn page(count: usize, mut make_header: impl FnMut(usize, usize) -> Vec<u8>) -> Vec<u8> {
    let mut bytes = support::page(&[vec![]], 0, &[]);
    let layer_offset = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
    bytes.truncate(layer_offset);
    bytes.extend((count as u16).to_le_bytes());
    bytes.extend(0_u16.to_le_bytes());
    for index in 0..count {
        bytes.extend(make_header(bytes.len(), index));
        bytes.extend(0_u32.to_le_bytes());
        bytes.extend([0xbb; 32]);
    }
    bytes.extend([0xcc; 32]);
    bytes.extend(b"Page for SAMSUNG S-Pen SDK");
    bytes
}

fn complete_fields() -> Vec<u8> {
    let mut fields = vec![129];
    fields.extend(0x81234567_u32.to_le_bytes());
    fields.extend(string("Ink 🖊"));
    fields.extend(string("layer-42"));
    fields.extend((-123456789_i64).to_le_bytes());
    fields.extend(812_u32.to_le_bytes());
    fields.extend(20_u32.to_le_bytes());
    fields.extend([0x61; 20]);
    fields
}

#[test]
fn decodes_native_layer_identity_flags_and_style_in_mask_order() {
    let bytes = page(1, |offset, _| {
        header(offset, 42, &[0x1f], &[0x7f], &[], &complete_fields())
    });
    let parsed = parse_stored_page_bytes(&bytes).unwrap();
    let layer = &parsed.layers.layers[0];
    let metadata = layer.metadata(&bytes).unwrap();
    assert_eq!(metadata.number, 42);
    assert!(!metadata.visible);
    assert!(metadata.event_forwardable);
    assert!(metadata.locked);
    assert!(metadata.alpha_locked);
    assert!(metadata.shadow_visible);
    assert_eq!(metadata.transparency, Some(129));
    assert_eq!(metadata.background_color, Some(0x81234567));
    assert_eq!(metadata.name.as_deref(), Some("Ink 🖊"));
    assert_eq!(metadata.uuid.as_deref(), Some("layer-42"));
    assert_eq!(metadata.modified_time, Some(-123456789));
    assert_eq!(metadata.thumbnail_media_id, Some(812));
    assert_eq!(
        metadata.shadow_effect.as_deref(),
        Some([0x61; 20].as_slice())
    );
    assert!(metadata.fixed_trailing_data.is_empty());
    assert!(metadata.flexible_trailing_data.is_empty());
}

#[test]
fn absolute_offsets_and_wide_masks_preserve_multiple_layer_boundaries() {
    let bytes = page(2, |offset, index| {
        let flexible = [
            string(&format!("layer-{index}")),
            (index as i64).to_le_bytes().to_vec(),
            vec![0x91, 0x92],
        ]
        .concat();
        header(
            offset,
            42 + index as u32,
            &[2, 0, 0, 0, 1],
            &[0x18, 0, 0, 0, 2],
            &[0x81; 7],
            &flexible,
        )
    });
    let parsed = parse_stored_page_bytes(&bytes).unwrap();
    for (index, layer) in parsed.layers.layers.iter().enumerate() {
        let metadata = layer.metadata(&bytes).unwrap();
        assert_eq!(metadata.number, 42 + index as u32);
        assert_eq!(
            metadata.uuid.as_deref(),
            Some(format!("layer-{index}").as_str())
        );
        assert_eq!(metadata.modified_time, Some(index as i64));
        assert_eq!(metadata.property_mask, [2, 0, 0, 0, 1]);
        assert_eq!(metadata.field_mask, [0x18, 0, 0, 0, 2]);
        assert_eq!(metadata.fixed_trailing_data, [0x81; 7]);
        assert_eq!(metadata.flexible_trailing_data, [0x91, 0x92]);
        assert!(metadata.visible);
    }
}

#[test]
fn absent_flexible_fields_remain_absent_with_a_zero_offset() {
    let bytes = page(1, |offset, _| {
        let mut header = header(offset, 42, &[0], &[0], &[], &[]);
        header[4..8].fill(0);
        header
    });
    let parsed = parse_stored_page_bytes(&bytes).unwrap();
    let metadata = parsed.layers.layers[0].metadata(&bytes).unwrap();
    assert!(metadata.visible);
    assert!(!metadata.event_forwardable);
    assert_eq!(metadata.transparency, None);
    assert_eq!(metadata.name, None);
    assert_eq!(metadata.uuid, None);
    assert_eq!(metadata.modified_time, None);
    assert_eq!(metadata.shadow_effect, None);
}

#[test]
fn metadata_offsets_cannot_point_into_masks_objects_or_other_layers() {
    let bytes = page(2, |offset, index| {
        header(
            offset,
            index as u32,
            &[2],
            &[0x18],
            &[],
            &[string("layer"), 123_i64.to_le_bytes().to_vec()].concat(),
        )
    });
    let parsed = parse_stored_page_bytes(&bytes).unwrap();
    let layer = &parsed.layers.layers[0];
    let offset = layer.header_offset;
    for invalid_offset in [
        0,
        offset - 1,
        offset + 11,
        offset + 15,
        offset + layer.header_size + 1,
        parsed.layers.layers[1].header_offset,
    ] {
        let mut invalid = bytes.clone();
        invalid[offset + 4..offset + 8].copy_from_slice(&(invalid_offset as u32).to_le_bytes());
        assert!(layer.metadata(&invalid).is_err(), "offset {invalid_offset}");
    }
}

#[test]
fn truncated_known_fields_cannot_borrow_from_the_object_count_or_hash() {
    let fields = complete_fields();
    for length in 0..fields.len() {
        let bytes = page(1, |offset, _| {
            header(offset, 42, &[2], &[0x7f], &[], &fields[..length])
        });
        let parsed = parse_stored_page_bytes(&bytes).unwrap();
        assert!(
            parsed.layers.layers[0].metadata(&bytes).is_err(),
            "length {length}"
        );
    }
}

#[test]
fn strings_and_sized_effects_obey_bounds_before_allocation() {
    let bytes = page(1, |offset, _| {
        header(offset, 42, &[2], &[0x7f], &[], &complete_fields())
    });
    let parsed = parse_stored_page_bytes(&bytes).unwrap();
    let limits = ParseLimits {
        max_text_characters: 2,
        ..ParseLimits::default()
    };
    assert!(matches!(
        parsed.layers.layers[0].metadata_with_limits(&bytes, &limits),
        Err(Error::LimitExceeded {
            resource: "text characters",
            ..
        })
    ));

    let bytes = page(1, |offset, _| {
        header(offset, 42, &[2], &[0x40], &[], &u32::MAX.to_le_bytes())
    });
    let parsed = parse_stored_page_bytes(&bytes).unwrap();
    assert!(parsed.layers.layers[0].metadata(&bytes).is_err());

    let bytes = page(1, |offset, _| {
        header(offset, 42, &[2], &[4], &[], &[1, 0, 0, 0xdc])
    });
    let parsed = parse_stored_page_bytes(&bytes).unwrap();
    assert!(
        parsed.layers.layers[0]
            .metadata(&bytes)
            .unwrap_err()
            .to_string()
            .contains("invalid UTF-16")
    );
}
