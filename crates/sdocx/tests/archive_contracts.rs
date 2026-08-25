use std::io::{Cursor, Write};

use sdocx::{Error, ParseOptions};

fn archive_with_entries(names: &[&str]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for name in names {
        writer
            .start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"test").unwrap();
    }
    writer.finish().unwrap().into_inner()
}

#[test]
fn archive_entry_limit_is_configurable() {
    let options = ParseOptions {
        limits: sdocx::ParseLimits {
            max_archive_entries: 1,
            ..sdocx::ParseLimits::default()
        },
    };

    let bytes = archive_with_entries(&["first.bin", "second.bin"]);
    let error = sdocx::parse_bytes_with_options(&bytes, &options).unwrap_err();

    assert!(matches!(
        error,
        Error::LimitExceeded {
            resource: "archive entry count",
            limit: 1,
            actual: 2,
        }
    ));
}
