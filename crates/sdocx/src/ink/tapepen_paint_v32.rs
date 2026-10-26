//! Test-only reconstruction of a synthetic TapePen v32 host composite.
//! Independent pattern upload and mask readback are inputs; the captured
//! composite is the oracle. v33 confirms ADD, SRC_ALPHA/ONE_MINUS_SRC_ALPHA
//! RGB and ONE/ONE_MINUS_SRC_ALPHA alpha. Transparent target, origin-zero
//! viewport, fixed phase and host Mesa only; no general device parity claimed.
//! Capture source SHA-256: f39dcc671e7ae49ca833f7c1273ed05fba6c417f12ae21e742982346e070476f.
//! Blend-observer capture SHA-256: 7ec706f8023b0c50254803c8ac481ac0b45b7d42031b4422b0d145f20675b8a4.
use flate2::read::ZlibDecoder;
use sha2::{Digest, Sha256};
use std::io::Read;

fn load(packed: &[u8], len: usize, hash: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    ZlibDecoder::new(packed).read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes.len(), len);
    assert_eq!(format!("{:x}", Sha256::digest(&bytes)), hash);
    bytes
}

// Pixel-space sample: centers are integer coordinates. Pattern wraps; mask
// clamps. Bilinear fractions and GL fixed-point rounding may vary by driver.
fn sample(image: &[u8], width: usize, height: usize, x: f32, y: f32, wrap: bool) -> [f32; 4] {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let (fx, fy) = (x - ix as f32, y - iy as f32);
    let index = |v: i32, size: usize| {
        if wrap {
            v.rem_euclid(size as i32) as usize
        } else {
            v.clamp(0, size as i32 - 1) as usize
        }
    };
    let mut out = [0.0; 4];
    for (dx, wx) in [(0, 1.0 - fx), (1, fx)] {
        for (dy, wy) in [(0, 1.0 - fy), (1, fy)] {
            let offset = (index(iy + dy, height) * width + index(ix + dx, width)) * 4;
            for c in 0..4 {
                out[c] += image[offset + c] as f32 / 255.0 * wx * wy;
            }
        }
    }
    out
}

