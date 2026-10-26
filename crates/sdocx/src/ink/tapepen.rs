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
