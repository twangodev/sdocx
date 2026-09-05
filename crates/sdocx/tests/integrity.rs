#[allow(dead_code)]
mod support;

use std::io::{Cursor, Write};

use sdocx::{DiagnosticCode, IntegrityCounts, IntegrityReport, ParseOptions, ParsedDocument};
use sha2::{Digest, Sha256};

const PARENT_HASH: &str = "f2a0ede82b5b172b5fe082f344cf232da686bda1c8e73017009cdf979efd52ad";
const CHILD_HASH: &str = "0cf4872c809bec249ebe3a2475702bafdca679a486ad5fe839ffc21bb7b4f26a";
const LEAF_HASH: &str = "3c0d83e4e4879ca7c2f92e172e4dd238a205d2693e59bd268b3bce50be5ea7a9";
const SIBLING_HASH: &str = "ec24920a5fc396bf61591973443242b9af19606ba45561dbbf7b33a75d4b0a9f";
const LAYER_A_HASH: &str = "cf36664978f7f6536c0092a1b4bc1ef2fad4c674732337e4207e1e6efe56cf7e";
const LAYER_B_HASH: &str = "a0bd0176104b9141d066925d144fdc917d70d7e530f57fb8eae692cbc3cd07ac";
const PAGE_HASH: &str = "5980e1f77d1ddef35b4af0412841360c178793fd1a09fb18265c0c0eae2ff34e";

fn hash(hex: &str) -> [u8; 32] {
    std::array::from_fn(|index| u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap())
}

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

fn base(uuid: &str, time: i64) -> Vec<u8> {
    let mut fixed = 5500_u32.to_le_bytes().to_vec();
    fixed.extend((uuid.len() as u16).to_le_bytes());
    fixed.extend(uuid.as_bytes());
    fixed.extend(time.to_le_bytes());
    for value in [0_f64, 0.0, 10.0, 10.0] {
        fixed.extend(value.to_le_bytes());
    }
    fixed.extend([0; 5]);
    frame(0, &fixed)
}

fn object(uuid: &str, time: i64, digest: &str, children: &[Vec<u8>]) -> Vec<u8> {
    let payload = base(uuid, time);
    let mut bytes = vec![250];
    bytes.extend((children.len() as u16).to_le_bytes());
    bytes.extend(((payload.len() + 32) as u32).to_le_bytes());
    bytes.extend(payload);
    bytes.extend(hash(digest));
    for child in children {
        bytes.extend(child);
    }
    bytes
}

fn layer(bytes: &mut Vec<u8>, uuid: &str, time: i64, digest: &str, objects: &[Vec<u8>]) {
    let start = bytes.len();
    bytes.extend([0; 8]);
    bytes.extend([1, 2, 1, 0x18]);
    bytes.extend(0_u32.to_le_bytes());
    let flexible_offset = bytes.len() as u32;
    bytes[start + 4..start + 8].copy_from_slice(&flexible_offset.to_le_bytes());
    bytes.extend(utf16(uuid));
    bytes.extend(time.to_le_bytes());
    let size = (bytes.len() - start) as u32;
    bytes[start..start + 4].copy_from_slice(&size.to_le_bytes());
    bytes.extend((objects.len() as u32).to_le_bytes());
    for object in objects {
        bytes.extend(object);
    }
    bytes.extend(hash(digest));
}

fn page() -> Vec<u8> {
    let leaf = object("leaf", i64::MAX, LEAF_HASH, &[]);
    let child = object("child🖊", i64::MIN, CHILD_HASH, &[leaf]);
    let parent = object("parent", -1, PARENT_HASH, &[child]);
    let sibling = object("sibling", 0, SIBLING_HASH, &[]);
    let mut bytes = support::page(&[vec![]], 0, &[]);
    let layer_offset = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
    bytes.truncate(layer_offset);
    bytes[layer_offset - 16..layer_offset - 8].copy_from_slice(&(-30_i64).to_le_bytes());
    bytes.extend(2_u16.to_le_bytes());
    bytes.extend(0_u16.to_le_bytes());
    layer(&mut bytes, "layer-a", -20, LAYER_A_HASH, &[parent, sibling]);
    layer(&mut bytes, "layer-b", 21, LAYER_B_HASH, &[]);
    bytes.extend(hash(PAGE_HASH));
    bytes.extend(b"Page for SAMSUNG S-Pen SDK");
    bytes
}

