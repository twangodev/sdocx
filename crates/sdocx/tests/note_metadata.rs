use sdocx::{Error, NoteMetadata, ParseLimits, parse_note_bytes};

fn string(value: &str) -> Vec<u8> {
    let mut bytes = (value.encode_utf16().count() as u16).to_le_bytes().to_vec();
    bytes.extend(value.encode_utf16().flat_map(u16::to_le_bytes));
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

fn note(fields: &[(usize, Vec<u8>)]) -> Vec<u8> {
    let mask_size = fields.last().map_or(1, |(bit, _)| bit / 8 + 1);
    let mut mask = vec![0; mask_size];
    for (bit, _) in fields {
        mask[bit / 8] |= 1 << (bit % 8);
    }
    let mut bytes = vec![0; 4];
    bytes.extend([1, 0, mask_size as u8]);
    bytes.extend(mask);
    bytes.extend(5500_u32.to_le_bytes());
    bytes.extend(string("note"));
    bytes.extend(1_u32.to_le_bytes());
    bytes.extend([0; 16]);
    for value in [1080_u32, 1527, 0, 0, 4000] {
        bytes.extend(value.to_le_bytes());
    }
    let mut base = 5500_u32.to_le_bytes().to_vec();
    base.extend([1, 0, b't']);
    base.extend([0; 45]);
    let text = [frame(0, &base), frame(6, &[]), frame(7, &[])].concat();
    for _ in 0..2 {
        bytes.extend((text.len() as u32).to_le_bytes());
        bytes.extend(&text);
    }
    let flexible_offset = bytes.len() as u32;
    bytes[..4].copy_from_slice(&flexible_offset.to_le_bytes());
    for (_, payload) in fields {
        bytes.extend(payload);
    }
    bytes.extend([0xee; 32]);
    bytes
}

fn decode(fields: &[(usize, Vec<u8>)]) -> sdocx::Result<NoteMetadata> {
    let bytes = note(fields);
    parse_note_bytes(&bytes)?.metadata(&bytes)
}

fn payload_size(payload: &[u8]) -> Vec<u8> {
    [(payload.len() as u32).to_le_bytes().as_slice(), payload].concat()
}

fn total_size(payload: &[u8]) -> Vec<u8> {
    [
        ((payload.len() + 4) as u32).to_le_bytes().as_slice(),
        payload,
    ]
    .concat()
}

fn string_table() -> Vec<u8> {
    let mut bytes = 2_u16.to_le_bytes().to_vec();
    for text in ["first", "second 🖊"] {
        bytes.extend(12_u32.to_le_bytes());
        bytes.extend(string(text));
    }
    bytes.extend([0xf1, 0xf2]);
    payload_size(&bytes)
}

fn pen(modern: bool) -> Vec<u8> {
    let mut bytes = string(if modern { "modern" } else { "compatible" });
    bytes.extend(4.5_f32.to_le_bytes());
    bytes.extend(0x80123456_u32.to_le_bytes());
    bytes.extend(1_u32.to_le_bytes());
    bytes.extend(string("{setting}"));
    for value in [1_i32, -7, 50] {
        bytes.extend(value.to_le_bytes());
    }
    if modern {
        bytes.extend(2.25_f32.to_le_bytes());
        bytes.extend(1_u32.to_le_bytes());
    }
    for value in [0.25_f32, 0.5, 0.75] {
        bytes.extend(value.to_le_bytes());
    }
    bytes.extend(17_u32.to_le_bytes());
    bytes
}

fn pen_extension() -> Vec<u8> {
    [
        1_u32.to_le_bytes(),
        0_u32.to_le_bytes(),
        0.875_f32.to_le_bytes(),
    ]
    .concat()
}

fn voice(recording_time: bool) -> Vec<u8> {
    let mut bytes = 31_u32.to_le_bytes().to_vec();
    bytes.extend(string("recording 🖊"));
    bytes.extend(string("00:03"));
    bytes.extend((-999_i64).to_le_bytes());
    bytes.extend(1_u32.to_le_bytes());
    bytes.extend(4_i32.to_le_bytes());
    bytes.extend(1234_i64.to_le_bytes());
    if recording_time {
        bytes.extend(3000_i64.to_le_bytes());
        bytes.extend([0xf3, 0xf4]);
    }
    bytes
}

fn voices() -> Vec<u8> {
    [
        2_u32.to_le_bytes().to_vec(),
        payload_size(&voice(false)),
        payload_size(&voice(true)),
    ]
    .concat()
}

fn attachments() -> Vec<u8> {
    let mut bytes = 2_u16.to_le_bytes().to_vec();
    for id in [31_u32, 32] {
        bytes.extend(string("duplicate.wav"));
        bytes.extend(id.to_le_bytes());
    }
    bytes
}

fn known_fields() -> Vec<(usize, Vec<u8>)> {
    vec![
        (0, string("Samsung Notes 🖊")),
        (
            1,
            [
                4_i32.to_le_bytes().to_vec(),
                7_i32.to_le_bytes().to_vec(),
                string("patch"),
            ]
            .concat(),
        ),
        (
            2,
            [
                string("Author 🖊"),
                u16::MAX.to_le_bytes().to_vec(),
                string(""),
                u32::MAX.to_le_bytes().to_vec(),
            ]
            .concat(),
        ),
        (
            3,
            [43.5_f64.to_le_bytes(), (-89.5_f64).to_le_bytes()].concat(),
        ),
        (6, string("template://grid")),
        (7, (-1_i32).to_le_bytes().to_vec()),
        (
            9,
            [
                33_u32.to_le_bytes().to_vec(),
                i64::MIN.to_le_bytes().to_vec(),
            ]
            .concat(),
        ),
        (10, string_table()),
        (11, (-12_i32).to_le_bytes().to_vec()),
        (12, pen(false)),
        (13, voices()),
        (14, attachments()),
        (
            15,
            total_size(&[pen(true), pen_extension(), vec![0xf5, 0xf6]].concat()),
        ),
        (16, i64::MAX.to_le_bytes().to_vec()),
        (17, string("Noto Sans")),
        (18, 2_i32.to_le_bytes().to_vec()),
        (19, 3_i32.to_le_bytes().to_vec()),
        (20, string("summary")),
        (21, 25_i32.to_le_bytes().to_vec()),
        (
            22,
            [
                3_u32.to_le_bytes().to_vec(),
                vec![b'A', 0, 0x3d, 0xd8, 0x8a, 0xdd],
            ]
            .concat(),
        ),
    ]
}

#[test]
fn decodes_consecutive_fields_with_distinct_values_and_preserves_repeated_ids() {
    let metadata = decode(&known_fields()).unwrap();
    assert_eq!(
        metadata.application_name.as_deref(),
        Some("Samsung Notes 🖊")
    );
    let version = metadata.application_version.unwrap();
    assert_eq!((version.major, version.minor), (4, 7));
    assert_eq!(version.patch_name, "patch");
    let author = metadata.author.unwrap();
    assert_eq!(author.name.as_deref(), Some("Author 🖊"));
    assert_eq!(author.phone_number, None);
    assert_eq!(author.email.as_deref(), Some(""));
    assert_eq!(author.image_media_id, u32::MAX);
    let location = metadata.location.unwrap();
    assert_eq!((location.latitude, location.longitude), (43.5, -89.5));
    assert_eq!(metadata.template_uri.as_deref(), Some("template://grid"));
    assert_eq!(metadata.last_edited_page_index, Some(-1));
    let edit = metadata.last_edited_page.unwrap();
    assert_eq!((edit.image_media_id, edit.time), (33, i64::MIN));
    let strings = metadata.string_table.unwrap();
    assert_eq!(
        strings
            .entries
            .iter()
            .map(|entry| (entry.id, entry.text.as_str()))
            .collect::<Vec<_>>(),
        [(12, "first"), (12, "second 🖊")]
    );
    assert_eq!(strings.trailing_data, [0xf1, 0xf2]);
    assert_eq!(metadata.body_font_size_delta, Some(-12));
    let compatible = metadata.compatible_pen.unwrap();
    assert_eq!(compatible.name, "compatible");
    assert_eq!(compatible.particle_size, None);
    assert_eq!(compatible.fixed_width, None);
    assert_eq!(compatible.extension, None);
    let pen = metadata.pen.unwrap();
    assert_eq!(pen.name, "modern");
    for settings in [&compatible, &pen] {
        assert_eq!(settings.size, 4.5);
        assert_eq!(settings.color, 0x80123456);
        assert!(settings.curvable);
        assert!(settings.eraser_enabled);
        assert_eq!(settings.advanced_setting, "{setting}");
        assert_eq!(settings.size_level, -7);
        assert_eq!(settings.particle_density, 50);
        assert_eq!(settings.hsv, [0.25, 0.5, 0.75]);
        assert_eq!(settings.color_ui_info, 17);
    }
    assert_eq!(pen.particle_size, Some(2.25));
    assert_eq!(pen.fixed_width, Some(true));
    let extension = pen.extension.unwrap();
    assert!(extension.fixed_opacity);
    assert!(!extension.auto_size_enabled);
    assert_eq!(extension.fit_ratio, 0.875);
    assert_eq!(pen.trailing_data, [0xf5, 0xf6]);
    let voices = metadata.voices.unwrap();
    assert_eq!(voices.len(), 2);
    for voice in &voices {
        assert_eq!(voice.media_id, 31);
        assert_eq!(voice.name, "recording 🖊");
        assert_eq!(voice.play_time, "00:03");
        assert_eq!(voice.created_time, -999);
        assert_eq!(voice.events.len(), 1);
        assert_eq!((voice.events[0].action, voice.events[0].time), (4, 1234));
    }
    assert_eq!(voices[0].recording_time, None);
    assert_eq!(voices[1].recording_time, Some(3000));
    assert_eq!(voices[1].trailing_data, [0xf3, 0xf4]);
    assert_eq!(
        metadata
            .attachments
            .unwrap()
            .iter()
            .map(|item| (item.name.as_str(), item.media_id))
            .collect::<Vec<_>>(),
        [("duplicate.wav", 31), ("duplicate.wav", 32)]
    );
    assert_eq!(metadata.server_checkpoint, Some(i64::MAX));
    assert_eq!(metadata.fixed_font.as_deref(), Some("Noto Sans"));
    assert_eq!(metadata.fixed_text_direction, Some(2));
    assert_eq!(metadata.fixed_background_theme, Some(3));
    assert_eq!(metadata.text_summarization.as_deref(), Some("summary"));
    assert_eq!(metadata.stroke_group_size, Some(25));
    assert_eq!(metadata.app_custom_data.as_deref(), Some("A🖊"));
    assert_eq!(metadata.first_unparsed_field, None);
    assert!(metadata.trailing_data.is_empty());
}

#[test]
fn every_known_field_decodes_in_isolation_and_rejects_truncation() {
    for (bit, payload) in known_fields() {
        let parsed = decode(&[(bit, payload.clone())]).unwrap();
        assert_eq!(parsed.first_unparsed_field, None, "bit {bit}");
        assert!(parsed.trailing_data.is_empty(), "bit {bit}");
        for length in 0..payload.len() {
            assert!(
                decode(&[(bit, payload[..length].to_vec())]).is_err(),
                "bit {bit}, length {length}"
            );
        }
    }
}

#[test]
fn unknown_fields_stop_decoding_before_later_fields_and_retain_all_remaining_bytes() {
    for bit in [4, 5, 8, 23, 40] {
        let mut fields = vec![(0, string("known")), (bit, vec![0xfa, 0xfb])];
        if bit < 17 {
            fields.push((17, string("do not guess")));
        }
        let expected = fields[1..]
            .iter()
            .flat_map(|(_, bytes)| bytes.clone())
            .collect::<Vec<_>>();
        let parsed = decode(&fields).unwrap();
        assert_eq!(parsed.application_name.as_deref(), Some("known"));
        assert_eq!(parsed.first_unparsed_field, Some(bit));
        assert_eq!(parsed.fixed_font, None);
        assert_eq!(parsed.trailing_data, expected);
    }
}

#[test]
fn sized_blocks_cannot_borrow_fields_from_the_next_record() {
    for (bit, mut block) in [(10, string_table()), (15, total_size(&pen(true)))] {
        let length = u32::from_le_bytes(block[..4].try_into().unwrap());
        let shortened = if bit == 10 { 7 } else { length - 1 };
        block[..4].copy_from_slice(&shortened.to_le_bytes());
        assert!(decode(&[(bit, block), (16, vec![0; 8])]).is_err());
    }
    let mut block = voices();
    block[4..8].copy_from_slice(&3_u32.to_le_bytes());
    assert!(decode(&[(13, block)]).is_err());
    for size in [0_u32, 1, 3, u32::MAX] {
        assert!(decode(&[(15, size.to_le_bytes().to_vec()), (16, vec![0; 8])]).is_err());
    }
}

#[test]
fn historical_voice_and_pen_records_end_only_at_known_optional_boundaries() {
    let base_pen = pen(true);
    let parsed = decode(&[
        (15, total_size(&base_pen)),
        (16, 77_i64.to_le_bytes().to_vec()),
    ])
    .unwrap();
    assert_eq!(parsed.pen.unwrap().extension, None);
    assert_eq!(parsed.server_checkpoint, Some(77));
    let tail = pen_extension();
    for length in 1..tail.len() {
        assert!(
            decode(&[(
                15,
                total_size(&[base_pen.clone(), tail[..length].to_vec()].concat())
            )])
            .is_err()
        );
    }
    let base_voice = voice(false);
    for length in 1..8 {
        let record = [base_voice.clone(), vec![0; length]].concat();
        let block = [1_u32.to_le_bytes().to_vec(), payload_size(&record)].concat();
        assert!(decode(&[(13, block)]).is_err());
    }
}

#[test]
fn metadata_limits_apply_to_strings_input_size_and_aggregate_collection_entries() {
    let bytes = note(&known_fields());
    let parsed = parse_note_bytes(&bytes).unwrap();
    let limits = ParseLimits {
        max_note_metadata_entries: 8,
        ..Default::default()
    };
    parsed.metadata_with_limits(&bytes, &limits).unwrap();
    let limits = ParseLimits {
        max_note_metadata_entries: 7,
        ..Default::default()
    };
    assert!(matches!(
        parsed.metadata_with_limits(&bytes, &limits),
        Err(Error::LimitExceeded {
            resource: "note metadata entries",
            actual: 8,
            ..
        })
    ));
    let limits = ParseLimits {
        max_entry_size: bytes.len() as u64 - 1,
        ..Default::default()
    };
    assert!(matches!(
        parsed.metadata_with_limits(&bytes, &limits),
        Err(Error::LimitExceeded {
            resource: "note size",
            ..
        })
    ));
    for (bit, data) in [
        (0, string("🖊")),
        (
            2,
            [string("🖊"), string(""), string(""), vec![0; 4]].concat(),
        ),
        (22, vec![2, 0, 0, 0, 0x3d, 0xd8, 0x8a, 0xdd]),
    ] {
        let bytes = note(&[(bit, data)]);
        let parsed = parse_note_bytes(&bytes).unwrap();
        let limits = ParseLimits {
            max_text_characters: 1,
            ..Default::default()
        };
        assert!(matches!(
            parsed.metadata_with_limits(&bytes, &limits),
            Err(Error::LimitExceeded {
                resource: "text characters",
                actual: 2,
                ..
            })
        ));
    }
    for bit in [13, 14] {
        let bytes = note(&[(bit, vec![0xff; 4])]);
        let parsed = parse_note_bytes(&bytes).unwrap();
        assert!(matches!(
            parsed.metadata(&bytes),
            Err(Error::LimitExceeded {
                resource: "note metadata entries",
                ..
            })
        ));
    }
}

#[test]
fn metadata_decoding_is_explicit_and_excludes_the_hash_trailer() {
    assert_eq!(decode(&[]).unwrap(), NoteMetadata::default());
    let bytes = note(&[(0, vec![3, 0, b'A', 0])]);
    let parsed = parse_note_bytes(&bytes).unwrap();
    assert!(parsed.metadata(&bytes).is_err());
    assert!(parsed.metadata(&bytes[..bytes.len() - 32]).is_err());
    assert!(parsed.metadata(&[]).is_err());
    let bytes = note(&[(0, vec![1, 0, 0x00, 0xdc])]);
    assert!(
        parse_note_bytes(&bytes)
            .unwrap()
            .metadata(&bytes)
            .unwrap_err()
            .to_string()
            .contains("UTF-16")
    );
    let bytes = note(&[(0, vec![0xff, 0xff])]);
    assert!(
        parse_note_bytes(&bytes)
            .unwrap()
            .metadata(&bytes)
            .unwrap_err()
            .to_string()
            .contains("null string sentinel")
    );
}
