//! Independent native Marker4 drawables, bounded to one Mesa host control.
#[allow(dead_code)]
#[path = "../src/ink/host_rgba8.rs"]
mod host_rgba8;

#[test]
fn marker4_independent_colors_composite_in_draw_order() {
    let pixels = include_bytes!("assets/marker4-independent-colors.rgba");
    assert_eq!(pixels.len(), 282 * 24);
    let background = [32, 64, 128, 255];
    let mut different = 0;
    for record in pixels.as_chunks::<24>().0 {
        let pixel =
            |index: usize| -> [u8; 4] { record[index * 4..index * 4 + 4].try_into().unwrap() };
        let [a, b, ab, ba, bg, bg0] = std::array::from_fn(pixel);
        assert_eq!(host_rgba8::over(a, b), ab);
        assert_eq!(host_rgba8::over(b, a), ba);
        assert_eq!(host_rgba8::over(background, a), bg0);
        assert_eq!(host_rgba8::over(bg0, b), bg);
        different += usize::from(ab != ba);
    }
    assert!(different > 0);
}
