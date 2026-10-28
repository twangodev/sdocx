const BASELINE: &[u8] = include_bytes!("../../../conformance/montblanc-translation-baseline.rgba");
const TRANSLATED: &[u8] =
    include_bytes!("../../../conformance/montblanc-translation-translated.rgba");

fn frame(crop: &[u8], left: usize, top: usize) -> Vec<u8> {
    assert_eq!(crop.len(), 23 * 7 * 4);
    let mut frame = vec![0; 256 * 256 * 4];
    for y in 0..7 {
        let offset = ((top + y) * 256 + left) * 4;
        frame[offset..offset + 23 * 4].copy_from_slice(&crop[y * 23 * 4..(y + 1) * 23 * 4]);
    }
    frame
}

#[test]
fn native_montblanc_integer_translation_moves_every_pixel() {
    let baseline = frame(BASELINE, 127, 126);
    let translated = frame(TRANSLATED, 143, 134);
    let mut shifted = vec![0; baseline.len()];
    for y in 0..248 {
        let source = y * 256 * 4;
        let dest = ((y + 8) * 256 + 16) * 4;
        shifted[dest..dest + 240 * 4].copy_from_slice(&baseline[source..source + 240 * 4]);
    }
    assert_eq!(shifted, translated);
}
