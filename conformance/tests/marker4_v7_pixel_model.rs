//! Fixed-grid native host controls for the experimental Marker4 V7 shader model.
//! This is not a production renderer or a claim of device/export parity.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{fs::File, io::BufReader, path::Path};

fn assert_sha256(path: &Path, expected: &str) {
    let bytes = std::fs::read(path).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        expected,
        "{}",
        path.display()
    );
}

fn alpha_png(path: &Path) -> (usize, usize, Vec<u8>) {
    let decoder = png::Decoder::new(BufReader::new(File::open(path).unwrap()));
    let mut reader = decoder.read_info().unwrap();
    let mut bytes = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut bytes).unwrap();
    assert_eq!(info.color_type, png::ColorType::Grayscale);
    assert_eq!(info.bit_depth, png::BitDepth::Eight);
    bytes.truncate(info.buffer_size());
    (info.width as usize, info.height as usize, bytes)
}

fn number(v: &Value) -> f64 {
    v.as_f64().unwrap()
}

#[test]
fn captured_rtv3_pixel_controls() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixture = root.join("marker4-v7-pixels");
    assert_sha256(
        &fixture.join("mask.png"),
        "1ad1fa98f80ff50ceb526f92fe43db5f7b9d74950eae9d9468706be03db84f4c",
    );
    let (mw, mh, mask) = alpha_png(&fixture.join("mask.png"));
    assert_eq!((mw, mh), (200, 200));
    let cases: Value =
        serde_json::from_reader(File::open(fixture.join("cases.json")).unwrap()).unwrap();
    let geometry: Value =
        serde_json::from_reader(File::open(root.join("marker4-v7-finger-native.json")).unwrap())
            .unwrap();
    for (index, case) in cases.as_array().unwrap().iter().enumerate() {
        let name = case["case"].as_str().unwrap();
        let dots = geometry["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == name)
            .unwrap()["dots"]
            .as_array()
            .unwrap();
        let native_png = fixture.join(format!("{index}.png"));
        assert_sha256(&native_png, case["png_sha256"].as_str().unwrap());
        let (nw, nh, native) = alpha_png(&native_png);
        let bounds = case["bounds"].as_array().unwrap();
        let left = bounds[0].as_i64().unwrap() as i32;
        let top = bounds[1].as_i64().unwrap() as i32;
        assert_eq!(nw, (bounds[2].as_i64().unwrap() as i32 - left) as usize);
        assert_eq!(nh, (bounds[3].as_i64().unwrap() as i32 - top) as usize);
        assert!(nw <= 128 && nh <= 128 && dots.len() <= 10000);
        let scale = number(&case["scale"]);
        let phase = &case["phase"];
        let center = &case["center"];
        let rotation = case["rotation"].as_f64().unwrap_or(0.0);
        let (rs, rc) = rotation.sin_cos();
        let mut l1 = 0u64;
        let mut max_error = 0u8;
        for py in top - 2..top + nh as i32 + 2 {
            for px in left - 2..left + nw as i32 + 2 {
                let mut output = 0u8;
                for dot in dots {
                    let x = number(&dot[0]);
                    let y = number(&dot[1]);
                    let radius = number(&dot[2]);
                    let angle = number(&dot[3]) + rotation;
                    let tw = 1.54 * (radius + 0.5);
                    let th = 2.0 * (radius + 0.5);
                    let (s, c) = angle.sin_cos();
                    let dx = x - number(&center[0]);
                    let dy = y - number(&center[1]);
                    let sx = 512.0 + scale * (rc * dx - rs * dy) + number(&phase[0]);
                    let sy = 256.0 + scale * (rs * dx + rc * dy) + number(&phase[1]);
                    let xx = px as f64 + 0.5 - sx;
                    let yy = py as f64 + 0.5 - sy;
                    let u = (xx * c + yy * s) / (scale * tw) + 0.5;
                    let v = (-xx * s + yy * c) / (scale * th) + 0.5;
                    if !(0.0..=1.0).contains(&u) || !(0.0..=1.0).contains(&v) {
                        continue;
                    }
                    let tx = (u * 200.0 - 0.5).clamp(0.0, 199.0);
                    let ty = (v * 200.0 - 0.5).clamp(0.0, 199.0);
                    let ix = tx.floor() as usize;
                    let iy = ty.floor() as usize;
                    let fx = tx - ix as f64;
                    let fy = ty - iy as f64;
                    let at = |x: usize, y: usize| mask[y * 200 + x] as f64 / 255.0;
                    let alpha = at(ix, iy) * (1.0 - fx) * (1.0 - fy)
                        + at((ix + 1).min(199), iy) * fx * (1.0 - fy)
                        + at(ix, (iy + 1).min(199)) * (1.0 - fx) * fy
                        + at((ix + 1).min(199), (iy + 1).min(199)) * fx * fy;
                    let dx = (c.abs() + s.abs()) / (scale * tw);
                    let dy = (c.abs() + s.abs()) / (scale * th);
                    let ex = ((0.5 - (u - 0.5).abs()) / dx).clamp(0.0, 1.0);
                    let ey = ((0.5 - (v - 0.5).abs()) / dy).clamp(0.0, 1.0);
                    output = output.max((alpha * ex * ey * 128.0).round() as u8);
                }
                let reference =
                    if px >= left && px < left + nw as i32 && py >= top && py < top + nh as i32 {
                        native[(py - top) as usize * nw + (px - left) as usize]
                    } else {
                        0
                    };
                assert_eq!(output > 0, reference > 0, "{name} support at ({px},{py})");
                let error = output.abs_diff(reference);
                max_error = max_error.max(error);
                l1 += u64::from(error);
            }
        }
        assert!(max_error <= 1, "{name} max error {max_error}");
        assert!(l1 <= case["l1_limit"].as_u64().unwrap(), "{name} L1 {l1}");
    }
}
