//! Shared RGBA8 arithmetic for the captured Mesa llvmpipe host profile.
//! Native frame regressions establish each caller's admitted use of this model;
//! this does not select a blend profile for saved notes or Samsung devices.

/// Captured lowp R8 sample: normalize in F32, narrow to F16 toward zero, widen.
pub(crate) fn sample_unorm8_half_rtz(byte: u8) -> f32 {
    let normalized = f32::from(byte) * f32::from_bits(0x3b80_8081);
    // Nonzero R8 UNORM values are normal in F16, so narrowing drops 13 bits.
    f32::from_bits(normalized.to_bits() & !0x1fff)
}
