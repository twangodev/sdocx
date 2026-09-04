use sdocx::{
    Error, ParseLimits, parse_media_manifest_bytes, parse_media_manifest_bytes_with_limits,
};

fn record(id: u32, name: &str, hash: &[u8]) -> Vec<u8> {
    let mut record = id.to_le_bytes().to_vec();
    record.extend_from_slice(&(name.encode_utf16().count() as u16).to_le_bytes());
    for unit in name.encode_utf16() {
        record.extend_from_slice(&unit.to_le_bytes());
    }
    record.extend_from_slice(hash);
    record.extend_from_slice(&2_u16.to_le_bytes());
    record.extend_from_slice(&1234_i64.to_le_bytes());
    record.push(1);
    let mut bytes = (record.len() as u32).to_le_bytes().to_vec();
    bytes.extend(record);
    bytes
}

fn manifest(records: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = 5500_u32.to_le_bytes().to_vec();
    bytes.extend_from_slice(&(records.len() as u16).to_le_bytes());
    for record in records {
        bytes.extend(record);
    }
    bytes.extend(b"EOFX");
    bytes
}

#[test]
fn preserves_bind_ids_independently_of_filenames_and_order() {
    let data = manifest(&[
        record(42, "9@画像.png", &[b'a'; 64]),
        record(3, "plain.jpg", &[0, 0]),
    ]);
    let parsed = parse_media_manifest_bytes(&data).unwrap();
    assert_eq!(parsed.format_version, 5500);
    assert_eq!(parsed.entries[0].bind_id, 42);
    assert_eq!(parsed.entries[0].file_name, "9@画像.png");
    assert_eq!(
        parsed.entries[0].sha256.as_deref(),
        Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
    );
    assert_eq!(parsed.entries[0].reference_count, 2);
    assert_eq!(parsed.entries[0].modified_time_raw, 1234);
    assert!(parsed.entries[0].is_attached);
    assert_eq!(parsed.entries[1].bind_id, 3);
    assert!(parsed.entries[1].sha256.is_none());
    assert!(
        parse_media_manifest_bytes(&manifest(&[]))
            .unwrap()
            .entries
            .is_empty()
    );
}

#[test]
fn records_cannot_borrow_from_the_next_record_or_end_marker() {
    let valid = manifest(&[record(1, "a.png", &[0, 0]), record(2, "b.png", &[b'b'; 64])]);
    for end in 0..valid.len() {
        assert!(
            parse_media_manifest_bytes(&valid[..end]).is_err(),
            "accepted {end} bytes"
        );
    }
    for size in [0_u32, 8, 21, u32::MAX] {
        let mut bytes = valid.clone();
        bytes[6..10].copy_from_slice(&size.to_le_bytes());
        assert!(parse_media_manifest_bytes(&bytes).is_err());
    }
    assert!(parse_media_manifest_bytes(&manifest(&[record(1, "x", &[b'z'; 64])])).is_err());
}

#[test]
fn limits_counts_and_retains_sized_extensions() {
    let data = manifest(&[record(1, "a", &[0, 0])]);
    let limits = ParseLimits {
        max_archive_entries: 0,
        ..ParseLimits::default()
    };
    assert!(matches!(
        parse_media_manifest_bytes_with_limits(&data, &limits),
        Err(Error::LimitExceeded {
            resource: "media manifest entries",
            ..
        })
    ));
    let mut entry = record(1, "a", &[0, 0]);
    let size = u32::from_le_bytes(entry[..4].try_into().unwrap()) + 3;
    entry[..4].copy_from_slice(&size.to_le_bytes());
    entry.extend([10, 11, 12]);
    let mut bytes = manifest(&[entry]);
    bytes.extend([13, 14]);
    let parsed = parse_media_manifest_bytes(&bytes).unwrap();
    assert_eq!(parsed.entries[0].trailing_data, [10, 11, 12]);
    assert_eq!(parsed.trailing_data, [13, 14]);
}
