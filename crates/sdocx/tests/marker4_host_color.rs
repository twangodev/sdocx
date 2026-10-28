//! Color encoding boundaries for the captured Marker4 V7 Mesa control.

#[test]
fn captured_colors_survive_straight_rgba8_encoding() {
    let pixels = include_bytes!("../../../conformance/marker4-v7-independent-colors.rgba");
    assert_eq!(pixels.len(), 282 * 24);
    let colors = [[255u32, 203, 48], [0, 64, 255]];
    for record in pixels.as_chunks::<24>().0 {
        for (index, color) in colors.iter().enumerate() {
            let pixel = &record[index * 4..index * 4 + 4];
            let alpha = u32::from(pixel[3]);
            assert!(alpha > 0);
            for channel in 0..3 {
                let native = u32::from(pixel[channel]);
                assert_eq!(native, (alpha * color[channel] + 127) / 255);
                let straight = (native * 255 + alpha / 2) / alpha;
                assert_eq!((straight * alpha + 127) / 255, native);
            }
        }
    }
}
