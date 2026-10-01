use std::borrow::Cow;

use rustybuzz::ttf_parser::{FaceParsingError, RawFace};
use write_fonts::{FontBuilder, types::Tag};

pub(super) fn standalone_face(data: &[u8], index: u32) -> Result<Cow<'_, [u8]>, FaceParsingError> {
    let face = RawFace::parse(data, index)?;
    if !data.starts_with(b"ttcf") {
        return Ok(Cow::Borrowed(data));
    }
    if face.table_records.len() > 4095 {
        return Err(FaceParsingError::MalformedFont);
    }

    let mut builder = FontBuilder::new();
    let mut previous_tag = None;
    let mut ranges = Vec::with_capacity(usize::from(face.table_records.len()));
    let mut rebuilt_length = 12_u64 + u64::from(face.table_records.len()) * 16;
    for record in face.table_records {
        if previous_tag.is_some_and(|previous| previous >= record.tag) {
            return Err(FaceParsingError::MalformedFont);
        }
        previous_tag = Some(record.tag);
        let table = face
            .table(record.tag)
            .ok_or(FaceParsingError::MalformedFont)?;
        let start = usize::try_from(record.offset).map_err(|_| FaceParsingError::MalformedFont)?;
        if !table.is_empty() {
            ranges.push(start..start + table.len());
        }
        if record.tag.to_bytes() != *b"DSIG" {
            rebuilt_length += u64::from(record.length).next_multiple_of(4);
            if rebuilt_length > u64::from(u32::MAX) {
                return Err(FaceParsingError::MalformedFont);
            }
            builder.add_raw(Tag::new(&record.tag.to_bytes()), table);
        }
    }
    ranges.sort_unstable_by_key(|range| range.start);
    if ranges.windows(2).any(|pair| pair[0].end > pair[1].start) {
        return Err(FaceParsingError::MalformedFont);
    }
    Ok(Cow::Owned(builder.build()))
}

#[cfg(test)]
mod tests {
    use super::*;

    mod fixtures {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/helpers/font_collection.rs"
        ));
    }

    const REGULAR: &[u8] = include_bytes!("../../../assets/fonts/Roboto-Regular.ttf");
    const MONO: &[u8] = include_bytes!("../../../assets/fonts/RobotoMono-Regular.ttf");

    fn checksum(bytes: &[u8]) -> u32 {
        bytes.chunks(4).fold(0_u32, |sum, chunk| {
            let mut word = [0; 4];
            word[..chunk.len()].copy_from_slice(chunk);
            sum.wrapping_add(u32::from_be_bytes(word))
        })
    }

    #[test]
    fn extracted_face_preserves_all_tables_and_repairs_checksums() {
        let collection = fixtures::collection(&[REGULAR, MONO]);
        for (index, original) in [REGULAR, MONO].into_iter().enumerate() {
            let extracted = standalone_face(&collection, index as u32).unwrap();
            assert_eq!(checksum(&extracted), 0xb1b0_afba);
            let source = RawFace::parse(original, 0).unwrap();
            let output = RawFace::parse(&extracted, 0).unwrap();
            for record in source.table_records {
                let tag = record.tag.to_bytes();
                if tag == *b"DSIG" {
                    assert!(output.table(record.tag).is_none());
                    continue;
                }
                let original_table = source.table(record.tag).unwrap();
                let output_table = output.table(record.tag).unwrap();
                if tag == *b"head" {
                    assert_eq!(&original_table[..8], &output_table[..8]);
                    assert_eq!(&original_table[12..], &output_table[12..]);
                } else {
                    assert_eq!(original_table, output_table, "table {tag:?}");
                }
            }
            for record in output.table_records {
                let mut table = output.table(record.tag).unwrap().to_vec();
                if record.tag.to_bytes() == *b"head" {
                    table[8..12].fill(0);
                }
                assert_eq!(checksum(&table), record.check_sum);
            }
        }
    }

    #[test]
    fn standalone_faces_are_borrowed_and_invalid_indices_are_rejected() {
        assert!(matches!(
            standalone_face(REGULAR, 0).unwrap(),
            Cow::Borrowed(_)
        ));
        assert!(standalone_face(REGULAR, 1).is_err());
        assert!(standalone_face(&fixtures::collection(&[REGULAR, MONO]), 2).is_err());
        assert!(standalone_face(b"ttcf", 0).is_err());
    }

    #[test]
    fn collection_extraction_preserves_cff_flavor_and_variation_tables() {
        for outlines in [*b"CFF ", *b"CFF2"] {
            let mut builder = FontBuilder::new();
            let tables = [
                (outlines, &[1, 0, 4, 4][..]),
                (*b"fvar", &[1, 0, 0, 0][..]),
                (*b"avar", &[1, 0, 0, 1][..]),
                (*b"STAT", &[1, 1, 0, 0][..]),
                (*b"xtra", &[7, 8, 9][..]),
            ];
            for (tag, bytes) in tables {
                builder.add_raw(Tag::new(&tag), bytes);
            }
            let original = builder.build();
            let collection = fixtures::collection(&[REGULAR, &original]);
            let extracted = standalone_face(&collection, 1).unwrap();
            assert!(extracted.starts_with(b"OTTO"));
            let output = RawFace::parse(&extracted, 0).unwrap();
            for (tag, bytes) in tables {
                assert_eq!(
                    output.table(rustybuzz::ttf_parser::Tag::from_bytes(&tag)),
                    Some(bytes)
                );
            }
        }
    }
}
