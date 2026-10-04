#[allow(dead_code)]
mod support;

use sdocx::{PageTemplateSource, PdfPaperRectangle};

fn parse(fields: &[u8], mask: u32, version: Option<u32>) -> sdocx::Document {
    let mut page = support::page(&[vec![]], mask, fields);
    let property_offset = u32::from_le_bytes(page[4..8].try_into().unwrap()) as usize;
    if let Some(version) = version {
        page[property_offset - 8..property_offset - 4].copy_from_slice(&version.to_le_bytes());
    } else {
        page.drain(property_offset - 8..property_offset);
        for offset in [0, 4] {
            let value = u32::from_le_bytes(page[offset..offset + 4].try_into().unwrap()) - 8;
            page[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
    }
    sdocx::parse_bytes(&support::archive(&page)).unwrap()
}

fn records_fields(records: &[[i32; 6]]) -> Vec<u8> {
    let mut fields = u16::try_from(records.len()).unwrap().to_le_bytes().to_vec();
    for record in records {
        for value in record {
            fields.extend_from_slice(&value.to_le_bytes());
        }
    }
    fields
}

#[test]
fn continuous_pdf_source_preserves_all_twenty_placements_and_independent_template_type() {
    // cs61bl_su22.sdocx SHA-256 fa2d3ba44023871c6a53436e810772f7b4f45b190dd28e05c172886b8f7e40a0:
    // field 8 at .page offset 0xac, format 4000; these are its frozen placement boundaries.
    let boundaries = [
        0, 1397, 2794, 4192, 5590, 6987, 8385, 9783, 11180, 12578, 13976, 15373, 16771, 18169,
        19566, 20964, 22362, 23760, 25157, 26554, 27952,
    ];
    let records: Vec<_> = boundaries
        .windows(2)
        .enumerate()
        .map(|(index, y)| [0, index as i32, 0, y[0], 1080, y[1]])
        .collect();
    let mut fields = records_fields(&records);
    fields.extend_from_slice(&11_u32.to_le_bytes());
    let document = parse(&fields, (1 << 8) | (1 << 9), Some(4000));
    let page = &document.pages[0];
    assert_eq!(page.background.template_type, Some(11));
    assert_eq!(
        page.template.unwrap().source,
        PageTemplateSource::CustomPdf { page_index: 0 }
    );
    let paper = page.background.pdf_paper.as_ref().unwrap();
    assert_eq!(paper.len(), 20);
    for (record, expected) in paper.iter().zip(&records) {
        assert_eq!(record.media_id, expected[0]);
        assert_eq!(record.page_index, expected[1]);
        assert_eq!(
            record.rectangle,
            PdfPaperRectangle::Integer(expected[2..].try_into().unwrap())
        );
    }
    let layout = sdocx::layout_document(&document);
    assert_eq!(layout.pages[0].page.background, page.background);
    #[cfg(feature = "serde")]
    {
        let json = serde_json::to_string(&layout).unwrap();
        let restored: sdocx::LayoutDocument = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.pages[0].page.background, page.background);
    }
}

#[test]
fn absent_empty_and_signed_duplicate_records_remain_distinct() {
    let absent = parse(&[], 0, Some(4000));
    let empty = parse(&[0, 0, 0, 0, 0, 0], (1 << 8) | (1 << 9), Some(4000));
    assert_eq!(absent.pages[0].background.pdf_paper, None);
    assert_eq!(absent.pages[0].background.template_type, None);
    assert_eq!(empty.pages[0].background.pdf_paper, Some(vec![]));
    assert_eq!(empty.pages[0].background.template_type, Some(0));
    assert_eq!(empty.pages[0].template, None);
    assert_ne!(absent.pages[0].background, empty.pages[0].background);

    let records = [
        [-1000, -1, i32::MIN, 3, -4, i32::MAX],
        [-1000, -1, i32::MIN, 3, -4, i32::MAX],
        [-1, 7, 10, 20, 5, 4],
    ];
    let document = parse(&records_fields(&records), 1 << 8, Some(2034));
    let page = &document.pages[0];
    let paper = page.background.pdf_paper.as_ref().unwrap();
    assert_eq!(paper.len(), 3);
    assert_eq!(paper[0], paper[1]);
    assert_eq!(paper[0].media_id, -1000);
    assert_eq!(paper[0].page_index, -1);
    assert_eq!(paper[2].media_id, -1);
    assert_eq!(
        paper[2].rectangle,
        PdfPaperRectangle::Integer([10, 20, 5, 4])
    );
    assert_eq!(
        page.template.unwrap().source,
        PageTemplateSource::CustomPdf {
            page_index: u32::MAX
        }
    );
}

#[test]
fn rectangle_encoding_follows_declared_page_version_without_normalizing_bits() {
    let bits = [0x80000000_u32, 0x7fc01234, 0x7f800000, 0xff800000];
    let raw = bits.map(|word| word as i32);
    let fields = records_fields(&[[-1000, -7, raw[0], raw[1], raw[2], raw[3]]]);
    let rectangle = |version| {
        parse(&fields, 1 << 8, version).pages[0]
            .background
            .pdf_paper
            .as_ref()
            .unwrap()[0]
            .rectangle
    };
    assert_eq!(
        rectangle(Some(2033)),
        PdfPaperRectangle::LegacyFloatBits(bits)
    );
    assert_eq!(rectangle(Some(2034)), PdfPaperRectangle::Integer(raw));
    let bytes: Vec<_> = bits.iter().flat_map(|word| word.to_le_bytes()).collect();
    assert_eq!(
        rectangle(None),
        PdfPaperRectangle::Unspecified(bytes.try_into().unwrap())
    );
    #[cfg(feature = "serde")]
    {
        let legacy = parse(&fields, 1 << 8, Some(2033));
        let json = serde_json::to_string(&legacy).unwrap();
        let restored: sdocx::Document = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.pages[0].background, legacy.pages[0].background);
    }
}

