//! Saved Marker2 geometry, traced from Samsung Notes 4.4.45.37.
//!
//! V1 and V2 share this saved redraw. Marker2's integer RT texture width is
//! produced by `PenDrawableRT::SetPenData` with FCVTZU. The pinned native
//! caller shows omitted and explicit-zero ObjectStroke sizes become 0.4 at
//! `Pen::GetSize`, then integer width 0; the RT quad collapses. Keep the native
//! XY stream with zero-radius stamps so it does not become a visible generic
//! width fallback. V2's `<3` alpha adjustment does not expand that quad.
//! Arithmetic stays float32 where the native implementation uses float32.
use super::path::{P, Quad};
use crate::{Point, Stroke};

const SPACING: f32 = 1.;
const REJECT_BELOW: f32 = 2.;
const ALTERNATE_BELOW: f32 = 20.;
const PEN: &str = "com.samsung.android.sdk.pen.pen.preload.Marker2";

pub(super) struct MarkerInk {
    pub(super) points: Vec<Point>,
    pub(super) radii: Vec<f64>,
    pub(super) sample_ends: Vec<usize>,
    pub(super) opacity: f64,
}

struct Pass {
    previous: P,
    midpoint: P,
    residual: f32,
    alternate: bool,
    skip_short: bool,
    radius: f64,
    points: Vec<Point>,
    radii: Vec<f64>,
}

impl Pass {
    fn stamp(&mut self, p: P) {
        self.points.push(Point {
            x: p.x as f64,
            y: p.y as f64,
        });
        self.radii.push(self.radius);
    }

    fn line(&mut self, p: P) {
        let distance = p.sub(self.previous).len();
        if distance < REJECT_BELOW {
            return;
        }
        if self.skip_short && distance < ALTERNATE_BELOW {
            // The first accepted short move is dropped. The next one is kept.
            self.alternate = !self.alternate;
            if self.alternate {
                return;
            }
        }
        let mid = self.previous.mid(p);
        let q = Quad::new(self.midpoint, self.previous, mid);
        let mut d = self.residual;
        if q.length > 0. {
            while d <= q.length {
                self.stamp(q.at(d));
                d += SPACING;
            }
        }
        self.residual = d - q.length;
        self.previous = p;
        self.midpoint = mid;
    }
}

pub(super) fn prepare(s: &Stroke) -> Option<MarkerInk> {
    let r = s.rendering.as_ref()?;
    if r.pen_name.as_deref() != Some(PEN)
        || !accepted_settings(r.advanced_settings.as_deref())
        || r.properties.eraser
        || r.properties.straighten
        || r.properties.rainbow_effect
        || s.points.is_empty()
        || s.points
            .iter()
            .any(|p| !p.x.is_finite() || !p.y.is_finite() || p.x.abs() > 1e7 || p.y.abs() > 1e7)
    {
        return None;
    }
    let distance: f64 = s
        .points
        .windows(2)
        .map(|p| (p[1].x - p[0].x).hypot(p[1].y - p[0].y))
        .sum();
    if distance > 400_000. || s.points.len() > 100_000 {
        return None;
    }
    // `Stroke.pen_width` is a presentation fallback (0.8 when serialized
    // size is absent). Marker2 reads ObjectStroke::GetPenSize instead, whose
    // absent default is 0.0; Drawing's SetSize clamps to 0.4 and RTV1/RTV2
    // SetPenData truncates that to integer width 0. Keep the generated XY
    // callbacks, but their zero-radius stamps must not fall through to that
    // visible generic fallback.
    let requested_size = r.style.pen_size.unwrap_or(0.0);
    if !requested_size.is_finite() {
        return None;
    }
    let native_width = requested_size.clamp(0.4, 800.).trunc();
    let radius = f64::from(native_width) / 2.0;
    let first = P::from(s.points[0]);
    let mut pass = Pass {
        previous: first,
        midpoint: first,
        residual: SPACING,
        alternate: false,
        // Tool type 1 enables the short-move skip. Tool type 2 also requires
        // MotionEvent source 0x1002, which the saved tool/input word does not
        // contain, so a stored stylus stroke does not alternate.
        skip_short: r.tool_type_raw == 1,
        radius,
        points: Vec::new(),
        radii: Vec::new(),
    };
    pass.stamp(first);
    let mut sample_ends = vec![0; s.points.len()];
    sample_ends[0] = pass.points.len();
    for (index, point) in s.points.iter().enumerate().skip(1) {
        pass.line((*point).into());
        sample_ends[index] = pass.points.len();
    }
    Some(MarkerInk {
        points: pass.points,
        radii: pass.radii,
        sample_ends,
        opacity: opacity(r.style.color_argb),
    })
}

fn accepted_settings(settings: Option<&str>) -> bool {
    let Some(settings) = settings else {
        return true;
    };
    settings
        .split(';')
        .next()
        .unwrap_or("")
        .parse::<i32>()
        .is_ok_and(|value| value >= 0)
}

