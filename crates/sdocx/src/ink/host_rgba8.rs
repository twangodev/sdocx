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

/// Captured RGBA8 linear/clamp sampler, red result narrowed to F16 RTZ.
/// Callers validate dimensions, storage and finite bounded coordinates first.
pub(crate) fn sample_linear_clamp_red(
    pixels: &[[u8; 4]],
    width: usize,
    height: usize,
    uv: [f32; 2],
) -> f32 {
    let axis = |coordinate: f32, size: usize| {
        let q = (coordinate * (size << 8) as f32).round_ties_even() as i32 - 128;
        let i = q >> 8;
        let clamp = |index: i32| index.clamp(0, size as i32 - 1) as usize;
        (clamp(i), clamp(i + 1), q & 255)
    };
    let (x0, x1, fx) = axis(uv[0], width);
    let (y0, y1, fy) = axis(uv[1], height);
    let at = |x: usize, y: usize| pixels[y * width + x][0];
    let lerp = |a: u8, b: u8, fraction: i32| {
        (i32::from(a) + ((fraction * (i32::from(b) - i32::from(a)) + 128) >> 8)).clamp(0, 255) as u8
    };
    sample_unorm8_half_rtz(lerp(
        lerp(at(x0, y0), at(x1, y0), fx),
        lerp(at(x0, y1), at(x1, y1), fx),
        fy,
    ))
}

pub(crate) fn multiply(a: u8, b: u8) -> u8 {
    let product = u32::from(a) * u32::from(b);
    ((product + (product >> 8) + 128) >> 8) as u8
}

pub(crate) fn premultiply(source: [u8; 4]) -> [u8; 4] {
    let [r, g, b, a] = source;
    [multiply(r, a), multiply(g, a), multiply(b, a), a]
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
