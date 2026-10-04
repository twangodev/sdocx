use super::*;
use serde_json::{Value, json};
use std::io::{Cursor, Write};

fn utf16(bytes: &mut Vec<u8>, text: &str) {
    bytes.extend_from_slice(&(text.encode_utf16().count() as u16).to_le_bytes());
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
}

fn archive(source_length: usize) -> Vec<u8> {
    let mut media = 5500_u32.to_le_bytes().to_vec();
    media.extend_from_slice(&2_u16.to_le_bytes());
    for (id, name) in [(91_u32, "7@source.pdf"), (4, "8@animation.gif")] {
        let mut record = id.to_le_bytes().to_vec();
        utf16(&mut record, name);
        record.extend_from_slice("a".repeat(64).as_bytes());
        record.extend_from_slice(&3_u16.to_le_bytes());
        record.extend_from_slice(&i64::MAX.to_le_bytes());
        record.push(1);
        record.extend_from_slice(&[231; 257]);
        media.extend_from_slice(&(record.len() as u32).to_le_bytes());
        media.extend(record);
    }
    media.extend_from_slice(b"EOFX");
    media.extend_from_slice(&[232; 513]);
    let mut pages = vec![0; 32];
    pages.extend_from_slice(&1_u16.to_le_bytes());
    utf16(&mut pages, "page");
    pages.extend_from_slice(&[204; 32]);
    pages.extend_from_slice(&[233; 769]);
    let page = crate::debugger::support::page(&[Vec::new()], 0, &[]);
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in [
        ("page.page", page),
        ("media/7@source.pdf", vec![234; source_length]),
        ("media/8@animation.gif", vec![235; 31]),
        ("media/mediaInfo.dat", media),
        ("pageIdInfo.dat", pages),
    ] {
        writer
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn assert_no_source_bytes(value: &Value) {
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                assert!(!matches!(key.as_str(), "data" | "trailing_data"));
                assert_no_source_bytes(value);
            }
        }
        Value::Array(values) => values.iter().for_each(assert_no_source_bytes),
        _ => {}
    }
}

