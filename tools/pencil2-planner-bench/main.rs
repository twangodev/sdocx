mod ink;
#[derive(Clone, Copy)]
struct Point {
    x: f64,
    y: f64,
}
use std::{hint::black_box, time::Instant};
macro_rules! bench {
    ($module:ident, $path:ident, $points:expr, $iter:expr, $expected:expr) => {{
        let points: Vec<ink::$path::P> = $points
            .iter()
            .map(|p| ink::$path::P { x: p.0, y: p.1 })
            .collect();
        let n = points.len();
        let input = ink::$module::SavedInput {
            advanced_version: 14,
            tool_type: 2,
            source: 0,
            stroke_type: 0,
            curve_enabled: true,
            color_argb: 0xff00baf7,
            size: 9.1168,
            effective_size: 9.1168,
            points,
            pressure: vec![0.5; n],
            tilt: vec![0.; n],
            orientation: vec![0.; n],
            timestamps: vec![0; n],
        };
        let alpha = ink::$module::AlphaState {
            alpha_byte: 255,
            square_pressure_flag: 0,
            base: 9.1168,
            size_level: 1,
        };
        let start = Instant::now();
        let mut result = None;
        for _ in 0..$iter {
            result = match black_box(ink::$module::run(black_box(&input), alpha)) {
                Ok(records) => {
                    assert_eq!(Some(records.len()), $expected);
                    Some(records.len())
                }
                Err(ink::$module::SamplerError::WorkLimit) => {
                    assert_eq!(None, $expected);
                    None
                }
                Err(error) => panic!("unexpected sampler error: {error}"),
            }
        }
        (start.elapsed().as_nanos() as f64 / $iter as f64, result)
    }};
}
fn main() {
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(std::env::args().nth(1).unwrap()).unwrap())
            .unwrap();
    let real: Vec<(f32, f32)> = json["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p["x"].as_f64().unwrap() as f32,
                p["y"].as_f64().unwrap() as f32,
            )
        })
        .collect();
    let dense: Vec<(f32, f32)> = (0..16384).map(|i| (i as f32 * 0.01, 0.)).collect();
    let long = vec![(0., 0.), (6500., 0.)];
    let reject = vec![(0., 0.), (1000000., 0.)];
    for (name, points, iters, expected) in [
        ("real", real, 3000, Some(208)),
        ("dense", dense, 100, Some(410)),
        ("long", long, 80, Some(16252)),
        ("reject", reject, 100, None),
    ] {
        let _ = bench!(before, before_path, points, 10, expected);
        let _ = bench!(after, after_path, points, 10, expected);
        for round in 0..4 {
            let (a, b) = if round % 2 == 0 {
                (
                    bench!(before, before_path, points, iters, expected),
                    bench!(after, after_path, points, iters, expected),
                )
            } else {
                let b = bench!(after, after_path, points, iters, expected);
                let a = bench!(before, before_path, points, iters, expected);
                (a, b)
            };
            assert_eq!(a.1, b.1);
            println!(
                "{name} before_ns={:.0} after_ns={:.0} result={:?}/{:?}",
                a.0, b.0, a.1, b.1
            )
        }
    }
}
