#[allow(dead_code)]
mod support;

use std::io::{Cursor, Read, Write};

use sdocx::ArchiveResourceError;
#[cfg(feature = "serde")]
use sdocx::Document;

const IMAGE_SOURCE: &[u8] = b"\x89PNG\r\n\x1a\n\0source\xff";
const OPAQUE_SOURCE: &[u8] = b"GIF89a\0\xff\xfe\x80retained";

fn archive() -> Vec<u8> {
    let page = support::page(&[Vec::new()], 0, &[]);
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in [
        ("page.page", page.as_slice()),
        ("media/12@image.png", IMAGE_SOURCE),
        ("media/13@opaque.gif", OPAQUE_SOURCE),
    ] {
        writer
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

#[test]
fn image_source_lookup_borrows_the_existing_asset_owner() {
    let parsed = sdocx::parse_bytes_detailed(&archive()).unwrap();
    let metadata = &parsed.document.metadata;
    assert_eq!(metadata.media_assets.len(), 1);
    let image = metadata.resolve_archive_resource(12).unwrap();
    assert!(image.inferred);
    assert_eq!(image.data, IMAGE_SOURCE);
    assert_eq!(image.data.as_ptr(), metadata.media_assets[0].data.as_ptr());
    assert_eq!(
        metadata.resolve_archive_resource(13).unwrap().data,
        OPAQUE_SOURCE
    );

    let document = parsed.into_document();
    assert_eq!(
        document.metadata.resolve_archive_resource(13).unwrap().data,
        OPAQUE_SOURCE
    );
}

#[test]
fn caller_mutation_cannot_redirect_a_resource_to_another_images_bytes() {
    let mut document = sdocx::parse_bytes(&archive()).unwrap();
    document.metadata.media_assets[0].name = "media/99@replacement.png".into();
    let resource = document
        .metadata
        .archive_resources
        .iter()
        .find(|resource| resource.name == "media/12@image.png")
        .unwrap();
    assert!(resource.data(&document.metadata.media_assets).is_none());
    assert!(matches!(
        document.metadata.resolve_archive_resource(12),
        Err(ArchiveResourceError::DataUnavailable { name })
            if name == "media/12@image.png"
    ));
    assert_eq!(
        document.metadata.resolve_archive_resource(13).unwrap().data,
        OPAQUE_SOURCE
    );
}

#[test]
fn understated_zip_sizes_cannot_bypass_the_resource_memory_budget() {
    let name = "media/budget.bin";
    let mut writer = zip::ZipWriter::new_append(Cursor::new(archive())).unwrap();
    writer
        .start_file(
            name,
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated),
        )
        .unwrap();
    writer.write_all(&[0_u8; 512]).unwrap();
    let mut bytes = writer.finish().unwrap().into_inner();
    assert!(sdocx::parse_bytes(&bytes).is_ok());
    let header = bytes
        .windows(46 + name.len())
        .position(|window| &window[..4] == b"PK\x01\x02" && &window[46..] == name.as_bytes())
        .unwrap();
    bytes[header + 24..header + 28].copy_from_slice(&1_u32.to_le_bytes());
    let mut archive = zip::ZipArchive::new(Cursor::new(&bytes)).unwrap();
    let advertised_total = u64::try_from(archive.decompressed_size().unwrap()).unwrap();
    assert!(advertised_total < 512);
    let mut entry = archive.by_name(name).unwrap();
    assert_eq!(entry.size(), 1);
    let mut actual = Vec::new();
    entry.read_to_end(&mut actual).unwrap();
    assert_eq!(actual, [0_u8; 512]);
    let options = sdocx::ParseOptions {
        limits: sdocx::ParseLimits {
            max_total_uncompressed_size: advertised_total,
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(sdocx::parse_bytes_with_options(&bytes, &options).is_err());
}

#[cfg(feature = "serde")]
#[test]
fn documents_serialized_before_source_retention_still_deserialize() {
    let old = serde_json::json!({
        "pages": [],
        "metadata": {
            "page_ids": [],
            "media_assets": [{
                "name": "media/12@image.png",
                "archive_id": 12,
                "mime_type": "image/png",
                "data": IMAGE_SOURCE,
            }],
        },
    });
    let document: Document = serde_json::from_value(old).unwrap();
    assert!(document.metadata.media_manifest.is_none());
    assert!(document.metadata.archive_resources.is_empty());
    assert_eq!(document.metadata.media_assets[0].data, IMAGE_SOURCE);
}

#[cfg(feature = "serde")]
#[test]
fn source_retention_roundtrips_without_serializing_image_bytes_twice() {
    let document = sdocx::parse_bytes(&archive()).unwrap();
    let encoded = serde_json::to_value(&document).unwrap();
    assert_eq!(byte_array_occurrences(&encoded, IMAGE_SOURCE), 1);
    assert_eq!(byte_array_occurrences(&encoded, OPAQUE_SOURCE), 1);

    let restored: Document = serde_json::from_value(encoded).unwrap();
    assert_eq!(restored.metadata.media_assets.len(), 1);
    let image = restored.metadata.resolve_archive_resource(12).unwrap();
    assert_eq!(image.data, IMAGE_SOURCE);
    assert_eq!(
        image.data.as_ptr(),
        restored.metadata.media_assets[0].data.as_ptr()
    );
    assert_eq!(
        restored.metadata.resolve_archive_resource(13).unwrap().data,
        OPAQUE_SOURCE
    );
}

#[cfg(feature = "serde")]
fn byte_array_occurrences(value: &serde_json::Value, bytes: &[u8]) -> usize {
    let matches = usize::from(value == &serde_json::json!(bytes));
    matches
        + match value {
            serde_json::Value::Array(values) => values
                .iter()
                .map(|value| byte_array_occurrences(value, bytes))
                .sum(),
            serde_json::Value::Object(fields) => fields
                .values()
                .map(|value| byte_array_occurrences(value, bytes))
                .sum(),
            _ => 0,
        }
}
