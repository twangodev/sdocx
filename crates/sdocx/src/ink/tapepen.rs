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
// Explicit captured shader and raster profile for two host controls. This
// uses bounding-box iteration rather than general triangle edge coverage and
// approximates fragment precision and UNORM rounding. Rotation and offset
// are zero, the viewport starts at the origin, and blending is MAX.
#[derive(Clone, Copy)]
struct RevealProfile {
    half_extent: [f32; 2],
    point_radius: f32,
    aa_width: f32,
    border_width: f32,
    rotation_rad: f32,
    offset: [f32; 2],
}

fn reveal_mask(points: &[[u32; 5]], profile: RevealProfile) -> Vec<u8> {
    const WIDTH: usize = 256;
    assert_eq!(profile.rotation_rad, 0.0);
    assert_eq!(profile.offset, [0.0, 0.0]);
    assert!(profile.point_radius > 0.0 && profile.aa_width > 0.0);
    let mut mask = vec![0u8; WIDTH * WIDTH];
    let aa = profile.aa_width.max(0.001);
    let ax = profile.point_radius * 0.25;
    let ay = profile.point_radius;
    let border = profile.border_width.max(0.0).min(0.48 * ax.min(ay));
    let b = border.max(aa * 2.0);
    let smoothstep = |lo: f32, hi: f32, x: f32| {
        let t = ((x - lo) / (hi - lo)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    };
    for p in points {
        let px = f32::from_bits(p[0]);
        let py = f32::from_bits(p[1]);
        for y in (py - profile.half_extent[1]).floor().max(0.0) as usize
            ..((py + profile.half_extent[1]).ceil() as usize).min(WIDTH)
        {
            for x in (px - profile.half_extent[0]).floor().max(0.0) as usize
                ..((px + profile.half_extent[0]).ceil() as usize).min(WIDTH)
            {
                let dx = x as f32 + 0.5 - px;
                let dy = y as f32 + 0.5 - py;
                let k1 = ((dx / ax).powi(2) + (dy / ay).powi(2)).sqrt();
                let k2 = ((dx / (ax * ax)).powi(2) + (dy / (ay * ay)).powi(2)).sqrt();
                let sd = if k2 < 1e-7 {
                    -ax.min(ay)
                } else {
                    k1 * (k1 - 1.0) / k2.max(1e-8)
                };
                let outer = 1.0 - smoothstep(0.0, aa, sd);
                let inner = 1.0 - smoothstep(-(b + aa), -(b - aa), sd);
                let value = (outer * inner * 255.0).round() as u8;
                let i = y * WIDTH + x;
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
    let actual = reveal_mask(
        &points,
        RevealProfile {
            half_extent: [1.161_818_1, 4.647_273],
            point_radius: 4.0,
            aa_width: f32::from_bits(0x3e25_b3aa),
            border_width: 3.0,
            rotation_rad: 0.0,
            offset: [0.0, 0.0],
        },
    );
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

// Independent model points versus native instance buffer and mask readback.
// The 0.5 spacing is captured in V43 getLength_call; threshold 0 is a
// provisional profile input, not a claim that native field +44 was read.
// V43 runner SHA-256: 181984ffe944b869a3aec1de07367e9b56fc2fd53477113be0307f61b7d11fe0.
#[test]
fn v43_reveal_bend_model_against_native() {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let points = tape_curve(
        [
            P { x: 32.0, y: 32.0 },
            P { x: 42.0, y: 32.0 },
            P { x: 42.0, y: 42.0 },
            P { x: 52.0, y: 42.0 },
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
    assert_eq!(bytes.len(), 980);
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        "f18881c934fca24b1067c40cd9b8d13310a41e6da3835d5735c1da59103dc085"
    );
    let profile = RevealProfile {
        half_extent: [1.680_000_1, 6.720_000_3],
        point_radius: 6.0,
        aa_width: 0.12,
        border_width: 3.0,
        rotation_rad: 0.0,
        offset: [0.0, 0.0],
    };
    let actual = reveal_mask(&points, profile);
    let mut oracle = Vec::new();
    flate2::read::ZlibDecoder::new(&include_bytes!("fixtures/tapepen-v43/mask.rgba.zlib")[..])
        .read_to_end(&mut oracle)
        .unwrap();
    assert_eq!(oracle.len(), 256 * 256 * 4);
    assert_eq!(
        format!("{:x}", Sha256::digest(&oracle)),
        "edcbbc59f56cb7bcaaef0889bb71c1b35b69dd7bff615d40cc27ea9d4458f7e4"
    );
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

// V43 rotated path. Rotation here describes input geometry; the captured
// stamp-rotation uniform is zero. Threshold remains a provisional profile input.
#[test]
fn v43_reveal_rotated_diagonal_model_against_native() {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let p = |x, y| P {
        x: f32::from_bits(x),
        y: f32::from_bits(y),
    };
    let points = tape_curve(
        [
            p(0x4240_0000, 0x4220_0000),
            p(0x424e_a41a, 0x4256_a41a),
            p(0x425d_4833, 0x4286_a41a),
            p(0x426b_ec4d, 0x42a1_f626),
        ],
        0.5,
        0.0,
        [204.0 / 255.0, 102.0 / 255.0, 51.0 / 255.0],
    );
    let bytes: Vec<u8> = points
        .iter()
        .flatten()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    assert_eq!(bytes.len(), 1700);
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        "6485e1daa81e086c8042bc1b29d0ad1e377a3370ad1d1eb0847583a6b6596211"
    );
    let actual = reveal_mask(
        &points,
        RevealProfile {
            half_extent: [f32::from_bits(0x3f63_4219), f32::from_bits(0x4063_4219)],
            point_radius: 3.0,
            aa_width: f32::from_bits(0x3e3c_0b2c),
            border_width: 3.0,
            rotation_rad: 0.0,
            offset: [0.0, 0.0],
        },
    );
    let mut oracle = Vec::new();
    flate2::read::ZlibDecoder::new(
        &include_bytes!("fixtures/tapepen-v43/diagonal-mask.rgba.zlib")[..],
    )
    .read_to_end(&mut oracle)
    .unwrap();
    assert_eq!(oracle.len(), 256 * 256 * 4);
    assert_eq!(
        format!("{:x}", Sha256::digest(&oracle)),
        "c27ce4a8e1d6ee4a74f7a3b9d91a3250a666ee798706ebbd2fe5a07450eaa212"
    );
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
