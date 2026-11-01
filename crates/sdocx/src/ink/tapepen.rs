//! Bounded synthetic TapePen v32 source-to-stream reconstruction.
//! Oracle: Unicorn 2.1.4, ARM64_MAX; pinned host glibc/Mesa profile.
//! No saved-document, Android device, paint or general-profile parity implied.
// Capture runner SHA-256: f39dcc671e7ae49ca833f7c1273ed05fba6c417f12ae21e742982346e070476f.
// Capture log SHA-256: 49e40f1bddae9aac23ee9bcabc8bfa41760cbbc6cbe8ca742482779f0e0be638.
// The observed zero-distance input was accepted; a zero threshold is a
// compatible test parameter, not an independently observed field value.
// Shared Quad implements the source-recovered SmPath 15-bit subdivision.
use super::path::{P, Quad};

// PenCommon ConvertSizeToSizeLevel (0x53a64), with TapePen's native
// GetMinPxSize/GetMaxPxSize values (6 and 30). Restrict the profile to
// finite sizes and positive screen dimensions; other native inputs are untested.
fn tape_size_level(size: f32, width: i32, height: i32) -> i32 {
    assert!(size.is_finite() && width > 0 && height > 0);
    let screen = width.min(height) as f32;
    let min = (6.0_f32 * screen) / 360.0;
    let max = (30.0_f32 * screen) / 360.0;
    if size <= min {
        1
    } else if size >= max {
        100
    } else {
        (((size - min) / (max - min) * 100.0).round() as i32).clamp(1, 100)
    }
}

// TapePen SetSizeLevel (0x3c594), including its single fused operation.
fn tape_aa_width(level: i32) -> f32 {
    if level < 2 {
        f32::from_bits(0x3e4c_cccd)
    } else if level > 99 {
        f32::from_bits(0x3ca3_d70a)
    } else {
        ((level - 1) as f32).mul_add(f32::from_bits(0xbaee_5010), f32::from_bits(0x3e4c_cccd))
    }
}

