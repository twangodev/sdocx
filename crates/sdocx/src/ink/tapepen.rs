//! Bounded synthetic TapePen v32 source-to-stream reconstruction.
//! Oracle: Unicorn 2.1.4, ARM64_MAX; pinned host glibc/Mesa profile.
//! No saved-document, Android device, paint or general-profile parity implied.
// Capture runner SHA-256: f39dcc671e7ae49ca833f7c1273ed05fba6c417f12ae21e742982346e070476f.
// Capture log SHA-256: 49e40f1bddae9aac23ee9bcabc8bfa41760cbbc6cbe8ca742482779f0e0be638.
// The observed zero-distance input was accepted; a zero threshold is a
// compatible test parameter, not an independently observed field value.
// Shared Quad implements the source-recovered SmPath 15-bit subdivision.
use super::path::{P, Quad};

fn sample_quad(
    curve: &Quad,
    carry: &mut f32,
    spacing: f32,
    rgb: [f32; 3],
    out: &mut Vec<[u32; 5]>,
) {
    let length = curve.length;
    while *carry <= length && length > 0.0 {
        let p = curve.at(*carry);
        out.push([
            p.x.to_bits(),
            p.y.to_bits(),
            rgb[0].to_bits(),
            rgb[1].to_bits(),
            rgb[2].to_bits(),
        ]);
        *carry += spacing;
    }
    *carry -= length;
}

// Caller supplies observed spacing and a provisional zero threshold. The
// capture admits the first zero-distance point; exact field +44 is unobserved.
// This model is bounded to finite, non-rainbow RGB and curve-enabled
// four-point input.
fn tape_curve(points: [P; 4], spacing: f32, threshold: f32, rgb: [f32; 3]) -> Vec<[u32; 5]> {
    let mut last = points[0];
    let mut endpoint = last;
    let mut carry = spacing;
    let mut out = Vec::new();
    for &p in &points[..3] {
        if p.sub(last).len() < threshold {
            continue;
        }
        let midpoint = last.mid(p);
        let curve = Quad::new(endpoint, last, midpoint);
        sample_quad(&curve, &mut carry, spacing, rgb, &mut out);
        last = p;
        endpoint = midpoint;
    }
    // endPen builds a terminal line from the previous midpoint, then adds
    // the exact endpoint. SmPath line uses the same 15-bit parameter scale.
    let end = points[3];
    let length = end.sub(endpoint).len();
    if length > 0.0 {
        let scale = 32767.0_f32 * f32::from_bits(0x3800_00fd);
        while carry <= length {
            let p = endpoint.lerp(end, carry * scale / length);
            out.push([
                p.x.to_bits(),
                p.y.to_bits(),
                rgb[0].to_bits(),
                rgb[1].to_bits(),
                rgb[2].to_bits(),
            ]);
            carry += spacing;
        }
    }
    out.push([
        end.x.to_bits(),
        end.y.to_bits(),
        rgb[0].to_bits(),
        rgb[1].to_bits(),
        rgb[2].to_bits(),
    ]);
    out
}

