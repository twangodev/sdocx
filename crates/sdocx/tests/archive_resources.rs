#[allow(dead_code)]
mod support;

use std::io::{Cursor, Write};

use sdocx::{ArchiveResourceError, Error, ParseLimits, ParseOptions};

fn manifest(entries: &[(u32, &str)]) -> Vec<u8> {
    let mut bytes = 5500_u32.to_le_bytes().to_vec();
    bytes.extend((entries.len() as u16).to_le_bytes());
    for (id, name) in entries {
        let mut record = id.to_le_bytes().to_vec();
        record.extend((name.encode_utf16().count() as u16).to_le_bytes());
        record.extend(name.encode_utf16().flat_map(u16::to_le_bytes));
        record.extend([b'a'; 64]);
        record.extend(3_u16.to_le_bytes());
        record.extend((-42_i64).to_le_bytes());
        record.extend([1, 12, 13]);
        bytes.extend((record.len() as u32).to_le_bytes());
        bytes.extend(record);
    }
    bytes.extend(b"EOFX");
    bytes.extend([14, 15]);
    bytes
}

fn archive(files: &[(&str, &[u8])]) -> Vec<u8> {
    let page = support::page(&[Vec::new()], 0, &[]);
    let mut writer = zip::ZipWriter::new_append(Cursor::new(support::archive(&page))).unwrap();
    writer
        .add_directory("media/", zip::write::SimpleFileOptions::default())
        .unwrap();
    for (name, data) in files {
        writer
            .start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(data).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

#[test]
fn manifest_authority_and_opaque_source_bytes_survive_ordinary_parsing() {
    let descriptor = manifest(&[
        (42, "9@paper.pdf"),
        (3, "animation.gif"),
        (7, "font.e1"),
        (11, "canvas.spi"),
        (0, "nested/value.bin"),
        (100, "2@image.png"),
    ]);
    let pdf = b"%PDF-1.7\0\xff\x80source";
    let gif = b"GIF89a\0\xff";
    let font = b"opaque E1 source\0\x81";
    let spi = b"opaque SPI source\0\x82";
    let unknown = b"future extension\0\x83";
    let png = b"source PNG bytes";
    let bytes = archive(&[
        ("media/9@paper.pdf", pdf),
        ("media/animation.gif", gif),
        ("media/font.e1", font),
        ("media/canvas.spi", spi),
        ("media/nested/value.bin", unknown),
        ("media/2@image.png", png),
        ("media/mediaInfo.dat", &descriptor),
        ("unrelated.bin", b"outside the media source contract"),
    ]);
    let document = sdocx::parse_bytes(&bytes).unwrap();
    let metadata = &document.metadata;
    assert_eq!(metadata.media_assets.len(), 1);
    assert_eq!(metadata.media_assets[0].name, "media/2@image.png");
    assert_eq!(metadata.archive_resources.len(), 7);
    let retained = metadata.media_manifest.as_ref().unwrap();
    assert_eq!(retained.format_version, 5500);
    assert_eq!(retained.entries[0].bind_id, 42);
    assert_eq!(retained.entries[0].file_name, "9@paper.pdf");
    assert_eq!(
        retained.entries[0].sha256.as_deref(),
        Some("a".repeat(64).as_str())
    );
    assert_eq!(retained.entries[0].reference_count, 3);
    assert_eq!(retained.entries[0].modified_time_raw, -42);
    assert!(retained.entries[0].is_attached);
    assert_eq!(retained.entries[0].trailing_data, [12, 13]);
    assert_eq!(retained.trailing_data, [14, 15]);
    let resolver = metadata.archive_resource_resolver();
    for (id, data) in [
        (42, pdf.as_slice()),
        (3, gif),
        (7, font),
        (11, spi),
        (0, unknown),
        (100, png),
    ] {
        let resolved = resolver.resolve(id).unwrap();
        assert_eq!(resolved.data, data);
        assert!(!resolved.inferred);
    }
    assert_eq!(resolver.resolve(42).unwrap().resource.archive_id, Some(9));
    assert!(matches!(
        resolver.resolve(9),
        Err(ArchiveResourceError::UnboundId { id: 9 })
    ));
    let raw_descriptor = metadata
        .archive_resources
        .iter()
        .find(|source| source.name == "media/mediaInfo.dat")
        .unwrap();
    assert_eq!(
        raw_descriptor.data(&metadata.media_assets),
        Some(descriptor.as_slice())
    );
}

#[test]
fn source_lookup_preserves_missing_duplicate_and_unbound_manifest_references() {
    let descriptor = manifest(&[
        (4, "absent.pdf"),
        (7, "paper.pdf"),
        (7, "image.png"),
        (5, "paper.pdf"),
    ]);
    let parsed = sdocx::parse_bytes_detailed(&archive(&[
        ("media/mediaInfo.dat", &descriptor),
        ("media/paper.pdf", b"source PDF"),
        ("media/image.png", b"source image"),
        ("media/8@future.bin", b"source future"),
    ]))
    .unwrap();
    let resolver = parsed.document.metadata.archive_resource_resolver();
    assert!(
        matches!(resolver.resolve(4), Err(ArchiveResourceError::MissingEntry { id: 4, name }) if name == "media/absent.pdf")
    );
    assert!(matches!(
        resolver.resolve(7),
        Err(ArchiveResourceError::AmbiguousBinding { id: 7 })
    ));
    assert!(matches!(
        resolver.resolve(8),
        Err(ArchiveResourceError::UnboundId { id: 8 })
    ));
    assert_eq!(resolver.resolve(5).unwrap().data, b"source PDF");
    assert_eq!(parsed.document.metadata.media_assets.len(), 1);
}

#[test]
fn filename_inference_accounts_for_opaque_sources_without_admitting_images() {
    let parsed = sdocx::parse_bytes_detailed(&archive(&[
        ("media/4@paper.pdf", b"PDF"),
        ("media/7@image.png", b"PNG"),
        ("media/7@future.bin", b"opaque"),
        ("media/no-prefix.gif", b"GIF"),
    ]))
    .unwrap();
    let metadata = &parsed.document.metadata;
    assert!(metadata.media_manifest.is_none());
    assert_eq!(metadata.media_assets.len(), 1);
    let source = metadata.resolve_archive_resource(4).unwrap();
    assert_eq!(source.data, b"PDF");
    assert!(source.inferred);
    assert!(matches!(
        metadata.resolve_archive_resource(7),
        Err(ArchiveResourceError::AmbiguousBinding { id: 7 })
    ));
    assert!(
        metadata
            .archive_resources
            .iter()
            .all(|source| source.name != "media/")
    );
}

#[test]
fn mutated_duplicate_source_names_are_explicitly_ambiguous() {
    let mut document = sdocx::parse_bytes(&archive(&[("media/4@paper.pdf", b"source")])).unwrap();
    let resource = document.metadata.archive_resources[0].clone();
    document.metadata.archive_resources.push(resource);
    assert!(
        matches!(document.metadata.resolve_archive_resource(4), Err(ArchiveResourceError::AmbiguousEntry { name }) if name == "media/4@paper.pdf")
    );
}

#[test]
fn retained_opaque_entries_obey_existing_archive_size_limits() {
    let opaque = vec![0_u8; 4096];
    let bytes = archive(&[("media/large.pdf", &opaque)]);
    let options = ParseOptions {
        limits: ParseLimits {
            max_entry_size: 4095,
            ..ParseLimits::default()
        },
        ..ParseOptions::default()
    };
    assert!(matches!(
        sdocx::parse_bytes_with_options(&bytes, &options),
        Err(Error::LimitExceeded {
            resource: "archive entry size",
            actual: 4096,
            ..
        })
    ));
    let archive = zip::ZipArchive::new(Cursor::new(&bytes)).unwrap();
    let total = u64::try_from(archive.decompressed_size().unwrap()).unwrap();
    let options = ParseOptions {
        limits: ParseLimits {
            max_total_uncompressed_size: total - 1,
            ..ParseLimits::default()
        },
        ..ParseOptions::default()
    };
    assert!(matches!(
        sdocx::parse_bytes_with_options(&bytes, &options),
        Err(Error::LimitExceeded {
            resource: "total uncompressed size",
            ..
        })
    ));
    let options = ParseOptions {
        limits: ParseLimits {
            max_entry_size: 4096,
            max_total_uncompressed_size: total,
            ..ParseLimits::default()
        },
        ..ParseOptions::default()
    };
    let document = sdocx::parse_bytes_with_options(&bytes, &options).unwrap();
    let source = &document.metadata.archive_resources[0];
    assert_eq!(
        source.data(&document.metadata.media_assets),
        Some(opaque.as_slice())
    );
}
