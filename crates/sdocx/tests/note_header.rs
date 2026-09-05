#[allow(dead_code)]
mod support;

use std::io::{Cursor, Write};

use sdocx::{
    Color, Error, FormatVersion, ParseLimits, parse_note_bytes, parse_note_bytes_with_limits,
};
use sha2::{Digest, Sha256};

fn utf16(value: &str) -> Vec<u8> {
    let mut bytes = (value.encode_utf16().count() as u16).to_le_bytes().to_vec();
    bytes.extend(value.encode_utf16().flat_map(|unit| unit.to_le_bytes()));
    bytes
}

fn frame(kind: i16, fixed: &[u8]) -> Vec<u8> {
    let size = (12 + fixed.len()) as u32;
    let mut bytes = size.to_le_bytes().to_vec();
    bytes.extend(kind.to_le_bytes());
    bytes.extend(size.to_le_bytes());
    bytes.extend([0, 0]);
    bytes.extend(fixed);
    bytes
}

fn text_object() -> Vec<u8> {
    let mut base = 5500_u32.to_le_bytes().to_vec();
    base.extend(1_u16.to_le_bytes());
    base.push(b't');
    base.extend(0_i64.to_le_bytes());
    for value in [0_f64, 0.0, 1080.0, 1527.0] {
        base.extend(value.to_le_bytes());
    }
    base.extend([0; 5]);
    [frame(0, &base), frame(6, &[]), frame(7, &[])].concat()
}

fn note(properties: &[u8], fields: &[u8], id: &str, fixed_extra: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; 4];
    bytes.push(properties.len() as u8);
    bytes.extend(properties);
    bytes.push(fields.len() as u8);
    bytes.extend(fields);
    bytes.extend(5500_u32.to_le_bytes());
    bytes.extend(utf16(id));
    bytes.extend(12_u32.to_le_bytes());
    bytes.extend((-101_i64).to_le_bytes());
    bytes.extend(102_i64.to_le_bytes());
    for value in [2000_u32, 3000, 11, 12, 4000] {
        bytes.extend(value.to_le_bytes());
    }
    for _ in 0..2 {
        let text = text_object();
        bytes.extend((text.len() as u32).to_le_bytes());
        bytes.extend(text);
    }
    bytes.extend(fixed_extra);
    let flexible_offset = bytes.len() as u32;
    bytes[..4].copy_from_slice(&flexible_offset.to_le_bytes());
    bytes.extend(utf16("Samsung Notes"));
    bytes.extend(Sha256::digest(&bytes));
    bytes
}

