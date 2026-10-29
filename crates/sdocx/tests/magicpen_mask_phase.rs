use serde_json::Value;
use sha2::{Digest, Sha256};

fn distance(x: i32, y: i32, center: (f64, f64)) -> f64 {
    (x as f64 + 0.5 - center.0).hypot(y as f64 + 0.5 - center.1)
}

fn mask_alpha(x: i32, y: i32, center: (f64, f64), radius: f64) -> u8 {
    let px = x as f64 + 0.5;
    let py = y as f64 + 0.5;
    if !(center.0 - radius..center.0 + radius).contains(&px)
        || !(center.1 - radius..center.1 + radius).contains(&py)
    {
        return 0;
    }

    let quad_x = x.div_euclid(2) * 2;
    let quad_y = y.div_euclid(2) * 2;
    let width = (distance(quad_x + 1, y, center) - distance(quad_x, y, center)).abs()
        + (distance(x, quad_y + 1, center) - distance(x, quad_y, center)).abs();
    let remaining = radius - distance(x, y, center);
    let coverage = if width == 0.0 {
        // Host fit for these controls, not a portable zero-derivative GLSL rule.
        f64::from(remaining > 0.0)
    } else {
        (remaining / width).clamp(0.0, 1.0)
    };
    (coverage * 255.0).round() as u8
}

#[test]
fn finite_quad_mask_matches_minimum_dot_host_controls() {
    let receipt: Value = serde_json::from_str(include_str!(
        "../../../tools/magicpen-phase/mask/receipt.json"
    ))
    .unwrap();
    let cases = receipt["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 48);
    let mut mismatches = Vec::new();
    let mut tested = [[false; 16]; 3];

    for case in cases {
        let name = case["case"].as_str().unwrap();
        let mut phase = name.strip_prefix("phase_").unwrap().split('_');
        let phase_x: usize = phase.next().unwrap().parse().unwrap();
        let phase_y: usize = phase.next().unwrap().parse().unwrap();
        assert!(phase.next().is_none() && phase_x < 4 && phase_y < 4);
        let scale = case["scale"].as_u64().unwrap() as usize;
        let scale_index = match scale {
            1 => 0,
            2 => 1,
            4 => 2,
            _ => panic!("unexpected scale {scale}"),
        };
        assert!(!std::mem::replace(
            &mut tested[scale_index][phase_x * 4 + phase_y],
            true
        ));
        let size = 32 * scale;
        let mut actual = vec![0_u8; size * size];
        for pixel in case["nonzero_pixels"].as_array().unwrap() {
            let pixel = pixel.as_array().unwrap();
            let x = pixel[0].as_u64().unwrap() as usize;
            let y = pixel[1].as_u64().unwrap() as usize;
            assert!(x < size && y < size);
            assert_eq!(
                &pixel[2..5],
                &[Value::from(0), Value::from(0), Value::from(0)]
            );
            let alpha = pixel[5].as_u64().unwrap() as u8;
            assert_ne!(alpha, 0);
            assert_eq!(std::mem::replace(&mut actual[y * size + x], alpha), 0);
        }
        let mut gl_rgba = vec![0_u8; size * size * 4];
        for y in 0..size {
            for x in 0..size {
                gl_rgba[((size - 1 - y) * size + x) * 4 + 3] = actual[y * size + x];
            }
        }
        assert_eq!(
            format!("{:x}", Sha256::digest(&gl_rgba)),
            case["mask_sha256"].as_str().unwrap(),
            "mask receipt hash for {name} at {scale}x"
        );
        let center = (
            (8.0 + phase_x as f64 / 4.0) * scale as f64,
            (8.0 + phase_y as f64 / 4.0) * scale as f64,
        );
        for y in 0..size {
            for x in 0..size {
                let expected = actual[y * size + x];
                let modeled = mask_alpha(x as i32, y as i32, center, scale as f64 / 2.0);
                if expected != modeled {
                    mismatches.push((scale, phase_x, phase_y, x, y, expected, modeled));
                }
            }
        }
    }
    assert!(tested.iter().flatten().all(|tested| *tested));
    assert_eq!(
        mismatches,
        vec![(1, 1, 3, 8, 8, 28, 27), (1, 3, 1, 8, 8, 28, 27)]
    );
}