#[test]
fn v32_native_stream() {
    use sha2::{Digest, Sha256};
    let p = |x, y| P {
        x: f32::from_bits(x),
        y: f32::from_bits(y),
    };
    let rgb = [68.0_f32 / 255.0, 170.0_f32 / 255.0, 238.0_f32 / 255.0];
    let actual = tape_curve(
        [
            p(0x00000000, 0x00000000),
            p(0x41200000, 0x00000000),
            p(0x41a00000, 0x40a00000),
            p(0x41f00000, 0x40a00000),
        ],
        f32::from_bits(0x3f000000),
        0.0,
        rgb,
    );
    let bytes: Vec<u8> = actual
        .iter()
        .flat_map(|record| record.iter().flat_map(|word| word.to_le_bytes()))
        .collect();
    assert_eq!(bytes.len(), 1240);
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        "b96ea83724950b6751f26594f57dbd057eb099abe1700b62da593e6cb909a2af"
    );
}
// Native GLSL from the pinned TapePen DSO: vertex offset 0x18bc6 and
// fragment offset 0x18e58. V42 observes MAX blending on the R8 target.
// Mask output is the test oracle only; no captured mask enters this model.
fn reveal_v41_mask(points: &[[u32; 5]]) -> Vec<u8> {
    const WIDTH: usize = 256;
    let mut mask = vec![0u8; WIDTH * WIDTH];
    let aa = f32::from_bits(0x3e25_b3aa);
    let smoothstep = |lo: f32, hi: f32, x: f32| {
        let t = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    for p in points {
        let px = f32::from_bits(p[0]);
        let py = f32::from_bits(p[1]);
        // Recorded half extents (1.1618181467, 4.6472725868),
        // zero rotation/offset, pixel centers, origin-zero viewport.
        for y in (py - 4.647_273).floor().max(0.0) as usize
            ..((py + 4.647_273).ceil() as usize).min(WIDTH)
        {
            for x in (px - 1.161_818_1).floor().max(0.0) as usize
                ..((px + 1.161_818_1).ceil() as usize).min(WIDTH)
            {
                let dx = x as f32 + 0.5 - px;
                let dy = y as f32 + 0.5 - py;
                let k1 = (dx * dx + (dy / 4.0).powi(2)).sqrt();
                let k2 = (dx * dx + (dy / 16.0).powi(2)).sqrt();
                let sd = if k2 < 1e-7 {
                    -1.0
                } else {
                    k1 * (k1 - 1.0) / k2.max(1e-8)
                };
                let outer = 1.0 - smoothstep(0.0, aa, sd);
                let inner = 1.0 - smoothstep(-(0.48 + aa), -(0.48 - aa), sd);
                let value = (outer * inner * 255.0).round() as u8;
                let i = y * WIDTH + x;
                // V42 observed GL_MAX with ONE/ONE factors for this target.
                mask[i] = mask[i].max(value);
            }
        }
    }
    mask
}

#[test]
fn v41_reveal_mask_from_curve_and_shader() {
    use sha2::{Digest, Sha256};
    let p = |x, y| P {
        x: f32::from_bits(x),
        y: f32::from_bits(y),
    };
    let points = tape_curve(
        [
            p(0, 0),
            p(0x4120_0000, 0),
            p(0x41a0_0000, 0x40a0_0000),
            p(0x41f0_0000, 0x40a0_0000),
        ],
        0.5,
        0.0,
        [68.0 / 255.0, 170.0 / 255.0, 238.0 / 255.0],
    );
    let bytes: Vec<u8> = points
        .iter()
        .flatten()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        "b96ea83724950b6751f26594f57dbd057eb099abe1700b62da593e6cb909a2af"
    );
    use std::io::Read;
    let mut oracle = Vec::new();
    flate2::read::ZlibDecoder::new(&include_bytes!("fixtures/tapepen-v41/mask.rgba.zlib")[..])
        .read_to_end(&mut oracle)
        .unwrap();
    assert_eq!(oracle.len(), 256 * 256 * 4);
    assert_eq!(
        format!("{:x}", Sha256::digest(&oracle)),
        "f5f0d57a32475ae30ea24e34c7179e104fb1d3034172053818b9284266bdc494"
    );
    let actual = reveal_v41_mask(&points);
    let mut different = 0;
    let mut max_error = 0;
    for (&a, pixel) in actual.iter().zip(oracle.as_chunks::<4>().0.iter()) {
        different += usize::from(a != pixel[0]);
        max_error = max_error.max(a.abs_diff(pixel[0]));
        assert_eq!(a != 0, pixel[0] != 0);
    }
    assert!(max_error <= 1, "max error {max_error}");
    assert!(different <= 1, "different pixels {different}");
}