#[test]
fn v110_size_conversion_and_aa_uniforms() {
    for (size, level, aa) in [
        (2.0, 1, 0x3e4c_cccd),
        (6.0, 10, 0x3e3c_0b2c),
        (8.0, 22, 0x3e25_b3aa),
        (10.0, 34, 0x3e0f_5c29),
        (12.0, 45, 0x3df5_c28f),
        (16.0, 69, 0x3d9c_6489),
    ] {
        assert_eq!(tape_size_level(size, 256, 256), level);
        assert_eq!(tape_aa_width(level).to_bits(), aa);
    }
    assert_eq!(tape_aa_width(1).to_bits(), 0x3e4c_cccd);
    assert_eq!(tape_aa_width(100).to_bits(), 0x3ca3_d70a);
}

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
// capture admits the first zero-distance point; the meaning of field +44 is unverified.
// This model is bounded to finite, non-rainbow RGB and curve-enabled
// input of at least two points. The two-point result is a host-only control;
// general branch and color behavior remain unverified.
fn tape_curve(points: &[P], spacing: f32, threshold: f32, rgb: [f32; 3]) -> Vec<[u32; 5]> {
    assert!(points.len() >= 2);
    let mut last = points[0];
    let mut endpoint = last;
    let mut carry = spacing;
    let mut out = Vec::new();
    for &p in &points[..points.len() - 1] {
        if p.sub(last).len() < threshold {
            continue;
        }
        let midpoint = last.mid(p);
        let curve = Quad::new(endpoint, last, midpoint);
        sample_quad(&curve, &mut carry, spacing, rgb, &mut out);
        last = p;
        endpoint = midpoint;
    }
    // Native saved redraw sets a flag, and drawPoint clears it after AddPoint.
    // If no point has been emitted, endPen uses the endpoint-only branch.
    if out.is_empty() {
        let end = points[points.len() - 1];
        out.push([
            end.x.to_bits(),
            end.y.to_bits(),
            rgb[0].to_bits(),
            rgb[1].to_bits(),
            rgb[2].to_bits(),
        ]);
        return out;
    }
    // endPen builds a terminal line from the previous midpoint, then adds
    // the exact endpoint. SmPath line uses the same 15-bit parameter scale.
    let end = points[points.len() - 1];
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

// Bounded non-curve synthetic control, with caller-supplied observed spacing.
// Sampling advances through straight segments and then emits the exact end.
// General zero-length and threshold behavior beyond this control is unverified.
fn tape_polyline(points: &[P], spacing: f32, rgb: [f32; 3]) -> Vec<[u32; 5]> {
    assert!(points.len() >= 2);
    let mut carry = spacing;
    let mut out = Vec::new();
    let scale = 32767.0_f32 * f32::from_bits(0x3800_00fd);
    for pair in points.windows(2) {
        let a = pair[0];
        let b = pair[1];
        let length = b.sub(a).len();
        if length > 0.0 {
            while carry <= length {
                let p = a.lerp(b, carry * scale / length);
                out.push([
                    p.x.to_bits(),
                    p.y.to_bits(),
                    rgb[0].to_bits(),
                    rgb[1].to_bits(),
                    rgb[2].to_bits(),
                ]);
                carry += spacing;
            }
            carry -= length;
        }
    }
    let end = points[points.len() - 1];
    out.push([
        end.x.to_bits(),
        end.y.to_bits(),
        rgb[0].to_bits(),
        rgb[1].to_bits(),
        rgb[2].to_bits(),
    ]);
    out
}

// V54 synthetic Model with curve disabled; pinned host profile only.
// Capture log SHA-256: 15a3affd7b42ed7e1108d1e83bf38a9a6184303d31fa9ba689f1469f1a55dad5.
#[test]
fn v54_no_curve_stream_matches_bounded_host() {
    use sha2::{Digest, Sha256};
    let points = tape_polyline(
        &[
            P { x: 32.0, y: 32.0 },
            P { x: 42.0, y: 32.0 },
            P { x: 42.0, y: 42.0 },
            P { x: 52.0, y: 42.0 },
        ],
        0.5,
        [68.0 / 255.0, 170.0 / 255.0, 238.0 / 255.0],
    );
    let bytes: Vec<u8> = points
        .iter()
        .flatten()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    assert_eq!(bytes.len(), 1220);
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        "ffa896f7013d9d6a42daf588bd277ea0611ce410a6c814f040a7a9598171f2f8"
    );
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
        &[
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

// V51 synthetic Model, no saved-document or device-parity claim. Native
// receiver +44 was observed zero; getLength recorded 0.5 spacing.
// Runner SHA-256: 6ef017d864d3d427921080cbfd4645860a5333f2e275fa95e27b63d26389e20c.
// Receipt SHA-256: 78e03e21d3033d69796781ccc1c9338a6eab17a92cbbf6ca0811e0d9096412bc.
#[test]
fn v51_three_point_curve_stream_matches_native() {
    use sha2::{Digest, Sha256};
    let points = tape_curve(
        &[
            P { x: 32.0, y: 32.0 },
            P { x: 42.0, y: 32.0 },
            P { x: 42.0, y: 42.0 },
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
    assert_eq!(bytes.len(), 660);
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        "eb2dfdc2f61ac3f9ef836fd0f1ccfb15d6b821850f3decee820a6eef8fcd3ccd"
    );
}
// Five-point synthetic Model; this tests a source-derived stream against the
// native AddPoint and source-tracked instance buffer. getLength observed 0.5
// spacing; threshold 0 is a compatible model input, with no inference here
// about the native threshold field. No saved-document or device parity claim.
// Native receipt SHA-256: c7d5c61cf02b0d6ec876e37c1d30d36102d3087d88fbc17516191dc0b93ffadf.
#[test]
fn v51_five_point_curve_stream_matches_native() {
    use sha2::{Digest, Sha256};
    let points = tape_curve(
        &[
            P { x: 32.0, y: 32.0 },
            P { x: 42.0, y: 32.0 },
            P { x: 42.0, y: 42.0 },
            P { x: 52.0, y: 42.0 },
            P { x: 62.0, y: 32.0 },
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
    assert_eq!(bytes.len(), 1560);
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        "4dcb3e46c86849ad2f818b555d608925ce2c5320e393d93884ce1985ce155bbd"
    );
}

// V54 two-point synthetic control: host glibc fmodf(0, 0) produced NaN;
// the guarded native consumer had an empty palette and returned finite RGB.
// This does not establish Android math or guest floating-point-state parity.
// Capture log SHA-256: 6bbc4fce620e72f04ddfd1fc6b4ea67aac4049a4c42a99ad85aeea912c37d685.
#[test]
fn v54_two_point_curve_stream_matches_bounded_host() {
    use sha2::{Digest, Sha256};
    let points = tape_curve(
        &[P { x: 32.0, y: 32.0 }, P { x: 42.0, y: 32.0 }],
        0.5,
        0.0,
        [68.0 / 255.0, 170.0 / 255.0, 238.0 / 255.0],
    );
    let bytes: Vec<u8> = points
        .iter()
        .flatten()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    assert_eq!(bytes.len(), 20);
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        "de62cd988bb853335a3e4ca792621fdbde26eeb9808eae9dca9d8fd14101d75a"
    );
}

// Repeated-point synthetic Model under the pinned host profile; no saved-note
// or device parity claim. V54 log SHA-256: 100b5ea9e7112465d313bbc4b718c569bb81ef12a71e5a2e18350097d911bef3.
#[test]
fn v54_repeated_curve_stream_matches_bounded_host() {
    use sha2::{Digest, Sha256};
    let points = tape_curve(
        &[
            P { x: 32.0, y: 32.0 },
            P { x: 32.0, y: 32.0 },
            P { x: 42.0, y: 32.0 },
            P { x: 42.0, y: 32.0 },
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
    assert_eq!(bytes.len(), 420);
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        "2b2b2f4b8722b906e1af81a28961a38a6f798b8839ebb9cc6f8f9196807c2292"
    );
}
// V54 short synthetic Model under the pinned host profile. The captured
// first three paths emit no samples; endPen supplies the exact final point.
// Capture log SHA-256: a0cd33ee092a0152be591c3aedbfb643919e3746527c6a4538fd8a9c237a8387.
// No saved-note, Android math, or device parity claim.
#[test]
fn v54_short_curve_stream_matches_bounded_host() {
    use sha2::{Digest, Sha256};
    let p = |x, y| P {
        x: f32::from_bits(x),
        y: f32::from_bits(y),
    };
    let points = tape_curve(
        &[
            p(0x4200_0000, 0x4200_0000),
            p(0x4200_6666, 0x4200_0000),
            p(0x4200_cccd, 0x4200_6666),
            p(0x4201_3333, 0x4200_6666),
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
    assert_eq!(bytes.len(), 20);
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        "15643cb421bd4ebb4d886dcff3a80f7f70f234336346f50ec3a2e01c834431ae"
    );
}
// Native GLSL from the pinned TapePen DSO: vertex offset 0x18bc6 and
// fragment offset 0x18e58. V42 observes MAX blending on the R8 target.
// Mask output is the test oracle only; no captured mask enters this model.
// Explicit captured shader and raster profile for bounded host controls. This
// uses inverse-rotated rectangle coverage rather than general triangle edge
// coverage and approximates fragment precision and UNORM rounding. The
// viewport starts at the origin and blending is MAX.
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
    assert_eq!(profile.offset, [0.0, 0.0]);
    assert!(profile.point_radius > 0.0 && profile.aa_width > 0.0);
    let mut mask = vec![0u8; WIDTH * WIDTH];
    let (sin, cos) = profile.rotation_rad.sin_cos();
    let hx = profile.half_extent[0];
    let hy = profile.half_extent[1];
    let bound_x = (cos * hx).abs() + (sin * hy).abs();
    let bound_y = (sin * hx).abs() + (cos * hy).abs();
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
        let px = f32::from_bits(p[0]) + profile.offset[0];
        let py = f32::from_bits(p[1]) + profile.offset[1];
        for y in
            (py - bound_y).floor().max(0.0) as usize..((py + bound_y).ceil() as usize).min(WIDTH)
        {
            for x in (px - bound_x).floor().max(0.0) as usize
                ..((px + bound_x).ceil() as usize).min(WIDTH)
            {
                let screen_x = x as f32 + 0.5 - px;
                let screen_y = y as f32 + 0.5 - py;
                // Fragment shader rotates vPhysRot back to ellipse-local space.
                let dx = cos * screen_x + sin * screen_y;
                let dy = -sin * screen_x + cos * screen_y;
                if dx.abs() > hx || dy.abs() > hy {
                    continue;
                }
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
        &[
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
        &[
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
        &[
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

// Native ObjectStroke::SetRotation has already run on this input. This model
// starts with the native post-rotation point array; it does not implement that
// setter. Spacing 0.5 is observed; zero threshold remains provisional.
// V46 runner SHA-256: 3a50791175105e279beed31cef3413d652e21e57a12f24fb7cbd8d2b49b21a0f.
#[test]
fn v46_reveal_native_model_rotation_against_native_mask() {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let p = |x, y| P {
        x: f32::from_bits(x),
        y: f32::from_bits(y),
    };
    let points = tape_curve(
        &[
            p(0x420f_5be6, 0x41dd_5be6),
            p(0x4232_0000, 0x4202_adf3),
            p(0x421e_0000, 0x4225_520d),
            p(0x4240_a41a, 0x4239_520d),
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
        "ca5a310ff7667cc220187ec6da530384936a3505f20cd967fa683bf9a70b7253"
    );
    let actual = reveal_mask(
        &points,
        RevealProfile {
            half_extent: [1.680_000_1, 6.720_000_3],
            point_radius: 6.0,
            aa_width: 0.12,
            border_width: 3.0,
            rotation_rad: f32::from_bits(0x3f06_0a92),
            offset: [0.0, 0.0],
        },
    );
    let mut oracle = Vec::new();
    flate2::read::ZlibDecoder::new(&include_bytes!("fixtures/tapepen-v46/mask.rgba.zlib")[..])
        .read_to_end(&mut oracle)
        .unwrap();
    assert_eq!(oracle.len(), 256 * 256 * 4);
    assert_eq!(
        format!("{:x}", Sha256::digest(&oracle)),
        "10b0db835e4be38b4e67204d90bf231d759b6f57f695790cf18c3636a10558d9"
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

// Bounded TapePen pattern-size model, using the captured axis-aligned
// Matrix4 profiles only. V49 used identity; V50 used scale (1.5, 0.75)
// and translation (24, 16). This does not model rotation or general matrices.
// RTV1::Draw 0x3a9bc..0x3aa38 and transform helper 0x3b154..0x3b17c:
// matrix-vector products use FMUL followed by FMLA, then homogeneous divide,
// subtraction of the transformed origin, and finally the scale divisor.
fn pattern_size(scale: f32, texture_width: i32, matrix: [f32; 4]) -> [f32; 2] {
    let extent = scale * texture_width as f32;
    // The captured matrix has zero off-diagonal and perspective terms. Keep
    // the native multiply/fused-add order instead of cancelling translation.
    let transform = |axis_scale: f32, translation: f32, x: f32, y: f32| {
        let lane_x = 0.0_f32 * y; // initial FMUL
        let lane_x = axis_scale.mul_add(x, lane_x);
        let lane_x = 0.0_f32.mul_add(0.0, lane_x);
        translation.mul_add(1.0, lane_x)
    };
    let origin = [matrix[2] / 1.0_f32, matrix[3] / 1.0_f32];
    let transformed = [
        transform(matrix[0], matrix[2], extent, extent),
        // For Y, native starts with scale_y * y before adding zero * x.
        0.0_f32.mul_add(extent, matrix[1] * extent),
    ];
    let transformed = [
        transformed[0],
        matrix[3].mul_add(1.0, 0.0_f32.mul_add(0.0, transformed[1])),
    ];
    let divisor = if scale > 1.0e-6_f32 { scale } else { 4.0_f32 };
    [
        (transformed[0] / 1.0_f32 - origin[0]) / divisor,
        (transformed[1] / 1.0_f32 - origin[1]) / divisor,
    ]
}

#[test]
fn compare_independent_native_uniform_words() {
    let identity = [1.0, 1.0, 0.0, 0.0];
    let transformed = [1.5, 0.75, 24.0, 16.0];
    // Exact input words and GL uniform words: V49/V50 paired 0/1 captures.
    for (matrix, scale_bits, expected) in [
        (identity, 0x0000_0000, [0x0000_0000, 0x0000_0000]),
        (identity, 0x3f80_0000, [0x4280_0000, 0x4280_0000]),
        (transformed, 0x0000_0000, [0x0000_0000, 0x0000_0000]),
        (transformed, 0x3f80_0000, [0x42c0_0000, 0x4240_0000]),
        // V50 translated tiny-scale capture retains rounding before subtraction.
        (transformed, 0x33d6_bf95, [0x3620_0000, 0x35c0_0000]),
        // V47/V48 identity captures.
        (identity, 0x4000_0000, [0x4280_0000, 0x4280_0000]),
        (identity, 0x33d6_bf95, [0x35d6_bf95, 0x35d6_bf95]),
    ] {
        assert_eq!(
            pattern_size(f32::from_bits(scale_bits), 64, matrix).map(f32::to_bits),
            expected
        );
    }
}

// Filled-stroke shader under the captured axis-aligned host raster profile.
// The input stream is generated from source points; no captured pixels are used.
fn filled_mask(points: &[[u32; 5]], radii: [f32; 2], extent: [f32; 2], aa: f32) -> Vec<u8> {
    let mut mask = vec![0; 256 * 256];
    for p in points {
        let px = f32::from_bits(p[0]);
        let py = f32::from_bits(p[1]);
        for y in
            (py - extent[1]).floor().max(0.0) as usize..((py + extent[1]).ceil() as usize).min(256)
        {
            for x in (px - extent[0]).floor().max(0.0) as usize
                ..((px + extent[0]).ceil() as usize).min(256)
            {
                let dx = x as f32 + 0.5 - px;
                let dy = y as f32 + 0.5 - py;
                if dx.abs() > extent[0] || dy.abs() > extent[1] {
                    continue;
                }
                let dist = (dx / radii[0]).powi(2) + (dy / radii[1]).powi(2);
                let t = ((dist - (1.0 - aa)) / aa).clamp(0.0, 1.0);
                let value = ((1.0 - t * t * (3.0 - 2.0 * t)) * 255.0).round() as u8;
                let i = y * 256 + x;
                mask[i] = mask[i].max(value);
            }
        }
    }
    mask
}

#[test]
fn v107_filled_mask_from_source_matches_native_size4() {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let points = tape_curve(
        &[
            P { x: 13.0, y: 21.0 },
            P { x: 22.0, y: 29.0 },
            P { x: 34.0, y: 18.0 },
            P { x: 47.0, y: 28.0 },
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
        "d448a37f1d7f7f596504492fbd346c06a892a307dc8fd016d9d01c375da69146"
    );
    let mut oracle = Vec::new();
    flate2::read::ZlibDecoder::new(
        &include_bytes!("fixtures/tapepen-v107/size4-mask.rgba.zlib")[..],
    )
    .read_to_end(&mut oracle)
    .unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&oracle)),
        "ab8787a247a7cfa866f6ddd0d55cc25ccb169da24ae5c6780ca76a8380b5b88f"
    );
    assert_eq!(oracle.len(), 256 * 256 * 4);
    let actual = filled_mask(
        &points,
        [0.5, 2.0],
        [0.6, 2.4],
        tape_aa_width(tape_size_level(4.0, 256, 256)),
    );
    assert!(
        actual
            .iter()
            .zip(oracle.as_chunks::<4>().0.iter())
            .all(|(a, p)| *a == p[3])
    );
}

#[test]
fn v109_size12_requires_size_specific_antialiasing() {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let points = tape_curve(
        &[
            P { x: 13.0, y: 21.0 },
            P { x: 22.0, y: 29.0 },
            P { x: 34.0, y: 18.0 },
            P { x: 47.0, y: 28.0 },
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
        "d448a37f1d7f7f596504492fbd346c06a892a307dc8fd016d9d01c375da69146"
    );
    let mut oracle = Vec::new();
    flate2::read::ZlibDecoder::new(
        &include_bytes!("fixtures/tapepen-v107/size12-mask.rgba.zlib")[..],
    )
    .read_to_end(&mut oracle)
    .unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&oracle)),
        "85b6c8bfea61ce8de48800b683a088c86dd363ae413a61e0356a3f6964f141f8"
    );
    assert_eq!(oracle.len(), 256 * 256 * 4);
    let actual = filled_mask(&points, [1.5, 6.0], [1.6800001, 6.7200003], 0.2);
    let different = actual
        .iter()
        .zip(oracle.as_chunks::<4>().0.iter())
        .filter(|(a, p)| **a != p[3])
        .count();
    assert_eq!(different, 44);
    let actual = filled_mask(
        &points,
        [1.5, 6.0],
        [1.6800001, 6.7200003],
        tape_aa_width(tape_size_level(12.0, 256, 256)),
    );
    assert!(
        actual
            .iter()
            .zip(oracle.as_chunks::<4>().0.iter())
            .all(|(a, p)| *a == p[3])
    );
}

#[test]
fn v112_size6_filled_mask_from_source() {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let points = tape_curve(
        &[
            P { x: 13.0, y: 21.0 },
            P { x: 22.0, y: 29.0 },
            P { x: 34.0, y: 18.0 },
            P { x: 47.0, y: 28.0 },
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
        "d448a37f1d7f7f596504492fbd346c06a892a307dc8fd016d9d01c375da69146"
    );
    let mut oracle = Vec::new();
    flate2::read::ZlibDecoder::new(
        &include_bytes!("fixtures/tapepen-v107/size6-mask.rgba.zlib")[..],
    )
    .read_to_end(&mut oracle)
    .unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&oracle)),
        "784fbd4b20ee23f544fd940b3a2968260d75c1465c3ab2a59b846e88080b13fe"
    );
    let actual = filled_mask(
        &points,
        [0.75, 3.0],
        [f32::from_bits(0x3f63_4219), f32::from_bits(0x4063_4219)],
        tape_aa_width(tape_size_level(6.0, 256, 256)),
    );
    assert!(
        actual
            .iter()
            .zip(oracle.as_chunks::<4>().0.iter())
            .all(|(a, p)| *a == p[3])
    );
}

#[test]
fn v113_rotated_filled_mask_from_source() {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let points = tape_curve(
        &[
            P { x: 13.0, y: 21.0 },
            P { x: 22.0, y: 29.0 },
            P { x: 34.0, y: 18.0 },
            P { x: 47.0, y: 28.0 },
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
        "d448a37f1d7f7f596504492fbd346c06a892a307dc8fd016d9d01c375da69146"
    );
    let mut oracle = Vec::new();
    flate2::read::ZlibDecoder::new(
        &include_bytes!("fixtures/tapepen-v107/rotation30-size12-mask.rgba.zlib")[..],
    )
    .read_to_end(&mut oracle)
    .unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&oracle)),
        "8dbd375b1bc140fcc8a6be8fc24cc1511f3d20e94ab7d192c79446f42c64619a"
    );
    let aa = tape_aa_width(tape_size_level(12.0, 256, 256));
    let (sin, cos) = f32::from_bits(0x3f06_0a92).sin_cos();
    let (hx, hy) = (1.6800001_f32, 6.7200003_f32);
    let (bx, by) = (
        (cos * hx).abs() + (sin * hy).abs(),
        (sin * hx).abs() + (cos * hy).abs(),
    );
    let mut mask = vec![0u8; 256 * 256];
    for p in points {
        let (px, py) = (f32::from_bits(p[0]), f32::from_bits(p[1]));
        for y in (py - by).floor().max(0.0) as usize..((py + by).ceil() as usize).min(256) {
            for x in (px - bx).floor().max(0.0) as usize..((px + bx).ceil() as usize).min(256) {
                let (sx, sy) = (x as f32 + 0.5 - px, y as f32 + 0.5 - py);
                let (dx, dy) = (cos * sx + sin * sy, -sin * sx + cos * sy);
                if dx.abs() > hx || dy.abs() > hy {
                    continue;
                }
                let dist = (dx / 1.5).powi(2) + (dy / 6.0).powi(2);
                let t = ((dist - (1.0 - aa)) / aa).clamp(0.0, 1.0);
                let value = ((1.0 - t * t * (3.0 - 2.0 * t)) * 255.0).round() as u8;
                let i = y * 256 + x;
                mask[i] = mask[i].max(value);
            }
        }
    }
    if let Ok(path) = std::env::var("SDOCX_TAPE_V114_MASK_OUT") {
        std::fs::write(path, &mask).unwrap();
    }
    let errors: Vec<_> = mask
        .iter()
        .zip(oracle.as_chunks::<4>().0)
        .filter_map(|(a, p)| (*a != p[3]).then_some(a.abs_diff(p[3])))
        .collect();
    assert!(errors.is_empty());
}

#[test]
fn v113_profile512_size12_filled_mask_from_source() {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let points = tape_curve(
        &[
            P { x: 13.0, y: 21.0 },
            P { x: 22.0, y: 29.0 },
            P { x: 34.0, y: 18.0 },
            P { x: 47.0, y: 28.0 },
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
        "d448a37f1d7f7f596504492fbd346c06a892a307dc8fd016d9d01c375da69146"
    );
    let mut oracle = Vec::new();
    flate2::read::ZlibDecoder::new(
        &include_bytes!("fixtures/tapepen-v107/profile512-size12-mask.rgba.zlib")[..],
    )
    .read_to_end(&mut oracle)
    .unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&oracle)),
        "ad1ccc047092f3d9405caa1ded122623d53ddfb7866c847a0bd52b43038ba859"
    );
    let level = tape_size_level(12.0, 512, 512);
    assert_eq!(level, 10);
    let actual = filled_mask(
        &points,
        [1.5, 6.0],
        [1.6800001, 6.7200003],
        tape_aa_width(level),
    );
    for y in 0..512 {
        for x in 0..512 {
            let value = if x < 256 && y < 256 {
                actual[y * 256 + x]
            } else {
                0
            };
            assert_eq!(value, oracle[(y * 512 + x) * 4 + 3], "at {x}, {y}");
        }
    }
}

#[test]
fn v115_translated_source_mask() {
    let points = tape_curve(
        &[
            P { x: 64.0, y: 64.0 },
            P { x: 73.0, y: 72.0 },
            P { x: 85.0, y: 61.0 },
            P { x: 98.0, y: 71.0 },
        ],
        0.5,
        0.0,
        [68.0 / 255.0, 170.0 / 255.0, 238.0 / 255.0],
    );
    let aa = tape_aa_width(tape_size_level(12.0, 256, 256));
    let (sin, cos) = f32::from_bits(0x3f06_0a92).sin_cos();
    let (hx, hy) = (1.6800001_f32, 6.7200003_f32);
    let (bx, by) = (
        (cos * hx).abs() + (sin * hy).abs(),
        (sin * hx).abs() + (cos * hy).abs(),
    );
    let mut mask = vec![0u8; 256 * 256];
    for p in points {
        let (px, py) = (f32::from_bits(p[0]), f32::from_bits(p[1]));
        for y in (py - by).floor().max(0.0) as usize..((py + by).ceil() as usize).min(256) {
            for x in (px - bx).floor().max(0.0) as usize..((px + bx).ceil() as usize).min(256) {
                let (sx, sy) = (x as f32 + 0.5 - px, y as f32 + 0.5 - py);
                let (dx, dy) = (cos * sx + sin * sy, -sin * sx + cos * sy);
                if dx.abs() > hx || dy.abs() > hy {
                    continue;
                }
                let dist = (dx / 1.5).powi(2) + (dy / 6.0).powi(2);
                let t = ((dist - (1.0 - aa)) / aa).clamp(0.0, 1.0);
                let value = ((1.0 - t * t * (3.0 - 2.0 * t)) * 255.0).round() as u8;
                let i = y * 256 + x;
                mask[i] = mask[i].max(value);
            }
        }
    }
    use sha2::{Digest, Sha256};
    assert_eq!(
        format!("{:x}", Sha256::digest(&mask)),
        "590fd4f32840d5dad3c218a67881eabe78f06df661732906449b335d9dd38531"
    );
    if let Ok(path) = std::env::var("SDOCX_TAPE_V115_MASK_OUT") {
        std::fs::write(path, mask).unwrap();
    }
}

#[test]
fn v247_scaled_source_mask() {
    let Ok(path) = std::env::var("SDOCX_TAPE_V247_MASK_OUT") else {
        return;
    };
    let scale: usize = std::env::var("SDOCX_TAPE_V247_SCALE")
        .unwrap()
        .parse()
        .unwrap();
    assert!([1, 2].contains(&scale));
    let s = scale as f32;
    let width = 256 * scale;
    let points = tape_curve(
        &[
            P { x: 77., y: 59. },
            P { x: 89., y: 52. },
            P { x: 101., y: 65. },
            P { x: 109., y: 57. },
        ],
        0.5,
        0.0,
        [215. / 255., 68. / 255., 51. / 255.],
    );
    let aa = tape_aa_width(tape_size_level(12., 256, 256));
    let (sin, cos) = f32::from_bits(0x3f06_0a92).sin_cos();
    let (hx, hy) = (1.6800001_f32 * s, 6.7200003_f32 * s);
    let (bx, by) = (
        (cos * hx).abs() + (sin * hy).abs(),
        (sin * hx).abs() + (cos * hy).abs(),
    );
    let mut mask = vec![0u8; width * width];
    for p in points {
        let (px, py) = (f32::from_bits(p[0]) * s, f32::from_bits(p[1]) * s);
        for y in (py - by).floor().max(0.) as usize..((py + by).ceil() as usize).min(width) {
            for x in (px - bx).floor().max(0.) as usize..((px + bx).ceil() as usize).min(width) {
                let (sx, sy) = (x as f32 + 0.5 - px, y as f32 + 0.5 - py);
                let (dx, dy) = (cos * sx + sin * sy, -sin * sx + cos * sy);
                if dx.abs() > hx || dy.abs() > hy {
                    continue;
                }
                let dist = (dx / (1.5 * s)).powi(2) + (dy / (6. * s)).powi(2);
                let t = ((dist - (1. - aa)) / aa).clamp(0., 1.);
                let value = ((1. - t * t * (3. - 2. * t)) * 255.).round() as u8;
                let i = y * width + x;
                mask[i] = mask[i].max(value);
            }
        }
    }
    std::fs::write(path, mask).unwrap();
}

#[test]
#[allow(clippy::excessive_precision)]
fn v248_scaled_source_mask() {
    let Ok(path) = std::env::var("SDOCX_TAPE_V248_MASK_OUT") else {
        return;
    };
    let scale: usize = std::env::var("SDOCX_TAPE_V248_SCALE")
        .unwrap()
        .parse()
        .unwrap();
    assert!([1, 2].contains(&scale));
    let s = scale as f32;
    let width = 256 * scale;
    let points = tape_curve(
        &[
            P { x: 77., y: 59. },
            P { x: 89., y: 52. },
            P { x: 101., y: 65. },
            P { x: 109., y: 57. },
        ],
        0.5,
        0.0,
        [215. / 255., 68. / 255., 51. / 255.],
    );
    let aa = tape_aa_width(tape_size_level(12., width as i32, width as i32));
    let (sin, cos) = f32::from_bits(0x3f06_0a92).sin_cos();
    let (hx, hy) = (
        (if scale == 1 {
            1.6800001_f32
        } else {
            1.7754546403884888_f32
        }) * s,
        (if scale == 1 {
            6.7200003_f32
        } else {
            7.101818561553955_f32
        }) * s,
    );
    let (bx, by) = (
        (cos * hx).abs() + (sin * hy).abs(),
        (sin * hx).abs() + (cos * hy).abs(),
    );
    let mut mask = vec![0u8; width * width];
    for p in points {
        let (px, py) = (f32::from_bits(p[0]) * s, f32::from_bits(p[1]) * s);
        for y in (py - by).floor().max(0.) as usize..((py + by).ceil() as usize).min(width) {
            for x in (px - bx).floor().max(0.) as usize..((px + bx).ceil() as usize).min(width) {
                let (sx, sy) = (x as f32 + 0.5 - px, y as f32 + 0.5 - py);
                let (dx, dy) = (cos * sx + sin * sy, -sin * sx + cos * sy);
                if dx.abs() > hx || dy.abs() > hy {
                    continue;
                }
                let dist = (dx / (1.5 * s)).powi(2) + (dy / (6. * s)).powi(2);
                let t = ((dist - (1. - aa)) / aa).clamp(0., 1.);
                let value = ((1. - t * t * (3. - 2. * t)) * 255.).round() as u8;
                let i = y * width + x;
                mask[i] = mask[i].max(value);
            }
        }
    }
    std::fs::write(path, mask).unwrap();
}

#[test]
fn v249_size_source_mask() {
    let Ok(path) = std::env::var("SDOCX_TAPE_V249_MASK_OUT") else {
        return;
    };
    let size: f32 = std::env::var("SDOCX_TAPE_V249_SIZE")
        .unwrap()
        .parse()
        .unwrap();
    assert!([2., 8., 16.].contains(&size));
    let points = tape_curve(
        &[
            P { x: 13., y: 21. },
            P { x: 22., y: 29. },
            P { x: 34., y: 18. },
            P { x: 47., y: 28. },
        ],
        0.5,
        0.0,
        [68. / 255., 170. / 255., 238. / 255.],
    );
    let aa = tape_aa_width(tape_size_level(size, 256, 256));
    let radii = [size / 8., size / 2.];
    let extent = radii.map(|r| r * (1. + aa));
    std::fs::write(path, filled_mask(&points, radii, extent, aa)).unwrap();
}

#[test]
fn v250_size24_source_mask_matches_native() {
    use std::io::Read;
    let points = tape_curve(
        &[
            P { x: 13., y: 21. },
            P { x: 22., y: 29. },
            P { x: 34., y: 18. },
            P { x: 47., y: 28. },
        ],
        0.5,
        0.0,
        [68. / 255., 170. / 255., 238. / 255.],
    );
    let size = 24.;
    let aa = tape_aa_width(tape_size_level(size, 256, 256));
    let radii = [size / 8., size / 2.];
    let extent = radii.map(|r| r * (1. + aa));
    let actual = filled_mask(&points, radii, extent, aa);
    let mut native = Vec::new();
    flate2::read::ZlibDecoder::new(
        &include_bytes!("../../../../tools/tapepen-public-drawing/v250-size24-mask.alpha.zlib")[..],
    )
    .read_to_end(&mut native)
    .unwrap();
    assert_eq!(actual, native);
}

#[test]
fn v251_changed_geometry_source_mask_matches_native() {
    use std::io::Read;
    let points = tape_curve(
        &[
            P { x: 13., y: 21. },
            P { x: 18., y: 10. },
            P { x: 31., y: 30. },
            P { x: 43., y: 16. },
        ],
        0.5,
        0.0,
        [68. / 255., 170. / 255., 238. / 255.],
    );
    let size = 24.;
    let aa = tape_aa_width(tape_size_level(size, 256, 256));
    let radii = [size / 8., size / 2.];
    let extent = radii.map(|r| r * (1. + aa));
    let actual = filled_mask(&points, radii, extent, aa);
    let mut native = Vec::new();
    flate2::read::ZlibDecoder::new(&include_bytes!("../../../../tools/tapepen-public-drawing/v251-geometry-mask.alpha.zlib")[..])
        .read_to_end(&mut native).unwrap();
    assert_eq!(actual, native);
}
