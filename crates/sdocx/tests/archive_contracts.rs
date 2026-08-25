use std::path::PathBuf;

use sdocx::{Error, FormatVersion, ParseOptions};

fn sample_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("samples")
        .join(name)
}

#[test]
fn sample_exposes_format_version_and_authoritative_page_order() {
    let document = sdocx::parse(sample_path("handwritten.sdocx")).unwrap();

    assert_eq!(document.metadata.format_version, Some(FormatVersion(4000)));
    assert!(
        document
            .pages
            .iter()
            .zip(&document.metadata.page_ids)
            .all(|(page, page_id)| page.uuid == *page_id)
    );
}

#[test]
fn archive_entry_limit_is_configurable() {
    let options = ParseOptions {
        limits: sdocx::ParseLimits {
            max_archive_entries: 1,
            ..sdocx::ParseLimits::default()
        },
    };

    let error = sdocx::parse_with_options(sample_path("quiz.sdocx"), &options).unwrap_err();

    assert!(matches!(
        error,
        Error::LimitExceeded {
            resource: "archive entry count",
            limit: 1,
            actual,
        } if actual > 1
    ));
}

#[test]
fn media_filename_resource_id_is_preserved() {
    let document = sdocx::parse(sample_path("quiz.sdocx")).unwrap();
    let asset = document
        .metadata
        .media_assets
        .iter()
        .find(|asset| asset.name.ends_with("@files_230820_133807_215.png"))
        .unwrap();

    assert_eq!(asset.archive_id, Some(7));
}
