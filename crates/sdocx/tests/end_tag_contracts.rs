#[allow(dead_code)]
mod support;

use std::io::{Cursor, Write};

use sdocx::{
    DiagnosticCode, Error, FormatVersion, ParseLimits, ParseOptions, parse_bytes_detailed,
    parse_bytes_detailed_with_options, parse_end_tag_bytes, parse_end_tag_bytes_with_limits,
};

fn string(value: &str) -> Vec<u8> {
    let units: Vec<_> = value.encode_utf16().collect();
    let mut bytes = (units.len() as u16).to_le_bytes().to_vec();
    bytes.extend(units.iter().flat_map(|unit| unit.to_le_bytes()));
    bytes
}

fn fields() -> Vec<Vec<u8>> {
    let core = [
        5500_u32.to_le_bytes().to_vec(),
        string("N🖊"),
        101_i64.to_le_bytes().to_vec(),
        0x102_u32.to_le_bytes().to_vec(),
        string("cover.png"),
        1080_u32.to_le_bytes().to_vec(),
        1527.5_f32.to_le_bytes().to_vec(),
        string("Samsung Notes"),
        4_i32.to_le_bytes().to_vec(),
        4_i32.to_le_bytes().to_vec(),
        string("45.37"),
        4000_u32.to_le_bytes().to_vec(),
        (-102_i64).to_le_bytes().to_vec(),
        (-1_i32).to_le_bytes().to_vec(),
        2_u16.to_le_bytes().to_vec(),
    ]
    .concat();
    let mut custom = 2_u32.to_le_bytes().to_vec();
    custom.extend("{}".encode_utf16().flat_map(|unit| unit.to_le_bytes()));
    vec![
        core,
        3_u16.to_le_bytes().to_vec(),
        string("owner"),
        [3_u32.to_le_bytes().as_slice(), &[0x81, 0x82, 0x83]].concat(),
        0_u32.to_le_bytes().to_vec(),
        [201_i64.to_le_bytes(), 202_i64.to_le_bytes()].concat(),
        301_i64.to_le_bytes().to_vec(),
        [
            string("Noto Sans"),
            (-1_i32).to_le_bytes().to_vec(),
            2_i32.to_le_bytes().to_vec(),
        ]
        .concat(),
        401_i64.to_le_bytes().to_vec(),
        2_i32.to_le_bytes().to_vec(),
        5400_i32.to_le_bytes().to_vec(),
        custom,
    ]
}

fn record(fields: &[Vec<u8>]) -> Vec<u8> {
    let mut payload = fields.concat();
    payload.extend(b"Document for S-Pen SDK");
    let mut bytes = (payload.len() as u16).to_le_bytes().to_vec();
    bytes.extend(payload);
    bytes
}

fn archive(tag: &[u8]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .start_file("page.page", zip::write::SimpleFileOptions::default())
        .unwrap();
    writer.write_all(&support::page(&[vec![]], 0, &[])).unwrap();
    writer
        .start_file("end_tag.bin", zip::write::SimpleFileOptions::default())
        .unwrap();
    writer.write_all(tag).unwrap();
    writer.finish().unwrap().into_inner()
}

#[test]
fn decodes_variable_strings_and_distinct_raw_and_display_timestamps() {
    let bytes = record(&fields());
    let tag = parse_end_tag_bytes(&bytes).unwrap();
    assert_eq!(tag.format_version, 5500);
    assert_eq!(tag.note_id.as_deref(), Some("N🖊"));
    assert_eq!(tag.modified_time, 101);
    assert_eq!(tag.created_time, -102);
    assert_eq!(tag.property_flags, 0x102);
    assert_eq!(tag.cover_image.as_deref(), Some("cover.png"));
    assert_eq!((tag.note_width, tag.note_height), (1080, 1527.5));
    assert_eq!(tag.application_name.as_deref(), Some("Samsung Notes"));
    assert_eq!(
        (tag.application_major_version, tag.application_minor_version),
        (4, 4)
    );
    assert_eq!(tag.application_patch_name.as_deref(), Some("45.37"));
    assert_eq!(tag.minimum_format_version, 4000);
    assert_eq!((tag.last_viewed_page_index, tag.page_mode), (-1, 2));
    assert_eq!(tag.document_type, Some(3));
    assert_eq!(tag.owner_id.as_deref(), Some("owner"));
    assert_eq!(
        tag.reserved_data.as_deref(),
        Some([0x81, 0x82, 0x83].as_slice())
    );
    assert_eq!(tag.encryption_data.as_deref(), Some([].as_slice()));
    let display = tag.display_timestamps.unwrap();
    assert_eq!((display.created_time, display.modified_time), (201, 202));
    assert_eq!(tag.last_recognized_data_modified_time, Some(301));
    let style = tag.fixed_style.unwrap();
    assert_eq!(style.font.as_deref(), Some("Noto Sans"));
    assert_eq!((style.text_direction, style.background_theme), (-1, 2));
    assert_eq!(tag.server_checkpoint, Some(401));
    assert_eq!(tag.new_orientation, Some(2));
    assert_eq!(tag.minimum_unknown_version, Some(5400));
    assert_eq!(tag.application_custom_data.as_deref(), Some("{}"));
    assert!(tag.trailing_data.is_empty());

    let parsed = parse_bytes_detailed(&archive(&bytes)).unwrap();
    assert_eq!(parsed.document.metadata.created_ms, Some(201));
    assert_eq!(parsed.document.metadata.modified_ms, Some(202));
    assert_eq!(
        parsed.document.metadata.format_version,
        Some(FormatVersion::CURRENT)
    );
    assert_eq!(parsed.end_tag.unwrap().created_time, -102);
}