#[test]
fn incomplete_record_lists_and_following_template_field_are_rejected() {
    let mut fields = records_fields(&[[0, 0, 0, 0, 1080, 1397], [0, 1, 0, 1397, 1080, 2794]]);
    fields.extend_from_slice(&11_u32.to_le_bytes());
    for end in [0, 1, 2, 9, 25, 26, 49, 50, 51, 53] {
        let page = support::page(&[vec![]], (1 << 8) | (1 << 9), &fields[..end]);
        assert!(sdocx::parse_bytes(&support::archive(&page)).is_err());
    }
    let page = support::page(&[vec![]], 1 << 8, &u16::MAX.to_le_bytes());
    let error = sdocx::parse_bytes(&support::archive(&page)).unwrap_err();
    assert!(error.to_string().contains("truncated PDF records"));
}

#[test]
fn compatibility_page_omission_compares_complete_pdf_source_and_template_type() {
    let fields = records_fields(&[[0, 0, 0, 0, 1080, 1397], [0, 1, 0, 1397, 1080, 2794]]);
    let mut document = parse(&fields, 1 << 8, Some(4000));
    document.pages.push(document.pages[0].clone());
    document.metadata.page_mode = Some(0);
    document.metadata.flow_dimensions = Some((1080, 3054));
    document.metadata.flow_page_padding = Some((0, 0));
    assert!(sdocx::layout_document(&document).omitted_trailing_blank_page);
    document.pages[1].background.pdf_paper.as_mut().unwrap()[1].page_index = 2;
    let layout = sdocx::layout_document(&document);
    assert!(!layout.omitted_trailing_blank_page);
    assert_eq!(layout.pages.len(), 2);
    document.pages[1].background = document.pages[0].background.clone();
    document.pages[1].background.template_type = Some(16);
    assert!(!sdocx::layout_document(&document).omitted_trailing_blank_page);
}

#[cfg(feature = "serde")]
#[test]
fn historical_background_json_defaults_new_source_fields_to_absent() {
    let historical =
        r#"{"template_uri":null,"image_id":null,"image_mode":null,"width":null,"rotation":null}"#;
    let background: sdocx::PageBackground = serde_json::from_str(historical).unwrap();
    assert_eq!(background, sdocx::PageBackground::default());
}
