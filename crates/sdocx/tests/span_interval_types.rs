mod support;

use sdocx::{PageElement, RichTextSpan, RichTextSpanType, SpanIntervalType};

fn span(raw: u32, start: u32, end: u32) -> RichTextSpan {
    RichTextSpan {
        kind: RichTextSpanType::FontSize,
        start_utf16: start,
        end_utf16: end,
        interval_type: SpanIntervalType::from(raw),
        payload: 17.0_f32.to_le_bytes().to_vec(),
    }
}

fn frame(kind: i16, fields: &[u8], fixed: &[u8], flexible: &[u8]) -> Vec<u8> {
    let offset = 13 + fields.len() + fixed.len();
    let mut bytes = ((offset + flexible.len()) as u32).to_le_bytes().to_vec();
    bytes.extend(kind.to_le_bytes());
    bytes.extend((offset as u32).to_le_bytes());
    bytes.extend([1, u8::from(kind == 0) << 3, fields.len() as u8]);
    bytes.extend(fields);
    bytes.extend(fixed);
    bytes.extend(flexible);
    bytes
}

fn text_object(raw_intervals: &[u32]) -> Vec<u8> {
    let mut base = 5500_u32.to_le_bytes().to_vec();
    base.extend(2_u16.to_le_bytes());
    base.extend(b"tx");
    base.extend(0_i64.to_le_bytes());
    for endpoint in [0.0_f64, 0.0, 100.0, 100.0] {
        base.extend(endpoint.to_le_bytes());
    }
    base.extend([0; 5]);
    let mut common = 3_u32.to_le_bytes().to_vec();
    for unit in "ABC".encode_utf16() {
        common.extend(unit.to_le_bytes());
    }
    common.extend((raw_intervals.len() as u32).to_le_bytes());
    for &raw in raw_intervals {
        common.extend(20_u16.to_le_bytes());
        // WDoc GetBinary 0x40d040–50 stores interval type at span-header offset 12.
        for value in [3_u32, 1, 2, raw] {
            common.extend(value.to_le_bytes());
        }
        common.extend(17.0_f32.to_le_bytes());
    }
    common.extend(0_u32.to_le_bytes());
    common.extend([0; 16]);
    common.push(0);
    common.extend(0_u16.to_le_bytes());
    common.extend([0; 8]);
    let mut flexible = (common.len() as u32).to_le_bytes().to_vec();
    flexible.extend(common);
    let mut bytes = frame(0, &[], &base, &[]);
    bytes.extend(frame(6, &[], &[], &[]));
    bytes.extend(frame(7, &[1], &[], &flexible));
    bytes.extend(frame(2, &[], &[], &[]));
    bytes
}

#[test]
fn parser_retains_all_native_interval_values_without_boolean_normalization() {
    let raw = [0, 1, 2, 3, 4, 0x8000_0000, u32::MAX];
    let page = support::page(&[vec![support::object(2, &text_object(&raw), &[])]], 0, &[]);
    let parsed = sdocx::parse_bytes_detailed(&support::archive(&page)).unwrap();
    let PageElement::TextBox(text) = parsed.document.pages[0].elements().next().unwrap() else {
        panic!("expected text box");
    };
    assert_eq!(text.text, "ABC");
    assert_eq!(text.spans.len(), raw.len());
    for (decoded, raw) in text.spans.iter().zip(raw) {
        assert_eq!(decoded.kind, RichTextSpanType::FontSize);
        assert_eq!(decoded.interval_type, SpanIntervalType::from(raw));
        assert_eq!(decoded.interval_type.raw(), raw);
        assert_eq!(decoded.font_size_value(), Some(17.0));
        assert_eq!((decoded.start_utf16, decoded.end_utf16), (1, 2));
    }
}

#[test]
fn native_caret_endpoint_policies_include_the_zero_length_exceptions() {
    // Text GetFontSize 0x62104 distinguishes these four policies, with default = 3.
    for (raw, expected, empty) in [
        (0, [false, true, true, false, false], true),
        (1, [false, true, true, true, false], true),
        (2, [false, false, true, false, false], false),
        (3, [false, false, true, true, false], true),
        (4, [false, false, true, true, false], true),
        (u32::MAX, [false, false, true, true, false], true),
    ] {
        let interval = span(raw, 10, 12);
        assert_eq!(
            [9, 10, 11, 12, 13].map(|i| interval.contains_caret(i)),
            expected,
            "raw {raw}"
        );
        let interval = span(raw, 10, 10);
        assert_eq!(
            [9, 10, 11].map(|i| interval.contains_caret(i)),
            [false, empty, false],
            "empty raw {raw}"
        );
        assert!(!span(raw, 12, 10).contains_caret(11));
        assert_eq!(span(raw, 12, 10).contains_caret(12), raw == 0);
        assert_eq!(
            span(raw, 12, 10).contains_caret(10),
            matches!(raw, 3 | 4 | u32::MAX)
        );
        assert_eq!(
            span(raw, u32::MAX, u32::MAX).contains_caret(u32::MAX),
            empty
        );
    }
}

#[cfg(feature = "serde")]
#[test]
fn json_preserves_raw_intervals_and_migrates_legacy_expand_without_a_second_field() {
    for raw in [0, 1, 2, 3, 4, 0x8000_0000, u32::MAX] {
        let span = span(raw, 1, 2);
        let value = serde_json::to_value(&span).unwrap();
        assert_eq!(value["interval_type"], raw);
        assert!(value.get("expand").is_none());
        assert_eq!(serde_json::from_value::<RichTextSpan>(value).unwrap(), span);
    }
    for expand in [false, true] {
        let mut value = serde_json::to_value(span(0, 1, 2)).unwrap();
        value.as_object_mut().unwrap().remove("interval_type");
        value["expand"] = expand.into();
        let migrated: RichTextSpan = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(migrated.interval_type.raw(), u32::from(expand));
        let canonical = serde_json::to_value(migrated).unwrap();
        assert_eq!(canonical["interval_type"], u32::from(expand));
        assert!(canonical.get("expand").is_none());
        value["interval_type"] = 2.into();
        assert!(serde_json::from_value::<RichTextSpan>(value).is_err());
    }
    for invalid in ["-1", "4294967296", "1.5", "\"1\"", "null"] {
        assert!(
            serde_json::from_str::<SpanIntervalType>(invalid).is_err(),
            "{invalid}"
        );
    }
}
