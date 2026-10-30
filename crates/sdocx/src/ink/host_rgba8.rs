//! Shared RGBA8 arithmetic for the captured Mesa llvmpipe host profile.
//! Native frame regressions establish each caller's admitted use of this model;
//! this does not select a blend profile for saved notes or Samsung devices.

pub(crate) fn quantize(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round_ties_even() as u8
}

/// Captured lowp R8 sample: normalize in F32, narrow to F16 toward zero, widen.
pub(crate) fn sample_unorm8_half_rtz(byte: u8) -> f32 {
    let normalized = f32::from(byte) * f32::from_bits(0x3b80_8081);
    // Nonzero R8 UNORM values are normal in F16, so narrowing drops 13 bits.
    f32::from_bits(normalized.to_bits() & !0x1fff)
}

pub(crate) fn multiply(a: u8, b: u8) -> u8 {
    let product = u32::from(a) * u32::from(b);
    ((product + (product >> 8) + 128) >> 8) as u8
}

pub(crate) fn over(destination: [u8; 4], source: [u8; 4]) -> [u8; 4] {
    let inverse_alpha = 255 - source[3];
    std::array::from_fn(|channel| {
        source[channel].saturating_add(multiply(destination[channel], inverse_alpha))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_byte_multiply_preserves_host_boundary_cases() {
        for (a, b, expected) in [(152, 229, 136), (191, 253, 189), (255, 255, 255)] {
            assert_eq!(multiply(a, b), expected);
            assert_eq!(multiply(b, a), expected);
        }
        let mismatches = (0..=255u8)
            .flat_map(|a| (0..=255u8).map(move |b| (a, b)))
            .filter(|&(a, b)| {
                u32::from(multiply(a, b)) != (u32::from(a) * u32::from(b) + 127) / 255
            })
            .count();
        assert_eq!(mismatches, 24);
    }
}