fn note() -> Vec<u8> {
    let mut bytes = vec![0; 4];
    bytes.push(4);
    bytes.extend(0_u32.to_le_bytes());
    bytes.push(4);
    bytes.extend(1_u32.to_le_bytes());
    bytes.extend(5500_u32.to_le_bytes());
    bytes.extend(utf16("note"));
    bytes.extend(1_u32.to_le_bytes());
    bytes.extend(101_i64.to_le_bytes());
    bytes.extend(102_i64.to_le_bytes());
    for value in [1080_u32, 1527, 0, 0, 4000] {
        bytes.extend(value.to_le_bytes());
    }
    for name in ["title", "body"] {
        let text = [base(name, 0), frame(6, &[]), frame(7, &[])].concat();
        bytes.extend((text.len() as u32).to_le_bytes());
        bytes.extend(text);
    }
    let flexible_offset = bytes.len() as u32;
    bytes[..4].copy_from_slice(&flexible_offset.to_le_bytes());
    bytes.extend(utf16("N"));
    bytes.extend(Sha256::digest(&bytes));
    bytes
}

fn manifest(note: &[u8], records: &[(&str, [u8; 32])]) -> Vec<u8> {
    let mut bytes = note[note.len() - 32..].to_vec();
    bytes.extend((records.len() as u16).to_le_bytes());
    for (id, digest) in records {
        bytes.extend(utf16(id));
        bytes.extend(digest);
    }
    bytes
}

fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in entries {
        writer
            .start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(data).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn verify(note: &[u8], page: &[u8], manifest: &[u8]) -> ParsedDocument {
    verify_archive(&archive(&[
        ("note.note", note),
        ("page.page", page),
        ("pageIdInfo.dat", manifest),
    ]))
}

fn verify_archive(bytes: &[u8]) -> ParsedDocument {
    sdocx::parse_bytes_detailed_with_options(
        bytes,
        &ParseOptions {
            verify_integrity: true,
            ..Default::default()
        },
    )
    .unwrap()
}

fn matches(count: usize) -> IntegrityCounts {
    IntegrityCounts {
        matched: count,
        ..Default::default()
    }
}

fn golden_report() -> IntegrityReport {
    IntegrityReport {
        note: matches(1),
        objects: matches(4),
        layers: matches(2),
        pages: matches(1),
        manifest: matches(2),
    }
}

#[test]
fn verifies_reference_hashes_for_signed_unicode_identities_and_depth_first_trees() {
    let note = note();
    let parsed = verify(
        &note,
        &page(),
        &manifest(&note, &[("page", hash(PAGE_HASH))]),
    );
    assert_eq!(parsed.integrity, Some(golden_report()));
    assert!(!parsed.report.diagnostics.iter().any(|d| matches!(
        d.code,
        DiagnosticCode::IntegrityMismatch | DiagnosticCode::IntegrityUnavailable
    )));
}

#[test]
fn integrity_is_opt_in_and_does_not_change_the_decoded_document() {
    let page = page();
    let bytes = archive(&[("page.page", &page)]);
    let ordinary = sdocx::parse_bytes_detailed(&bytes).unwrap();
    let checked = verify_archive(&bytes);
    assert!(ordinary.integrity.is_none());
    assert_eq!(
        format!("{:?}", ordinary.document),
        format!("{:?}", checked.document)
    );
    assert_eq!(checked.integrity.unwrap().manifest.unavailable, 1);
}

#[test]
fn detects_independent_mutations_at_every_hash_level() {
    let note = note();
    let page = page();
    let manifest = manifest(&note, &[("page", hash(PAGE_HASH))]);
    let stored = sdocx::parse_stored_page_bytes(&page).unwrap();
    let parent = &stored.layers.layers[0].objects[0];
    let leaf = &parent.children[0].children[0];
    let cases = [
        ("object", leaf.payload_offset + leaf.payload_size),
        ("layer", stored.layers.layers[1].header_offset - 32),
        ("page", stored.integrity_offset),
    ];
    for (kind, offset) in cases {
        let mut corrupt = page.clone();
        corrupt[offset] ^= 1;
        let parsed = verify(&note, &corrupt, &manifest);
        let report = parsed.integrity.unwrap();
        let mut expected = golden_report();
        let mut fail = |scope: &str| {
            let counts = match scope {
                "object" => &mut expected.objects,
                "layer" => &mut expected.layers,
                "page" => &mut expected.pages,
                _ => &mut expected.manifest,
            };
            counts.matched -= 1;
            counts.mismatched += 1;
        };
        fail(kind);
        fail(match kind {
            "object" => "layer",
            "layer" => "page",
            _ => "manifest",
        });
        assert_eq!(report, expected, "{kind}");
        assert_eq!(
            parsed
                .report
                .diagnostics
                .iter()
                .filter(|d| d.code == DiagnosticCode::IntegrityMismatch)
                .count(),
            2
        );
    }
    let mut corrupt = note.clone();
    let offset = corrupt.len() - 33;
    corrupt[offset] ^= 1;
    let report = verify(&corrupt, &page, &manifest).integrity.unwrap();
    assert_eq!(report.note.mismatched, 1);
    assert_eq!(report.manifest, matches(2));
    let mut corrupt = manifest.clone();
    corrupt[0] ^= 1;
    assert_eq!(
        verify(&note, &page, &corrupt)
            .integrity
            .unwrap()
            .manifest
            .mismatched,
        1
    );
}

#[test]
fn geometry_changes_do_not_turn_identity_hashes_into_payload_authentication() {
    let note = note();
    let mut page = page();
    let stored = sdocx::parse_stored_page_bytes(&page).unwrap();
    let object = &stored.layers.layers[0].objects[0];
    let bbox_offset = object.payload_offset + 12 + 4 + 2 + "parent".len() + 8;
    page[bbox_offset..bbox_offset + 8].copy_from_slice(&1.25_f64.to_le_bytes());
    assert_eq!(
        verify(&note, &page, &manifest(&note, &[("page", hash(PAGE_HASH))])).integrity,
        Some(golden_report())
    );
}

#[test]
fn unreadable_object_or_layer_metadata_is_unavailable_and_keeps_parent_links_checkable() {
    let note = note();
    let page = page();
    let manifest = manifest(&note, &[("page", hash(PAGE_HASH))]);
    let stored = sdocx::parse_stored_page_bytes(&page).unwrap();
    let mut corrupt = page.clone();
    let object = &stored.layers.layers[0].objects[0];
    corrupt[object.payload_offset + 4..object.payload_offset + 6]
        .copy_from_slice(&99_i16.to_le_bytes());
    let report = verify(&note, &corrupt, &manifest).integrity.unwrap();
    assert_eq!(
        report.objects,
        IntegrityCounts {
            matched: 3,
            unavailable: 1,
            mismatched: 0
        }
    );
    assert_eq!(report.layers, matches(2));
    let mut corrupt = page.clone();
    corrupt[stored.layers.layers[0].header_offset + 11] = 0;
    let report = verify(&note, &corrupt, &manifest).integrity.unwrap();
    assert_eq!(
        report.layers,
        IntegrityCounts {
            matched: 1,
            unavailable: 1,
            mismatched: 0
        }
    );
    assert_eq!(report.pages, matches(1));
}

#[test]
fn truncated_or_displaced_page_footers_are_not_accepted_as_hashes() {
    let note = note();
    let page = page();
    let manifest = manifest(&note, &[("page", hash(PAGE_HASH))]);
    let stored = sdocx::parse_stored_page_bytes(&page).unwrap();
    for length in stored.integrity_offset..page.len() {
        let report = verify(&note, &page[..length], &manifest).integrity.unwrap();
        assert_eq!(report.pages.unavailable, 1);
        assert_eq!(report.manifest.unavailable, 1);
    }
    let mut corrupt = page.clone();
    corrupt.extend([0; 4]);
    assert_eq!(
        verify(&note, &corrupt, &manifest)
            .integrity
            .unwrap()
            .pages
            .unavailable,
        1
    );
    let mut corrupt = page.clone();
    corrupt[stored.integrity_offset + 32] ^= 1;
    assert_eq!(
        verify(&note, &corrupt, &manifest)
            .integrity
            .unwrap()
            .pages
            .unavailable,
        1
    );
}

#[test]
fn note_flexible_fields_precede_the_hash_and_missing_trailers_are_unavailable() {
    let note = note();
    let manifest = manifest(&note, &[("page", hash(PAGE_HASH))]);
    let parsed = sdocx::parse_note_bytes(&note).unwrap();
    assert_eq!(parsed.header.integrity_offset as usize, note.len() - 36);
    let report = verify(&note[..note.len() - 32], &page(), &manifest)
        .integrity
        .unwrap();
    assert_eq!(report.note.unavailable, 1);
    assert_eq!(report.manifest.unavailable, 1);
    let mut invalid = note.clone();
    invalid[..4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        verify(&invalid, &page(), &manifest)
            .integrity
            .unwrap()
            .note
            .unavailable,
        1
    );
}

#[test]
fn missing_note_or_manifest_pages_are_reported_as_unavailable_links() {
    let note = note();
    let manifest = manifest(&note, &[("missing", [0; 32]), ("page", hash(PAGE_HASH))]);
    let parsed = verify_archive(&archive(&[
        ("page.page", &page()),
        ("pageIdInfo.dat", &manifest),
    ]));
    let report = parsed.integrity.unwrap();
    assert_eq!(report.note, matches(0));
    assert_eq!(
        report.manifest,
        IntegrityCounts {
            matched: 1,
            mismatched: 0,
            unavailable: 2
        }
    );
}

#[test]
fn duplicate_manifest_ids_pair_with_physical_pages_in_the_existing_order() {
    let note = note();
    let first = page();
    let mut second = page();
    let stored = sdocx::parse_stored_page_bytes(&second).unwrap();
    let time_offset = stored.header.raw_layer_offset as usize - 16;
    second[time_offset..time_offset + 8].copy_from_slice(&31_i64.to_le_bytes());
    let mut digest = Sha256::new();
    digest.update(hash(LAYER_A_HASH));
    digest.update(hash(LAYER_B_HASH));
    digest.update(Sha256::digest(b"page31"));
    let second_hash: [u8; 32] = digest.finalize().into();
    second[stored.integrity_offset..stored.integrity_offset + 32].copy_from_slice(&second_hash);
    let manifest = manifest(&note, &[("page", hash(PAGE_HASH)), ("page", second_hash)]);
    let parsed = verify_archive(&archive(&[
        ("z.page", &second),
        ("note.note", &note),
        ("a.page", &first),
        ("pageIdInfo.dat", &manifest),
    ]));
    let report = parsed.integrity.unwrap();
    assert_eq!(report.pages, matches(2));
    assert_eq!(report.manifest, matches(3));
}

#[test]
fn integrity_checks_preserve_configured_metadata_limits() {
    let mut options = ParseOptions {
        verify_integrity: true,
        ..Default::default()
    };
    options.limits.max_text_characters = 2;
    assert!(matches!(
        sdocx::parse_bytes_detailed_with_options(&archive(&[("page.page", &page())]), &options),
        Err(sdocx::Error::LimitExceeded {
            resource: "text characters",
            ..
        })
    ));
}