fn paint(pattern: &[u8], mask: &[u8]) -> Vec<u8> {
    assert_eq!(pattern.len(), 64 * 64 * 4);
    assert_eq!(mask.len(), 256 * 256 * 4);
    let mut out = vec![0; 256 * 256 * 4];
    // Exact recorded glUniform2fv bits, converted from normalized UV to pixels.
    let shadow_x = f32::from_bits(0x3a385200) * 256.0;
    let shadow_y = f32::from_bits(0x3a4ccc00) * 256.0;
    let input_alpha = f32::from_bits(0x3f008081);
    for y in 0..256 {
        for x in 0..256 {
            let a = sample(mask, 256, 256, x as f32, y as f32, false)[3];
            let shadow = 0.2
                * sample(
                    mask,
                    256,
                    256,
                    x as f32 - shadow_x,
                    y as f32 - shadow_y,
                    false,
                )[3];
            if a <= 0.0 && shadow <= 0.0 {
                continue;
            }
            let tex = sample(pattern, 64, 64, x as f32, y as f32, true);
            let stroke = if a > 0.0 {
                tex[3] * input_alpha * a
            } else {
                0.0
            };
            let alpha = stroke + shadow * (1.0 - stroke);
            let i = (y * 256 + x) * 4;
            // Source-over on a transparent target; shader emits straight RGB.
            for c in 0..3 {
                // Hypothesis only: an intermediate UNORM8 conversion yields a
                // close match on this host; the capture does not prove it.
                let source = (tex[c] * stroke / alpha.max(1e-5) * 255.0).round();
                let source_alpha = (alpha * 255.0).round();
                let value = source * source_alpha / 255.0;
                out[i + c] = value.round().clamp(0.0, 255.0) as u8;
            }
            out[i + 3] = (alpha * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
    out
}

#[test]
fn v32_host_pattern_composite_matches_within_one_channel_value() {
    let pattern = load(
        include_bytes!("fixtures/tapepen-v32/pattern.rgba.zlib"),
        64 * 64 * 4,
        "78a0a70b418c59c460f8791d48d7f131ad7d445f3797323eb041fd9b689ba189",
    );
    let mask = load(
        include_bytes!("fixtures/tapepen-v32/mask.rgba.zlib"),
        256 * 256 * 4,
        "e7d130ee45bc2ffc1c833559cb09226c1be981b93989845ece21ade38a9d56cc",
    );
    let native = load(
        include_bytes!("fixtures/tapepen-v32/composite.rgba.zlib"),
        256 * 256 * 4,
        "f0406047442a946faa61642c830e535c14afa16d789435f514db5dd9b961f78e",
    );
    let actual = paint(&pattern, &mask);
    let mut different = 0usize;
    let mut max_error = 0u8;
    for (&a, &b) in actual.iter().zip(&native) {
        let d = a.abs_diff(b);
        different += usize::from(d != 0);
        max_error = max_error.max(d);
    }
    // A few one-byte differences remain; this is a bounded host regression,
    // not a claim of exact fragment conversion or Android-device parity.
    assert!(max_error <= 1, "maximum channel error: {max_error}");
    assert!(different <= 12, "different channels: {different}");
    let actual_support: Vec<_> = actual
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| p[3] != 0)
        .collect();
    let native_support: Vec<_> = native
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| p[3] != 0)
        .collect();
    assert_eq!(actual_support, native_support, "alpha support differs");
    assert!(native_support.iter().any(|&v| v), "empty native output");
    assert!(
        native_support.iter().any(|&v| !v),
        "native output fills viewport"
    );
}

#[test]
fn v41_reveal_intermediate_attenuation_matches_bounded_host_oracle() {
    // Inputs are independent native snapshots at the R8 reveal-mask output and
    // the RGBA intermediate immediately before program 7. The post-draw
    // intermediate is the oracle. This numerical fit does not establish GPU
    // precision, native mask generation, or production page composition.
    // V41 runner SHA-256: ddfc99e6f9af9f68020dc86802beae3eb14a15af4bd228ce60a22805a046adc4.
    // Reveal-on capture SHA-256: 98b15543b3db1b008d885b490d48f933aee17e9c16711b626b67a32374b36b77.
    let mask = load(
        include_bytes!("fixtures/tapepen-v41/mask.rgba.zlib"),
        256 * 256 * 4,
        "f5f0d57a32475ae30ea24e34c7179e104fb1d3034172053818b9284266bdc494",
    );
    let before = load(
        include_bytes!("fixtures/tapepen-v41/preimage.rgba.zlib"),
        256 * 256 * 4,
        "045b081614f58f38282261248b0d1766698347718851baf0b158e3f63c805d52",
    );
    let oracle = load(
        include_bytes!("fixtures/tapepen-v41/oracle.rgba.zlib"),
        256 * 256 * 4,
        "3bb884d8fed797088f2e72f4522a19bbe11aa70bb640e5a267c8abfb715c56f5",
    );
    let mut differing_channels = 0;
    let mut max_error = 0;
    let mut changed_pixels = 0;
    for ((mask_pixel, input), native) in mask
        .as_chunks::<4>()
        .0
        .iter()
        .zip(before.as_chunks::<4>().0.iter())
        .zip(oracle.as_chunks::<4>().0.iter())
    {
        let factor = 1.0 - 0.9 * f64::from(mask_pixel[0]) / 255.0;
        let mut model = [0_u8; 4];
        for channel in 0..4 {
            model[channel] = (f64::from(input[channel]) * factor).round_ties_even() as u8;
            let error = model[channel].abs_diff(native[channel]);
            differing_channels += usize::from(error != 0);
            max_error = max_error.max(error);
        }
        let mask_support = mask_pixel[0] != 0;
        assert_eq!(model != *input, mask_support, "model change support");
        assert_eq!(native != input, mask_support, "native change support");
        changed_pixels += usize::from(mask_support);
    }
    assert_eq!(changed_pixels, 172);
    assert_eq!(max_error, 1);
    assert_eq!(differing_channels, 31);
}