#[test]
fn accepts_historical_extension_boundaries_and_rejects_partial_groups() {
    let groups = fields();
    for end in 1..=groups.len() {
        let tag = parse_end_tag_bytes(&record(&groups[..end])).unwrap();
        assert_eq!(tag.display_timestamps.is_some(), end > 5);
        assert_eq!(tag.fixed_style.is_some(), end > 7);
        assert_eq!(tag.application_custom_data.is_some(), end > 11);
        for length in 1..groups[end - 1].len() {
            let mut partial = groups[..end].to_vec();
            partial.last_mut().unwrap().truncate(length);
            assert!(
                parse_end_tag_bytes(&record(&partial)).is_err(),
                "group {end}, length {length}"
            );
        }
    }
    let parsed = parse_bytes_detailed(&archive(&record(&groups[..1]))).unwrap();
    assert_eq!(parsed.document.metadata.created_ms, Some(-102));
    assert_eq!(parsed.document.metadata.modified_ms, Some(101));
}

#[test]
fn retains_full_width_versions_and_unknown_extensions() {
    let mut groups = fields();
    groups[0][..4].copy_from_slice(&70000_u32.to_le_bytes());
    groups.push(vec![0x91, 0x92]);
    let bytes = record(&groups);
    let tag = parse_end_tag_bytes(&bytes).unwrap();
    assert_eq!(tag.format_version, 70000);
    assert_eq!(tag.trailing_data, [0x91, 0x92]);
    assert_eq!(
        parse_bytes_detailed(&archive(&bytes))
            .unwrap()
            .document
            .metadata
            .format_version,
        None
    );
}

#[test]
fn accepts_null_strings_without_consuming_the_next_field() {
    let mut groups = fields();
    groups[0].splice(4..12, u16::MAX.to_le_bytes());
    groups[2] = u16::MAX.to_le_bytes().to_vec();
    groups[7].splice(..20, u16::MAX.to_le_bytes());
    groups[11] = u32::MAX.to_le_bytes().to_vec();
    let tag = parse_end_tag_bytes(&record(&groups)).unwrap();
    assert_eq!(tag.note_id, None);
    assert_eq!(tag.modified_time, 101);
    assert_eq!(tag.owner_id, None);
    let style = tag.fixed_style.unwrap();
    assert_eq!(style.font, None);
    assert_eq!(style.text_direction, -1);
    assert_eq!(tag.server_checkpoint, Some(401));
    assert_eq!(tag.application_custom_data, None);
}

#[test]
fn rejects_invalid_size_signature_and_utf16() {
    let bytes = record(&fields());
    for length in 0..bytes.len() {
        assert!(parse_end_tag_bytes(&bytes[..length]).is_err());
    }
    let mut invalid = bytes.clone();
    *invalid.last_mut().unwrap() = 0;
    assert!(parse_end_tag_bytes(&invalid).is_err());
    let mut invalid = bytes;
    invalid[8..10].copy_from_slice(&0xdc00_u16.to_le_bytes());
    assert!(
        parse_end_tag_bytes(&invalid)
            .unwrap_err()
            .to_string()
            .contains("invalid UTF-16")
    );
}

#[test]
fn diagnoses_invalid_optional_members_without_inventing_metadata() {
    let parsed = parse_bytes_detailed(&archive(&[0; 200])).unwrap();
    assert!(parsed.end_tag.is_none());
    assert_eq!(parsed.document.metadata.created_ms, None);
    assert!(
        parsed
            .report
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::InvalidEndTag)
    );
}

#[test]
fn enforces_byte_and_text_limits_in_public_and_archive_parsers() {
    let bytes = record(&fields());
    let limits = ParseLimits {
        max_entry_size: 20,
        ..ParseLimits::default()
    };
    assert!(matches!(
        parse_end_tag_bytes_with_limits(&bytes, &limits),
        Err(Error::LimitExceeded {
            resource: "end tag size",
            ..
        })
    ));
    let limits = ParseLimits {
        max_text_characters: 2,
        ..ParseLimits::default()
    };
    assert!(matches!(
        parse_end_tag_bytes_with_limits(&bytes, &limits),
        Err(Error::LimitExceeded {
            resource: "text characters",
            ..
        })
    ));
    assert!(matches!(
        parse_bytes_detailed_with_options(&archive(&bytes), &ParseOptions { limits }),
        Err(Error::LimitExceeded {
            resource: "text characters",
            ..
        })
    ));
    let mut groups = fields();
    groups[11] = (u32::MAX - 1).to_le_bytes().to_vec();
    assert!(matches!(
        parse_end_tag_bytes(&record(&groups)),
        Err(Error::LimitExceeded { .. })
    ));
}
