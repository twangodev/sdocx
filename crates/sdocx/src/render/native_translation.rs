/// Offset in native f32 arithmetic, then round outwards. Rectangle area is a
/// caller policy: native text writer windows also accept degenerate rectangles.
pub(super) fn offset_rounded_rect(rect: [f32; 4], origin: [f32; 2]) -> Option<[f32; 4]> {
    if !rect.into_iter().chain(origin).all(f32::is_finite) {
        return None;
    }
    let shifted = std::array::from_fn::<_, 4, _>(|axis| rect[axis] + origin[axis % 2]);
    if !shifted.into_iter().all(f32::is_finite) {
        return None;
    }
    Some([
        shifted[0].floor(),
        shifted[1].floor(),
        shifted[2].ceil(),
        shifted[3].ceil(),
    ])
}
