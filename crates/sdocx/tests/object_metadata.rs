#[allow(dead_code)]
mod support;

use sdocx::{ObjectMetadata, ObjectResizeMode, Result, parse_stored_page_bytes};

fn frame(properties: &[u8], fields: &[u8], fixed: &[u8], flexible: &[u8]) -> Vec<u8> {
    let offset = 12 + properties.len() + fields.len() + fixed.len();
    let mut bytes = ((offset + flexible.len()) as u32).to_le_bytes().to_vec();
    bytes.extend(0_i16.to_le_bytes());
    bytes.extend((offset as u32).to_le_bytes());
    bytes.push(properties.len() as u8);
    bytes.extend(properties);
    bytes.push(fields.len() as u8);
    bytes.extend(fields);
    bytes.extend(fixed);
    bytes.extend(flexible);
    bytes
}

fn fixed() -> Vec<u8> {
    let uuid = "object-🖊";
    let mut bytes = 5500_u32.to_le_bytes().to_vec();
    bytes.extend((uuid.len() as u16).to_le_bytes());
    bytes.extend(uuid.as_bytes());
    bytes.extend((-123456789_i64).to_le_bytes());
    bytes.extend(
        [1.0_f64, 2.0, 30.0, 40.0]
            .into_iter()
            .flat_map(f64::to_le_bytes),
    );
    bytes.extend((-987_i32).to_le_bytes());
    bytes.push(255);
    bytes
}

fn metadata(payload: &[u8]) -> Result<ObjectMetadata> {
    let bytes = support::page(&[vec![support::object(250, payload, &[])]], 0, &[]);
    let page = parse_stored_page_bytes(&bytes)?;
    page.layers.layers[0].objects[0].base_metadata(&bytes)
}

#[test]
fn common_metadata_preserves_identity_raw_values_and_bounded_extensions() {
    let fixed = [fixed(), vec![0x81, 0x82]].concat();
    let flexible = [(-15.5_f32).to_le_bytes().to_vec(), vec![0x91, 0x92]].concat();
    let payload = [
        frame(&[0xff, 0xff, 0, 0, 1], &[3, 0, 0, 0, 2], &fixed, &flexible),
        frame(&[], &[], &[0x99; 10], &[]),
    ]
    .concat();
    let value = metadata(&payload).unwrap();
    assert_eq!(value.format_version, 5500);
    assert_eq!(value.uuid, "object-🖊");
    assert_eq!(value.modified_time_raw, -123456789);
    assert_eq!(value.bbox.x_min, 1.0);
    assert_eq!(value.bbox.y_min, 2.0);
    assert_eq!(value.bbox.x_max, 30.0);
    assert_eq!(value.bbox.y_max, 40.0);
    assert_eq!(value.replay_timestamp_raw, -987);
    assert_eq!(value.resize_mode_raw, 255);
    assert_eq!(value.rotation_degrees, Some(-15.5));
    assert_eq!(value.property_mask, [0xff, 0xff, 0, 0, 1]);
    assert_eq!(value.field_mask, [3, 0, 0, 0, 2]);
    assert_eq!(value.fixed_trailing_data, [0x81, 0x82]);
    assert_eq!(value.flexible_trailing_data, [0x91, 0x92]);
}

#[test]
fn native_property_bits_are_independent_and_removable_is_inverted() {
    for bit in 0..40 {
        let mut mask = [0; 5];
        mask[bit / 8] = 1 << (bit % 8);
        let value = metadata(&frame(&mask, &[], &fixed(), &[])).unwrap();
        let properties = [
            value.rotatable,
            value.selectable,
            value.movable,
            value.visible,
            value.replayable,
            value.out_of_canvas_enabled,
            value.template,
            value.flip_enabled,
            value.float_drawn_rect,
            value.locked,
            !value.removable,
        ];
        let expected = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 12].map(|property_bit| bit == property_bit);
        assert_eq!(properties, expected, "property bit {bit}");
    }
}

#[test]
fn resize_modes_preserve_values_outside_the_native_getters_supported_range() {
    for (raw, expected) in [
        (0, ObjectResizeMode::Free),
        (1, ObjectResizeMode::KeepRatio),
        (2, ObjectResizeMode::Disabled),
        (3, ObjectResizeMode::Other(3)),
        (255, ObjectResizeMode::Other(255)),
    ] {
        let mut data = fixed();
        *data.last_mut().unwrap() = raw;
        let value = metadata(&frame(&[8], &[], &data, &[])).unwrap();
        assert_eq!(value.resize_mode_raw, raw);
        assert_eq!(value.resize_mode(), expected);
    }
}

#[test]
fn short_property_masks_zero_extend_without_defaulting_objects_to_visible() {
    for mask in [vec![], vec![0], vec![0, 0]] {
        let mut payload = frame(&mask, &[], &fixed(), &[]);
        payload[6..10].fill(0);
        let value = metadata(&payload).unwrap();
        assert!(!value.visible);
        assert!(!value.locked);
        assert!(value.removable);
        assert_eq!(value.rotation_degrees, None);
        assert_eq!(value.property_mask, mask);
        assert!(value.flexible_trailing_data.is_empty());
    }
    let value = metadata(&frame(&[8], &[], &fixed(), &[])).unwrap();
    assert!(value.visible);
    assert!(!value.locked);
    assert!(value.removable);
}

#[test]
fn undecoded_flexible_fields_survive_without_a_rotation_field() {
    let value = metadata(&frame(&[8], &[0xfe, 255], &fixed(), &[1, 2, 3, 4])).unwrap();
    assert_eq!(value.rotation_degrees, None);
    assert_eq!(value.flexible_trailing_data, [1, 2, 3, 4]);
}

#[test]
fn fixed_fields_cannot_borrow_from_flexible_data_or_the_next_frame() {
    let data = fixed();
    for length in 0..data.len() {
        let payload = [
            frame(&[8], &[1], &data[..length], &[0; 256]),
            frame(&[], &[], &[0; 256], &[]),
        ]
        .concat();
        assert!(metadata(&payload).is_err(), "fixed length {length}");
    }
}

#[test]
fn rotation_cannot_borrow_from_the_next_frame() {
    for length in 0..4 {
        let payload = [
            frame(&[8], &[1], &fixed(), &15.5_f32.to_le_bytes()[..length]),
            frame(&[], &[], &[0; 256], &[]),
        ]
        .concat();
        assert!(metadata(&payload).is_err(), "rotation length {length}");
    }
}

#[test]
fn nonfinite_rotation_is_rejected() {
    for rotation in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(metadata(&frame(&[8], &[1], &fixed(), &rotation.to_le_bytes())).is_err());
    }
}
