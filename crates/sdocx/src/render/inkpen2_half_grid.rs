use super::{BoundingBox, InkPen2HalfGridViewport, Paint, Scene, Styled, rectangle};
use std::{cell::Cell, collections::BTreeMap};

type GridPixels = BTreeMap<(i32, i32), u8>;

// Budget charges generated stamps, tested fragments and retained grid cells for
// the entire page. A failed stroke does not consume budget or emit paint.
pub(super) fn render(
    scene: &mut Scene,
    ink: &crate::PreparedInkPen2V9,
    view: InkPen2HalfGridViewport,
    budget: &Cell<[usize; 3]>,
) -> bool {
    let Some((pixels, used)) = sample(ink, view, budget.get()) else {
        return false;
    };
    let old = budget.get();
    let remaining = [old[0] - used[0], old[1] - used[1], old[2] - used[2]];
    let color = format!("#{:02x}{:02x}{:02x}", ink.color.r, ink.color.g, ink.color.b);
    for ((x, y), alpha) in pixels {
        let bounds = BoundingBox {
            x_min: f64::from(x) * 2.,
            y_min: f64::from(y) * 2.,
            x_max: f64::from(x + 1) * 2.,
            y_max: f64::from(y + 1) * 2.,
        };
        scene.push(
            rectangle(bounds, 0., 5)
                .fill(Paint::from_hex(&color))
                .fill_opacity(f64::from(alpha) / 255.),
        );
    }
    budget.set(remaining);
    true
}