fn archive(note: &[u8], page: &[u8], end_tag: Option<&[u8]>) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in [("note.note", note), ("page.page", page)] {
        writer
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(data).unwrap();
    }
    if let Some(tag) = end_tag {
        writer
            .start_file("end_tag.bin", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(tag).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn end_tag() -> Vec<u8> {
    let mut payload = 5400_u32.to_le_bytes().to_vec();
    payload.extend(utf16("tag"));
    payload.extend(902_i64.to_le_bytes());
    payload.extend(0_u32.to_le_bytes());
    payload.extend(utf16(""));
    payload.extend(1080_u32.to_le_bytes());
    payload.extend(1527_f32.to_le_bytes());
    payload.extend(utf16(""));
    payload.extend([0; 8]);
    payload.extend(utf16(""));
    payload.extend(4000_u32.to_le_bytes());
    payload.extend(901_i64.to_le_bytes());
    payload.extend([0; 6]);
    payload.extend(b"Document for S-Pen SDK");
    let mut bytes = (payload.len() as u16).to_le_bytes().to_vec();
    bytes.extend(payload);
    bytes
}

#[test]
fn variable_masks_and_unicode_ids_keep_all_header_fields_aligned() {
    for property_width in 1..=4 {
        for field_width in 1..=4 {
            let mut properties = vec![0; property_width];
            properties[0] = 24;
            let mut fields = vec![0; field_width];
            fields[0] = 1;
            let bytes = note(&properties, &fields, "note 🖊", &[]);
            let parsed = parse_note_bytes(&bytes).unwrap();
            let header = &parsed.header;
            assert_eq!(header.property_mask, properties);
            assert_eq!(header.field_mask, fields);
            assert_eq!(header.header_constant_1, property_width as u8);
            assert_eq!(header.header_constant_2, field_width as u8);
            assert_eq!(header.format_version, 5500);
            assert_eq!(header.note_id, "note 🖊");
            assert_eq!(header.file_revision, 12);
            assert_eq!(
                (header.created_time_raw, header.modified_time_raw),
                (-101, 102)
            );
            assert_eq!((header.width, header.height), (2000, 3000));
            assert_eq!(
                (header.page_horizontal_padding, header.page_vertical_padding),
                (11, 12)
            );
            assert_eq!(header.minimum_format_version, 4000);
            assert!(header.inverts_background_color());
            assert!(!header.tape_visible());
            assert_eq!(
                parsed.fixed_data_end,
                header.flexible_data_offset() as usize
            );
            assert!(parsed.fixed_trailing_data.is_empty());
            assert!(parsed.title.text.is_empty());
            assert!(parsed.body.text.is_empty());
        }
    }
}

#[test]
fn preserves_wider_future_masks_and_fixed_extensions() {
    let bytes = note(&[8, 0, 0, 0, 1], &[1, 0, 0, 0, 2], "future", &[0x91; 7]);
    let parsed = parse_note_bytes(&bytes).unwrap();
    assert_eq!(parsed.header.property_mask, [8, 0, 0, 0, 1]);
    assert_eq!(parsed.header.field_mask, [1, 0, 0, 0, 2]);
    assert_eq!(parsed.header.header_flags, 8);
    assert_eq!(parsed.header.property_flags, 1);
    assert_eq!(parsed.fixed_trailing_data, [0x91; 7]);
    assert_eq!(
        parsed.fixed_data_end + 7,
        parsed.header.flexible_data_offset() as usize
    );
}

#[test]
fn archive_metadata_uses_structured_note_fields_and_retains_end_tag_precedence() {
    let bytes = note(&[8], &[1, 0, 0], "variable length 🖊", &[]);
    let page = support::page(&[vec![]], 0, &[]);
    let parsed = sdocx::parse_bytes_detailed(&archive(&bytes, &page, None)).unwrap();
    let metadata = parsed.document.metadata;
    assert_eq!(metadata.format_version, Some(FormatVersion::CURRENT));
    assert_eq!(metadata.dark_mode_compatibility, Some(true));
    assert_eq!(metadata.created_ms, Some(-101));
    assert_eq!(metadata.modified_ms, Some(102));
    assert_eq!(metadata.flow_dimensions, Some((2000, 3000)));
    assert_eq!(metadata.flow_page_padding, Some((11, 12)));
    assert_eq!(metadata.page_dimensions, Some((1080, 1527)));
    let parsed = sdocx::parse_bytes_detailed(&archive(&bytes, &page, Some(&end_tag()))).unwrap();
    assert_eq!(
        parsed.document.metadata.format_version,
        Some(FormatVersion(5400))
    );
    assert_eq!(parsed.document.metadata.created_ms, Some(901));
    assert_eq!(parsed.document.metadata.modified_ms, Some(902));
}

#[test]
fn arbitrary_note_bytes_do_not_supply_the_background_color() {
    let decoy = [24, 0, 0, 0, 1, 0, 0, 0, 0x81, 0x82, 0x83, 0xff];
    let bytes = note(&[0], &[1], "note", &decoy);
    let page = support::page(&[vec![]], 0, &[]);
    let parsed = sdocx::parse_bytes_detailed(&archive(&bytes, &page, None)).unwrap();
    assert_eq!(parsed.document.metadata.background_color, None);
    assert_eq!(
        parsed.document.metadata.dark_mode_compatibility,
        Some(false)
    );
    let page = support::page(&[vec![]], 1 << 5, &0xff123456_u32.to_le_bytes());
    let parsed = sdocx::parse_bytes_detailed(&archive(&bytes, &page, None)).unwrap();
    assert_eq!(
        parsed.document.metadata.background_color,
        Some(Color {
            r: 0x12,
            g: 0x34,
            b: 0x56
        })
    );
}

#[test]
fn fixed_fields_respect_the_declared_flexible_boundary() {
    let bytes = note(&[8], &[1], "note", &[]);
    let fixed_end = parse_note_bytes(&bytes).unwrap().fixed_data_end;
    for offset in 0..fixed_end {
        let mut invalid = bytes.clone();
        invalid[..4].copy_from_slice(&(offset as u32).to_le_bytes());
        assert!(parse_note_bytes(&invalid).is_err(), "offset {offset}");
    }
    for offset in [bytes.len() + 1, u32::MAX as usize] {
        let mut invalid = bytes.clone();
        invalid[..4].copy_from_slice(&(offset as u32).to_le_bytes());
        assert!(parse_note_bytes(&invalid).is_err());
    }
    let body_size_offset = fixed_end - text_object().len() - 4;
    let mut invalid = bytes.clone();
    invalid[body_size_offset..body_size_offset + 4]
        .copy_from_slice(&((text_object().len() + 1) as u32).to_le_bytes());
    assert!(
        parse_note_bytes(&invalid)
            .unwrap_err()
            .to_string()
            .contains("body object")
    );
}

#[test]
fn masks_and_strings_are_bounded_and_full_width_versions_are_retained() {
    let bytes = note(&[8], &[1], "note", &[]);
    for length in 0..bytes.len() - 32 - utf16("Samsung Notes").len() {
        assert!(parse_note_bytes(&bytes[..length]).is_err());
    }
    let mut invalid = bytes.clone();
    invalid[4] = 255;
    assert!(parse_note_bytes(&invalid).is_err());
    let mut invalid = bytes.clone();
    invalid[14..16].copy_from_slice(&0xdc00_u16.to_le_bytes());
    assert!(
        parse_note_bytes(&invalid)
            .unwrap_err()
            .to_string()
            .contains("UTF-16")
    );
    let limits = ParseLimits {
        max_text_characters: 2,
        ..Default::default()
    };
    assert!(matches!(
        parse_note_bytes_with_limits(&bytes, &limits),
        Err(Error::LimitExceeded {
            resource: "text characters",
            ..
        })
    ));
    let limits = ParseLimits {
        max_entry_size: 8,
        ..Default::default()
    };
    assert!(matches!(
        parse_note_bytes_with_limits(&bytes, &limits),
        Err(Error::LimitExceeded {
            resource: "note size",
            ..
        })
    ));
    let mut future = bytes;
    future[8..12].copy_from_slice(&70000_u32.to_le_bytes());
    assert_eq!(
        parse_note_bytes(&future).unwrap().header.format_version,
        70000
    );
    let parsed =
        sdocx::parse_bytes_detailed(&archive(&future, &support::page(&[vec![]], 0, &[]), None))
            .unwrap();
    assert_eq!(parsed.document.metadata.format_version, None);
}
