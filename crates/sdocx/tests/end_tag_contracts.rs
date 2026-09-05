#[allow(dead_code)]
mod support;

use std::io::{Cursor, Write};

use sdocx::{
    DiagnosticCode, EndTagSource, Error, FormatVersion, ParseLimits, ParseOptions,
    parse_bytes_detailed, parse_bytes_detailed_with_options, parse_end_tag_bytes,
    parse_end_tag_bytes_with_limits,
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
    archive_with_comment(Some(tag), Vec::new())
}

fn archive_with_comment(tag: Option<&[u8]>, comment: Vec<u8>) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    writer.set_raw_comment(comment.into_boxed_slice());
    writer
        .start_file("page.page", zip::write::SimpleFileOptions::default())
        .unwrap();
    writer.write_all(&support::page(&[vec![]], 0, &[])).unwrap();
    if let Some(tag) = tag {
        writer
            .start_file("end_tag.bin", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(tag).unwrap();
    }
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
    assert_eq!(parsed.end_tag_source, Some(EndTagSource::ArchiveEntry));
}

#[test]
fn appended_tags_override_members_after_the_complete_zip_comment() {
    let inner = record(&fields());
    let mut outer = fields();
    outer[5] = [901_i64.to_le_bytes(), 902_i64.to_le_bytes()].concat();
    for comment in [Vec::new(), b"export PK\x05\x06 comment".to_vec()] {
        let mut bytes = archive_with_comment(Some(&inner), comment);
        bytes.extend(record(&outer));
        let parsed = parse_bytes_detailed(&bytes).unwrap();
        assert_eq!(parsed.document.metadata.created_ms, Some(901));
        assert_eq!(parsed.document.metadata.modified_ms, Some(902));
        assert_eq!(parsed.end_tag_source, Some(EndTagSource::Appended));
        assert!(
            !parsed
                .report
                .diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::InvalidEndTag)
        );
    }
}

#[test]
fn appended_tags_work_without_a_member_and_cannot_supply_a_fake_zip_footer() {
    let mut groups = fields();
    let mut fake_footer = b"PK\x05\x06".to_vec();
    fake_footer.resize(22, 0);
    groups[3] = [
        (fake_footer.len() as u32).to_le_bytes().to_vec(),
        fake_footer,
    ]
    .concat();
    let mut bytes = archive_with_comment(None, Vec::new());
    bytes.extend(record(&groups));
    let parsed = parse_bytes_detailed(&bytes).unwrap();
    assert_eq!(parsed.document.pages.len(), 1);
    assert_eq!(parsed.end_tag_source, Some(EndTagSource::Appended));
    assert_eq!(parsed.document.metadata.modified_ms, Some(202));
}

#[test]
fn bounds_tail_search_for_maximum_zip_comment_and_record_lengths() {
    let mut groups = fields();
    let extra = usize::from(u16::MAX) + 2 - record(&groups).len();
    groups.push(vec![0x91; extra]);
    let mut bytes = archive_with_comment(None, vec![b'c'; usize::from(u16::MAX)]);
    bytes.extend(record(&groups));
    let parsed = parse_bytes_detailed(&bytes).unwrap();
    assert_eq!(parsed.end_tag_source, Some(EndTagSource::Appended));
    assert_eq!(parsed.end_tag.unwrap().trailing_data.len(), extra);
}

#[test]
fn malformed_appended_tags_report_their_source_and_fall_back_to_the_member() {
    let inner = record(&fields());
    let mut corruptions = Vec::new();
    let mut invalid_size = inner.clone();
    invalid_size[0] ^= 1;
    corruptions.push(invalid_size);
    let mut invalid_signature = inner.clone();
    *invalid_signature.last_mut().unwrap() = 0;
    corruptions.push(invalid_signature);
    let mut invalid_string = inner.clone();
    invalid_string[8..10].copy_from_slice(&0xdc00_u16.to_le_bytes());
    corruptions.push(invalid_string);
    for outer in corruptions {
        let mut bytes = archive(&inner);
        bytes.extend(outer);
        let parsed = parse_bytes_detailed(&bytes).unwrap();
        assert_eq!(parsed.end_tag_source, Some(EndTagSource::ArchiveEntry));
        assert_eq!(parsed.document.metadata.modified_ms, Some(202));
        let warning = parsed
            .report
            .diagnostics
            .iter()
            .find(|d| d.code == DiagnosticCode::InvalidEndTag)
            .unwrap();
        assert!(warning.archive_entry.is_none());
        assert!(warning.message.contains("Appended"));
    }
}

#[test]
fn signatures_in_zip_comments_are_not_appended_records() {
    let mut comment = b"PK\x05\x06".to_vec();
    comment.resize(22, 0);
    comment[8..12].fill(0xff);
    comment.extend(record(&fields()));
    let bytes = archive_with_comment(None, comment);
    let parsed = parse_bytes_detailed(&bytes).unwrap();
    assert!(parsed.end_tag.is_none());
    assert!(parsed.end_tag_source.is_none());
    assert_eq!(parsed.document.metadata.created_ms, None);
}

#[test]
fn appended_tags_preserve_zip64_directory_offsets() {
    let mut bytes = archive_with_comment(None, Vec::new());
    let footer_position = bytes.len() - 22;
    let mut footer = bytes.split_off(footer_position);
    let directory_size = u32::from_le_bytes(footer[12..16].try_into().unwrap());
    let directory_offset = u32::from_le_bytes(footer[16..20].try_into().unwrap());
    bytes.extend(b"PK\x06\x06");
    bytes.extend(44_u64.to_le_bytes());
    bytes.extend(45_u16.to_le_bytes());
    bytes.extend(45_u16.to_le_bytes());
    bytes.extend([0; 8]);
    for value in [1, 1, u64::from(directory_size), u64::from(directory_offset)] {
        bytes.extend(value.to_le_bytes());
    }
    bytes.extend(b"PK\x06\x07");
    bytes.extend(0_u32.to_le_bytes());
    bytes.extend((footer_position as u64).to_le_bytes());
    bytes.extend(1_u32.to_le_bytes());
    footer[8..20].fill(0xff);
    bytes.extend(footer);
    bytes.extend(record(&fields()));
    let parsed = parse_bytes_detailed(&bytes).unwrap();
    assert_eq!(parsed.document.pages.len(), 1);
    assert_eq!(parsed.end_tag_source, Some(EndTagSource::Appended));
}

#[test]
fn appended_tags_preserve_archives_with_a_preamble() {
    let mut bytes = b"archive preamble".to_vec();
    bytes.extend(archive_with_comment(None, Vec::new()));
    bytes.extend(record(&fields()));
    let parsed = parse_bytes_detailed(&bytes).unwrap();
    assert_eq!(parsed.document.pages.len(), 1);
    assert_eq!(parsed.end_tag_source, Some(EndTagSource::Appended));
}

#[test]
fn appended_metadata_obeys_limits_even_when_the_member_is_valid() {
    let inner = record(&fields());
    let mut outer = fields();
    outer[11] = (u32::MAX - 1).to_le_bytes().to_vec();
    let mut bytes = archive(&inner);
    bytes.extend(record(&outer));
    assert!(matches!(
        parse_bytes_detailed(&bytes),
        Err(Error::LimitExceeded {
            resource: "text characters",
            ..
        })
    ));
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