fn sample(
    ink: &crate::PreparedInkPen2V9,
    view: InkPen2HalfGridViewport,
    limits: [usize; 3],
) -> Option<(GridPixels, [usize; 3])> {
    if !(1..=8192).contains(&view.width)
        || !(1..=8192).contains(&view.height)
        || ink.records.len() > limits[0]
        || ink.per_stamp_rgb
        || ink.enhanced_antialias
    {
        return None;
    }
    let mut pixels = BTreeMap::new();
    let mut work = 0usize;
    for record in &ink.records {
        let (cx, cy, r) = (
            f64::from(record[0]) * 0.5,
            f64::from(record[1]) * 0.5,
            f64::from(record[2]) * 0.5,
        );
        if ![
            cx,
            cy,
            r,
            cx - r - 0.5,
            cx + r - 0.5,
            cy - r - 0.5,
            cy + r - 0.5,
            2. * r,
        ]
        .iter()
        .all(|v| v.is_finite())
            || r <= 0.
        {
            return None;
        }
        let bound = |v: f64, max: u32| v.ceil().clamp(0., f64::from(max)) as i32;
        let (x0, x1, y0, y1) = (
            bound(cx - r - 0.5, view.width),
            bound(cx + r - 0.5, view.width),
            bound(cy - r - 0.5, view.height),
            bound(cy + r - 0.5, view.height),
        );
        work = work.checked_add(((x1 - x0) as usize).checked_mul((y1 - y0) as usize)?)?;
        if work > limits[1] {
            return None;
        }
        let distance =
            |x: i32, y: i32| ((f64::from(x) + 0.5 - cx).hypot(f64::from(y) + 0.5 - cy)) / (2. * r);
        for y in y0..y1 {
            for x in x0..x1 {
                let (qx, qy) = (x & !1, y & !1);
                let d = distance(x, y);
                let w = (distance(qx + 1, y) - distance(qx, y)).abs()
                    + (distance(x, qy + 1) - distance(x, qy)).abs();
                if !d.is_finite() || !w.is_finite() {
                    return None;
                }
                let a = if w == 0. {
                    if d >= 0.5 {
                        return None;
                    }
                    1.
                } else {
                    ((0.5 - d) / w).clamp(0., 1.)
                };
                let previous = f64::from(*pixels.get(&(x, y)).unwrap_or(&0));
                let value = (255. * a + previous * (1. - a)).round_ties_even() as u8;
                if value != 0 {
                    if !pixels.contains_key(&(x, y)) && pixels.len() >= limits[2] {
                        return None;
                    }
                    pixels.insert((x, y), value);
                }
            }
        }
    }
    let used = [ink.records.len(), work, pixels.len()];
    Some((pixels, used))
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::*;
    use base64::Engine as _;
    use std::io::Read as _;

    #[test]
    fn failed_layer_keeps_scene_and_budget_for_later_layer() {
        let ink = crate::PreparedInkPen2V9 {
            records: vec![[8., 8., 4., 4., 0., 1., 0., 0., 0.]],
            sample_ends: vec![],
            color: crate::Color { r: 1, g: 2, b: 3 },
            opacity: 1.,
            per_stamp_rgb: false,
            enhanced_antialias: false,
            bounds: BoundingBox {
                x_min: 0.,
                y_min: 0.,
                x_max: 16.,
                y_max: 16.,
            },
        };
        let view = InkPen2HalfGridViewport {
            width: 16,
            height: 16,
        };
        let budget = Cell::new([2, 1000, 1000]);
        let mut scene = Scene::new(super::super::Svg::new());
        assert!(render(&mut scene, &ink, view, &budget));
        let after_first = budget.get();
        let mut larger = ink.clone();
        larger.records.push(larger.records[0]);
        assert!(!render(&mut scene, &larger, view, &budget));
        assert_eq!(budget.get(), after_first);
        assert!(render(&mut scene, &ink, view, &budget));
        let rendered = scene.finish();
        let mut expected = Scene::new(super::super::Svg::new());
        let fresh = Cell::new([2, 1000, 1000]);
        assert!(render(&mut expected, &ink, view, &fresh));
        assert!(render(&mut expected, &ink, view, &fresh));
        assert_eq!(rendered, expected.finish());
        assert_eq!(budget.get(), fresh.get());
    }

    #[test]
    fn retained_host_frames_and_atomic_budget() {
        let receipt: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tools/inkpen2-v1-tablet-half/receipt.json"
        ))
        .unwrap();
        for (name, case) in receipt["cases"].as_object().unwrap() {
            let hex = case["geometry_hex"].as_str().unwrap();
            let bytes: Vec<_> = hex
                .as_bytes()
                .as_chunks::<2>().0.iter()
                .map(|v| u8::from_str_radix(std::str::from_utf8(v).unwrap(), 16).unwrap())
                .collect();
            let records: Vec<_> = bytes
                .as_chunks::<12>().0.iter()
                .map(|v| {
                    let f = |i| f32::from_le_bytes(v[i..i + 4].try_into().unwrap());
                    [f(0) + 32., f(4) + 32., f(8), f(8), 0., 1., 0., 0., 0.]
                })
                .collect();
            let ink = crate::PreparedInkPen2V9 {
                records,
                sample_ends: vec![],
                color: crate::Color { r: 0, g: 0, b: 0 },
                opacity: 1.,
                per_stamp_rgb: false,
                enhanced_antialias: false,
                bounds: BoundingBox {
                    x_min: 0.,
                    y_min: 0.,
                    x_max: 128.,
                    y_max: 96.,
                },
            };
            let view = InkPen2HalfGridViewport {
                width: 64,
                height: 48,
            };
            let (pixels, used) = sample(&ink, view, [32768, 1000000, 100000]).unwrap();
            for i in 0..3 {
                if used[i] > 0 {
                    let mut short = used;
                    short[i] -= 1;
                    assert!(sample(&ink, view, short).is_none(), "{name} budget {i}");
                }
            }
            let compressed = base64::engine::general_purpose::STANDARD
                .decode(case["rgba_zlib_base64"].as_str().unwrap())
                .unwrap();
            let mut rgba = Vec::new();
            flate2::read::ZlibDecoder::new(compressed.as_slice())
                .read_to_end(&mut rgba)
                .unwrap();
            let mut expected = vec![0; 64 * 48];
            for ((x, y), a) in pixels {
                expected[y as usize * 64 + x as usize] = a;
            }
            let error: u64 = rgba
                .as_chunks::<4>().0.iter()
                .zip(&expected)
                .map(|(p, &a)| u64::from(p[3].abs_diff(a)))
                .sum();
            assert!(error <= 100, "{name} alpha L1 {error}");
        }
    }
}