fn opacity(argb: Option<u32>) -> f64 {
    argb.map_or(1., |argb| f64::from(argb >> 24) / 255.)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BoundingBox, StrokeProperties, StrokeRendering, StrokeStyle};

    fn stroke(points: &[(f64, f64)], settings: Option<&str>, tool: u16) -> Stroke {
        Stroke {
            rendering: Some(StrokeRendering {
                metadata: None,
                pen_name: Some(PEN.into()),
                advanced_settings: settings.map(str::to_string),
                tool_type_raw: tool,
                properties: StrokeProperties {
                    compressed: false,
                    replay_only: false,
                    stylus_channels: false,
                    eraser: false,
                    fixed_width: false,
                    millisecond_timestamps: false,
                    top_layer_pen: true,
                    alpha_lock: false,
                    binary_added: true,
                    generated: false,
                    fixed_opacity: false,
                    rainbow_effect: false,
                    straighten: false,
                    reveal_mode: false,
                },
                style: StrokeStyle {
                    color_argb: Some(0x80ff_ee00),
                    pen_size: Some(4.9),
                    ..StrokeStyle::default()
                },
            }),
            bbox: BoundingBox::default(),
            points: points
                .iter()
                .copied()
                .map(|(x, y)| Point { x, y })
                .collect(),
            pressures: vec![0.2; points.len()],
            timestamps: vec![],
            tilts: vec![],
            orientations: vec![],
            color: Some(crate::Color {
                r: 255,
                g: 238,
                b: 0,
            }),
            pen_width: 4.9,
        }
    }

    #[test]
    fn two_point_redraw_matches_the_traced_chord() {
        let ink = prepare(&stroke(&[(0., 0.), (4., 0.)], Some("2;"), 2)).unwrap();
        assert_eq!(ink.sample_ends, vec![1, 3]);
        assert!(ink.points.iter().all(|p| p.y.abs() < 1e-6));
        let xs: Vec<f64> = ink.points.iter().map(|p| p.x).collect();
        assert!((xs[0] - 0.).abs() < 1e-6, "{xs:?}");
        assert!((xs[1] - 0.499_999_642_4).abs() < 1e-7, "{xs:?}");
        assert!((xs[2] - 1.999_998_569_5).abs() < 1e-7, "{xs:?}");
        assert!(xs.iter().all(|x| (*x - 4.).abs() > 0.1));
        assert!(ink.radii.iter().all(|r| (*r - 2.).abs() < 1e-9));
        assert!((ink.opacity - 128. / 255.).abs() < 1e-12);
    }

    #[test]
    fn short_move_skip_drops_the_first_move() {
        let ink = prepare(&stroke(&[(0., 0.), (4., 0.)], None, 1)).unwrap();
        assert_eq!(ink.sample_ends, vec![1, 1]);
        assert_eq!(ink.points.len(), 1);
        assert!(ink.points[0].x.abs() < 1e-6);
    }

    #[test]
    fn pressure_and_v2_settings_do_not_change_the_stamps() {
        let mut light = stroke(&[(0., 0.), (4., 0.)], Some("2;"), 2);
        light.pressures = vec![0.05, 0.95];
        let heavy = stroke(&[(0., 0.), (4., 0.)], Some("18;"), 2);
        let light = prepare(&light).unwrap();
        let heavy = prepare(&heavy).unwrap();
        assert_eq!(light.points, heavy.points);
        assert_eq!(light.radii, heavy.radii);
    }

    #[test]
    fn native_style_size_controls_width_and_rejects_non_finite_size() {
        let mut source_sized = stroke(&[(0., 0.), (4., 0.)], Some("2;"), 2);
        source_sized.rendering.as_mut().unwrap().style.pen_size = Some(10.);
        source_sized.pen_width = 99.;
        let ink = prepare(&source_sized).unwrap();
        assert!(!ink.radii.is_empty());
        assert!(ink.radii.iter().all(|radius| *radius == 5.));

        let mut non_finite = source_sized;
        non_finite.rendering.as_mut().unwrap().style.pen_size = Some(f32::NAN);
        assert!(prepare(&non_finite).is_none());
    }

    #[test]
    fn other_pens_and_unresolved_settings_stay_approximate() {
        let mut other = stroke(&[(0., 0.), (4., 0.)], Some("2;"), 2);
        other.rendering.as_mut().unwrap().pen_name =
            Some("com.samsung.android.sdk.pen.pen.preload.Marker".into());
        assert!(prepare(&other).is_none());
        assert!(prepare(&stroke(&[(0., 0.), (4., 0.)], Some(""), 2)).is_none());
        assert!(prepare(&stroke(&[(0., 0.), (4., 0.)], Some("-1;"), 2)).is_none());
        let mut eraser = stroke(&[(0., 0.), (4., 0.)], Some("2;"), 2);
        eraser.rendering.as_mut().unwrap().properties.eraser = true;
        assert!(prepare(&eraser).is_none());
        assert!(prepare(&stroke(&[(0., 0.), (400_001., 0.)], Some("2;"), 2)).is_none());
    }

    fn native_fixture() -> serde_json::Value {
        serde_json::from_str(include_str!(
            "../../../../conformance/marker2-saved-caller-native.json"
        ))
        .unwrap()
    }

    fn native_f32(value: &serde_json::Value) -> f32 {
        let text = value.as_str().unwrap();
        let bytes: [u8; 4] = std::array::from_fn(|index| {
            u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).unwrap()
        });
        f32::from_le_bytes(bytes)
    }

    fn caller_stroke(input: &serde_json::Value) -> Stroke {
        let points = input["points_f32_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|point| Point {
                x: f64::from(native_f32(&point[0])),
                y: f64::from(native_f32(&point[1])),
            })
            .collect::<Vec<_>>();
        let curve_enabled = input["curve_enabled"].as_bool().unwrap();
        let requested_size = native_f32(&input["pen_size_f32_bits"]);
        let size_is_omitted = input["size_input"] == "omitted";
        Stroke {
            rendering: Some(StrokeRendering {
                metadata: None,
                pen_name: Some(PEN.into()),
                advanced_settings: input["advanced_setting"].as_str().map(str::to_string),
                tool_type_raw: input["tool_type"].as_u64().unwrap() as u16,
                properties: StrokeProperties {
                    compressed: curve_enabled,
                    replay_only: false,
                    stylus_channels: false,
                    eraser: false,
                    fixed_width: false,
                    millisecond_timestamps: false,
                    top_layer_pen: true,
                    alpha_lock: false,
                    binary_added: true,
                    generated: false,
                    fixed_opacity: false,
                    rainbow_effect: false,
                    straighten: false,
                    reveal_mode: false,
                },
                style: StrokeStyle {
                    color_argb: Some(
                        u32::from_str_radix(
                            input["color_argb"]
                                .as_str()
                                .unwrap()
                                .trim_start_matches("0x"),
                            16,
                        )
                        .unwrap(),
                    ),
                    pen_size: (!size_is_omitted).then_some(requested_size),
                    stroke_type_raw: Some(input["stroke_type"].as_u64().unwrap() as u16),
                    ..StrokeStyle::default()
                },
            }),
            bbox: BoundingBox::default(),
            points,
            pressures: input["pressure_f32_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(native_f32)
                .map(f64::from)
                .collect(),
            timestamps: input["timestamps_i32"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_i64().unwrap())
                .collect(),
            tilts: input["tilt_f32_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(native_f32)
                .map(f64::from)
                .collect(),
            orientations: input["orientation_f32_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(native_f32)
                .map(f64::from)
                .collect(),
            color: None,
            // This is deliberately the serialized rendering fallback; native
            // Marker2 sizing comes from `style.pen_size` above.
            pen_width: if size_is_omitted { 0.8 } else { requested_size },
        }
    }

    #[test]
    fn matches_native_caller_xy_words_and_sample_boundaries_for_all_sizes() {
        let fixture = native_fixture();
        let cases = fixture["cases"].as_object().unwrap();
        assert_eq!(cases.len(), 5, "fixture must retain all native controls");

        for (case_id, case) in cases {
            let ink = prepare(&caller_stroke(&case["input"])).expect(case_id);
            let native_hex = case["phase_final_vectors"]["saved_redraw"]["vector_bytes"]
                .as_str()
                .unwrap();
            assert_eq!(native_hex.len() % 8, 0, "{case_id} native XY stream");
            let expected_words = native_hex
                .as_bytes()
                .as_chunks::<8>()
                .0
                .iter()
                .map(|word| {
                    let text = std::str::from_utf8(word).unwrap();
                    let bytes: [u8; 4] = std::array::from_fn(|index| {
                        u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).unwrap()
                    });
                    u32::from_le_bytes(bytes)
                })
                .collect::<Vec<_>>();
            assert_eq!(ink.points.len() * 2, expected_words.len(), "{case_id}");
            for (index, (point, expected)) in ink
                .points
                .iter()
                .flat_map(|point| [point.x as f32, point.y as f32])
                .zip(expected_words)
                .enumerate()
            {
                assert_eq!(point.to_bits(), expected, "{case_id} XY word {index}");
            }

            let mut native_ends = vec![1];
            native_ends.extend(
                case["sample_boundaries_from_native_drawLine_returns"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|row| row["phase"] == "saved_redraw")
                    .map(|row| row["sample_end"].as_u64().unwrap() as usize),
            );
            assert_eq!(ink.sample_ends, native_ends, "{case_id} sample boundaries");

            let native_pen_size = native_f32(&serde_json::Value::String(
                case["native_calls"]["pen_GetSize_after_draw_f32_bits"]
                    .as_str()
                    .unwrap()
                    .to_string(),
            ));
            let native_width = native_pen_size as u32;
            let expected_radius = f64::from(native_width) / 2.0;
            assert!(
                ink.radii.iter().all(|radius| *radius == expected_radius),
                "{case_id} native integer width {native_width}"
            );
            if native_width == 0 {
                assert_eq!(
                    native_pen_size.to_bits(),
                    0x3ecc_cccd,
                    "{case_id} clamped size"
                );
                assert!(ink.radii.iter().all(|radius| *radius == 0.0), "{case_id}");
            }
        }
    }
}