#[test]
fn source_inspection_preserves_metadata_without_transporting_source_bytes() {
    let bytes = archive(2 * 1024 * 1024);
    let parsed = sdocx::parse_bytes_detailed(&bytes).unwrap();
    let layout = sdocx::layout_document(&parsed.document);
    let inspection = serde_json::to_value(crate::inspection_data(&parsed, &layout)).unwrap();
    let metadata = &inspection["document"]["metadata"];
    assert_eq!(metadata["media_assets"], json!([]));
    let manifest = &metadata["media_manifest"];
    assert_eq!(manifest["entries"][0]["bind_id"], 91);
    assert_eq!(manifest["entries"][1]["bind_id"], 4);
    assert_eq!(manifest["entries"][0]["file_name"], "7@source.pdf");
    assert_eq!(manifest["entries"][0]["sha256"], "a".repeat(64));
    assert_eq!(manifest["entries"][0]["reference_count"], 3);
    assert_eq!(
        manifest["entries"][0]["modified_time_raw"],
        i64::MAX.to_string()
    );
    assert_eq!(manifest["entries"][0]["is_attached"], true);
    assert_eq!(manifest["entries"][0]["trailing_byte_length"], 257);
    assert_eq!(manifest["trailing_byte_length"], 513);
    assert_eq!(inspection["page_manifest"]["trailing_byte_length"], 769);
    let resources = metadata["archive_resources"].as_array().unwrap();
    assert_eq!(resources.len(), 3);
    let pdf = resources
        .iter()
        .find(|source| source["name"] == "media/7@source.pdf")
        .unwrap();
    assert_eq!(pdf["archive_id"], 7);
    assert_eq!(pdf["kind"], "opaque");
    assert_eq!(pdf["media_index"], Value::Null);
    assert_eq!(pdf["byte_length"], 2 * 1024 * 1024);
    assert_eq!(pdf["data_available"], true);
    assert_eq!(
        parsed
            .document
            .metadata
            .resolve_archive_resource(91)
            .unwrap()
            .data
            .len(),
        2 * 1024 * 1024
    );
    assert_no_source_bytes(metadata);
    assert_no_source_bytes(&inspection["page_manifest"]);
    assert!(serde_json::to_vec(&inspection).unwrap().len() < 10_000);

    let mut source = crate::debugger::Source::new(&bytes).unwrap();
    let debug: Value = serde_json::from_str(
        &source
            .request(&parsed, &layout, r#"{"kind":"document"}"#)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(debug["metadata"]["media_manifest"], *manifest);
    assert_eq!(
        debug["metadata"]["archive_resources"],
        metadata["archive_resources"]
    );
    assert_eq!(debug["manifest"], inspection["page_manifest"]);
    for (entry, expected) in [(3, manifest), (4, &inspection["page_manifest"])] {
        let decoded: Value = serde_json::from_str(
            &source
                .request(
                    &parsed,
                    &layout,
                    &json!({"kind":"entry", "entry":entry}).to_string(),
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(&decoded, expected);
        assert_no_source_bytes(&decoded);
    }
    let raw: Value = serde_json::from_str(
        &source
            .request(
                &parsed,
                &layout,
                r#"{"kind":"bytes","entry":1,"offset":0,"length":3}"#,
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(raw["bytes"], json!([234, 234, 234]));

    let small = sdocx::parse_bytes_detailed(&archive(1)).unwrap();
    let small_layout = sdocx::layout_document(&small.document);
    let small_json = serde_json::to_vec(&crate::inspection_data(&small, &small_layout)).unwrap();
    assert!(
        serde_json::to_vec(&inspection)
            .unwrap()
            .len()
            .abs_diff(small_json.len())
            < 32
    );
}

#[test]
fn source_inspection_rejects_stale_image_links_and_keeps_legacy_defaults() {
    let metadata = sdocx::DocumentMetadata {
    media_assets: serde_json::from_value(json!([{"name":"media/right.png", "archive_id":7, "mime_type":"image/png", "data":[97,98,99]}])).unwrap(),
    archive_resources: serde_json::from_value(json!([
        {"name":"media/right.png", "archive_id":7, "content":{"MediaAsset":{"media_index":0}}},
        {"name":"media/wrong.png", "archive_id":8, "content":{"MediaAsset":{"media_index":0}}},
        {"name":"media/missing.png", "archive_id":9, "content":{"MediaAsset":{"media_index":1}}}
    ]))
    .unwrap(),
    ..Default::default()
    };
    let summary = serde_json::to_value(archive_resources(&metadata)).unwrap();
    assert_eq!(summary[0]["kind"], "media_asset");
    assert_eq!(summary[0]["media_index"], 0);
    assert_eq!(summary[0]["byte_length"], 3);
    for index in [1, 2] {
        assert_eq!(summary[index]["data_available"], false);
        assert_eq!(summary[index]["byte_length"], Value::Null);
    }
    assert_no_source_bytes(&summary);
    let bytes =
        crate::debugger::support::archive(&crate::debugger::support::page(&[Vec::new()], 0, &[]));
    let parsed = sdocx::parse_bytes_detailed(&bytes).unwrap();
    let layout = sdocx::layout_document(&parsed.document);
    let inspection = serde_json::to_value(crate::inspection_data(&parsed, &layout)).unwrap();
    assert_eq!(
        inspection["document"]["metadata"]["media_manifest"],
        Value::Null
    );
    assert_eq!(
        inspection["document"]["metadata"]["archive_resources"],
        json!([])
    );
    assert_eq!(inspection["document"]["metadata"]["page_ids"], json!([]));
    let mut parsed = parsed;
    parsed.document.metadata.media_assets = metadata.media_assets;
    parsed.document.metadata.archive_resources = metadata.archive_resources;
    parsed.document.metadata.media_manifest = Some(serde_json::from_value(json!({
        "format_version":5500,
        "entries":[{"bind_id":7,"file_name":"right.png","sha256":"a".repeat(64),"reference_count":1,"modified_time_raw":0,"is_attached":false,"trailing_data":[]}],
        "trailing_data":[]
    })).unwrap());
    let layout = sdocx::layout_document(&parsed.document);
    let inspection = serde_json::to_value(crate::inspection_data(&parsed, &layout)).unwrap();
    let metadata = &inspection["document"]["metadata"];
    assert_eq!(
        metadata["media_assets"][0]["sha256"],
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        metadata["media_manifest"]["entries"][0]["sha256"],
        "a".repeat(64)
    );
    assert_eq!(metadata["media_assets"].as_array().unwrap().len(), 1);
    assert_no_source_bytes(metadata);
}
