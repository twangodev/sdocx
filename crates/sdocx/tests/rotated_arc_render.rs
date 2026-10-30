#![cfg(feature = "render")]
mod support;
use support::{archive, object, page};
fn single(payload: &[u8]) -> Vec<u8> {
    archive(&page(&[vec![object(7, payload, &[])]], 0, &[]))
}
fn options() -> sdocx::RenderOptions {
    let mut o = sdocx::RenderOptions::default();
    o.rotated_arc_sector_approximation = true;
    o
}
fn count(frame: &[u8], opt: &sdocx::RenderOptions) -> usize {
    let parsed = sdocx::parse_bytes_detailed(&single(frame)).unwrap();
    sdocx::render_document_svg(&parsed.document, opt)[0]
        .svg
        .matches("<path ")
        .count()
}
#[test]
fn scale_and_boundary() {
    for (frame, expected) in [
        (
            include_bytes!("fixtures/native-arc-scaled-0-5-frame.bin").as_slice(),
            2,
        ),
        (
            include_bytes!("fixtures/native-arc-scaled-2-frame.bin").as_slice(),
            1,
        ),
    ] {
        assert_eq!(count(frame, &Default::default()), 1);
        assert_eq!(count(frame, &options()), expected);
    }
    let source = include_bytes!("fixtures/native-arc-rotated-frame.bin");
    for (offset, delta, expected) in [
        (501, 0.00006, 1),
        (501, 0.0001, 1),
        (517, 0.0001, 2),
        (517, 0.00015, 1),
    ] {
        let mut frame = source.to_vec();
        let x = f64::from_le_bytes(frame[offset..offset + 8].try_into().unwrap());
        frame[offset..offset + 8].copy_from_slice(&(x + delta).to_le_bytes());
        assert_eq!(count(&frame, &options()), expected);
    }
}
#[cfg(feature = "serde")]
#[test]
fn legacy_options_default_to_fallback() {
    let mut value = serde_json::to_value(sdocx::RenderOptions::default()).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .remove("rotated_arc_sector_approximation");
    let old: sdocx::RenderOptions = serde_json::from_value(value).unwrap();
    assert!(!old.rotated_arc_sector_approximation);
    let roundtrip: sdocx::RenderOptions =
        serde_json::from_value(serde_json::to_value(options()).unwrap()).unwrap();
    assert!(roundtrip.rotated_arc_sector_approximation);
}
