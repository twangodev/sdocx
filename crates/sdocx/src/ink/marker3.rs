//! Saved Marker3 V5 centerline stamps, measured against the pinned native
//! ObjectStroke redraw. The mask and composite shaders remain separate.
use super::path::{P, Quad};
use crate::{Point, Stroke};

const PEN: &str = "com.samsung.android.sdk.pen.pen.preload.Marker3";
const STRAIGHT_PEN: &str = "com.samsung.android.sdk.pen.pen.preload.StraightMarker";
const SPACING: f32 = 1.0;
const REJECT_BELOW: f32 = 2.0;
const ALTERNATE_BELOW: f32 = 20.0;

pub(crate) struct Ink {
    pub points: Vec<Point>,
    pub radii: Vec<f64>,
    /// Native redraw callback boundaries; a one-point stroke has `[1, 2]`
    /// because its initial drawPoint and endPen terminal point are separate.
    /// These are not PreparedStroke replay sample attribution boundaries.
    pub sample_ends: Vec<usize>,
}

pub(crate) fn prepare(s: &Stroke) -> Option<Ink> {
    let r = s.rendering.as_ref()?;
    if !matches!(r.pen_name.as_deref(), Some(PEN | STRAIGHT_PEN))
        // Drawing admits null settings to Marker3 V1; this V5 sampler is only
        // valid for the observed empty and explicit `5;` settings.
        || !matches!(r.advanced_settings.as_deref(), Some("" | "5;"))
        || r.properties.eraser
        || r.properties.rainbow_effect
        || r.properties.straighten
        || !matches!(r.tool_type_raw, 1..=3)
        || s.points.is_empty()
        || s.points.len() > 100_000
        || !s.pen_width.is_finite()
        || !(0.4..=800.0).contains(&s.pen_width)
        || s.points.iter().any(|p| {
            !p.x.is_finite() || !p.y.is_finite() || p.x.abs() > 1e7 || p.y.abs() > 1e7
        })
    {
        return None;
    }
    let total: f64 = s
        .points
        .windows(2)
        .map(|w| (w[1].x - w[0].x).hypot(w[1].y - w[0].y))
        .sum();
    if !total.is_finite() || total > 400_000. {
        return None;
    }

    let size = s.pen_width.clamp(0.4, 800.0).trunc();
    let radius = f64::from(size.mul_add(0.5, 0.5));
    let mut points = Vec::new();
    let mut sample_ends = vec![0; s.points.len().max(2)];
    let mut previous = P::from(s.points[0]);
    let mut midpoint = previous;
    let mut residual = SPACING;
    // Initial drawPoint sets the alternate flag before saved samples are read.
    let mut alternate = true;
    points.push(s.points[0]);
    sample_ends[0] = 1;

    for (i, point) in s.points.iter().enumerate().skip(1) {
        let next = P::from(*point);
        let last = i + 1 == s.points.len();
        let distance = next.sub(previous).len();
        let draw = if distance < REJECT_BELOW {
            false
        } else if r.tool_type_raw == 1 && distance < ALTERNATE_BELOW {
            let old = alternate;
            alternate = !alternate;
            !old
        } else {
            alternate = true;
            true
        };
        if draw {
            // Saved redraw moves to the previous midpoint and uses the
            // previous sample as control. Intermediate edges end at their
            // midpoint; the final saved edge ends at the sample itself.
            let end = if last { next } else { previous.mid(next) };
            let q = Quad::new(midpoint, previous, end);
            let mut d = residual;
            while q.length > 0.0 && d <= q.length {
                let p = q.at(d);
                points.push(Point {
                    x: p.x as f64,
                    y: p.y as f64,
                });
                d += SPACING;
            }
            residual = d - q.length;
            midpoint = end;
            previous = next;
        }
        sample_ends[i] = points.len();
    }
    // endPen reads the terminal coordinates through native
    // MotionEvent.GetX/GetY and emits that endpoint as a point stamp.
    points.push(*s.points.last()?);
    if let Some(end) = sample_ends.last_mut() {
        *end = points.len();
    }
    let radii = vec![radius; points.len()];
    Some(Ink {
        points,
        radii,
        sample_ends,
    })
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::*;
    use crate::{BoundingBox, StrokeProperties, StrokeRendering, StrokeStyle};
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Fixture {
        cases: Vec<Case>,
    }

    #[derive(Deserialize)]
    struct Case {
        id: String,
        input: Input,
        native: Native,
    }

    #[derive(Deserialize)]
    struct Input {
        samples: Vec<[f64; 2]>,
        width: f32,
        tool_type_raw: u16,
        is_shape: bool,
        fixed_width: bool,
        wrapper_flag: bool,
    }

    #[derive(Deserialize)]
    struct Native {
        sample_ends: Vec<usize>,
        stamps: Vec<[f32; 3]>,
    }

    fn fixture() -> Fixture {
        serde_json::from_str(include_str!(
            "../../../../conformance/marker3-saved-native.json"
        ))
        .unwrap()
    }

    fn stroke(input: &Input) -> Stroke {
        Stroke {
            rendering: Some(StrokeRendering {
                metadata: None,
                pen_name: Some(PEN.to_owned()),
                advanced_settings: Some("5;".into()),
                tool_type_raw: input.tool_type_raw,
                properties: StrokeProperties {
                    compressed: false,
                    replay_only: false,
                    stylus_channels: false,
                    eraser: false,
                    fixed_width: input.fixed_width,
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
                    color_argb: Some(0xff33_6699),
                    ..StrokeStyle::default()
                },
            }),
            bbox: BoundingBox::default(),
            points: input
                .samples
                .iter()
                .map(|[x, y]| Point { x: *x, y: *y })
                .collect(),
            pressures: Vec::new(),
            timestamps: Vec::new(),
            tilts: Vec::new(),
            orientations: Vec::new(),
            color: None,
            pen_width: input.width,
        }
    }

    #[test]
    fn v5_positions_radii_and_sample_boundaries_match_native_bits() {
        let fixture = fixture();
        let mut supported = 0;
        for case in fixture.cases {
            // The wrapped redraw mode emits no vector stamps, and IsShape is
            // not represented by this Stroke input contract.
            if case.input.wrapper_flag || case.input.is_shape {
                continue;
            }
            supported += 1;
            let ink = prepare(&stroke(&case.input)).unwrap();
            assert_eq!(ink.sample_ends, case.native.sample_ends);
            assert_eq!(ink.points.len(), case.native.stamps.len());
            assert_eq!(ink.radii.len(), case.native.stamps.len());
            for ((point, radius), [x, y, native_radius]) in
                ink.points.iter().zip(&ink.radii).zip(&case.native.stamps)
            {
                assert_eq!((point.x as f32).to_bits(), x.to_bits());
                assert_eq!((point.y as f32).to_bits(), y.to_bits());
                assert_eq!((*radius as f32).to_bits(), native_radius.to_bits());
            }
        }
        assert_eq!(supported, 10, "native fixture support coverage changed");
    }

    fn f32_from_bits_hex(value: &str) -> f32 {
        let bytes = bytes_from_hex(value);
        f32::from_bits(u32::from_le_bytes(bytes.try_into().unwrap()))
    }

    fn bytes_from_hex(value: &str) -> Vec<u8> {
        value
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }

    fn f32_array_from_hex(value: &str) -> Vec<f32> {
        bytes_from_hex(value)
            .as_chunks::<4>()
            .0
            .iter()
            .map(|bytes| f32::from_bits(u32::from_le_bytes(*bytes)))
            .collect()
    }

    fn caller_fixture() -> serde_json::Value {
        serde_json::from_str(include_str!(
            "../../../../conformance/marker3-saved-caller-native.json"
        ))
        .unwrap()
    }

    fn caller_stroke(case: &serde_json::Value) -> Stroke {
        let input = &case["input"];
        let source = &case["native_calls"]["source_ObjectStroke_arrays"];
        let source_xy = f32_array_from_hex(source["points"]["data_hex"].as_str().unwrap());
        let points = source_xy
            .as_chunks::<2>()
            .0
            .iter()
            .map(|xy| Point {
                x: f64::from(xy[0]),
                y: f64::from(xy[1]),
            })
            .collect::<Vec<_>>();
        let pressures: Vec<f64> =
            f32_array_from_hex(source["pressures"]["data_hex"].as_str().unwrap())
                .into_iter()
                .map(f64::from)
                .collect();
        let timestamps = bytes_from_hex(source["timestamps"]["data_hex"].as_str().unwrap())
            .as_chunks::<4>()
            .0
            .iter()
            .map(|bytes| i64::from(i32::from_le_bytes(*bytes)))
            .collect();
        let tilts = f32_array_from_hex(source["tilts"]["data_hex"].as_str().unwrap())
            .into_iter()
            .map(f64::from)
            .collect();
        let orientations = f32_array_from_hex(source["orientations"]["data_hex"].as_str().unwrap())
            .into_iter()
            .map(f64::from)
            .collect();

        let input_points = input["points_f32_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|xy| {
                [
                    f32_from_bits_hex(xy[0].as_str().unwrap()),
                    f32_from_bits_hex(xy[1].as_str().unwrap()),
                ]
            })
            .collect::<Vec<_>>();
        assert_eq!(source_xy.len(), input_points.len() * 2);
        for (actual, expected) in source_xy.iter().zip(input_points.iter().flatten()) {
            assert_eq!(actual.to_bits(), expected.to_bits());
        }
        assert_eq!(
            pressures.len(),
            source["pressures"]["count"].as_u64().unwrap() as usize
        );

        let advanced_settings = input["advanced_setting"].as_str().map(str::to_owned);
        let width = f32_from_bits_hex(input["pen_size_f32_bits"].as_str().unwrap());
        Stroke {
            rendering: Some(StrokeRendering {
                metadata: None,
                pen_name: Some(input["pen_name"].as_str().unwrap().to_owned()),
                advanced_settings,
                tool_type_raw: input["tool_type"].as_u64().unwrap() as u16,
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
                    ..StrokeStyle::default()
                },
            }),
            bbox: BoundingBox::default(),
            points,
            pressures,
            timestamps,
            tilts,
            orientations,
            color: None,
            pen_width: width,
        }
    }

    fn assert_caller_xy_matches(ink: &Ink, case: &serde_json::Value) {
        let output = bytes_from_hex(
            case["phase_final_vectors"]["saved_redraw"]["vector_bytes"]
                .as_str()
                .unwrap(),
        );
        assert_eq!(output.len() % 8, 0);
        assert_eq!(ink.points.len(), output.len() / 8);
        for (point, pair) in ink.points.iter().zip(output.as_chunks::<8>().0) {
            let x = f32::from_bits(u32::from_le_bytes(pair[..4].try_into().unwrap()));
            let y = f32::from_bits(u32::from_le_bytes(pair[4..].try_into().unwrap()));
            assert_eq!((point.x as f32).to_bits(), x.to_bits());
            assert_eq!((point.y as f32).to_bits(), y.to_bits());
        }
    }

    fn assert_native_sample_ends(ink: &Ink, case_id: &str) {
        let fixture: Fixture = serde_json::from_str(include_str!(
            "../../../../conformance/marker3-saved-native.json"
        ))
        .unwrap();
        let native_case = fixture
            .cases
            .into_iter()
            .find(|case| case.id == case_id)
            .unwrap();
        assert_eq!(ink.sample_ends, native_case.native.sample_ends);
    }

    #[test]
    fn actual_drawing_empty_and_v5_caller_cases_match_complete_v5_xy_and_boundaries() {
        let fixture = caller_fixture();
        let cases = &fixture["cases"];
        for setting in ["empty", "5"] {
            let key = format!("marker3_linked_saved1:{setting}:tool2:curve1");
            let case = &cases[&key];
            assert_eq!(
                case["native_calls"]["route"]["drawing_selected_drawable_version"],
                "5"
            );
            let expected_setting = if setting == "empty" { "" } else { "5;" };
            assert_eq!(
                case["native_calls"]["ObjectStroke_GetAdvancedPenSetting"]["utf8"],
                expected_setting
            );
            let stroke = caller_stroke(case);
            let ink = prepare(&stroke).unwrap();
            assert_caller_xy_matches(&ink, case);
            assert_native_sample_ends(&ink, "marker3-v5-saved-1");
        }
    }

    #[test]
    fn actual_drawing_null_v1_caller_is_not_admitted_to_v5_sampler() {
        let fixture = caller_fixture();
        let case = &fixture["cases"]["marker3_linked_saved1:null:tool2:curve1"];
        assert_eq!(
            case["native_calls"]["route"]["drawing_selected_drawable_version"],
            "1"
        );
        assert!(
            caller_stroke(case)
                .rendering
                .unwrap()
                .advanced_settings
                .is_none()
        );
        assert!(prepare(&caller_stroke(case)).is_none());
    }

    #[test]
    fn actual_drawing_nontrivial_v5_caller_matches_complete_xy_and_boundaries() {
        let fixture = caller_fixture();
        let case = &fixture["cases"]["marker3_nontrivial_saved3:5:tool2:curve1"];
        assert_eq!(
            case["native_calls"]["route"]["drawing_selected_drawable_version"],
            "5"
        );
        let ink = prepare(&caller_stroke(case)).unwrap();
        assert_caller_xy_matches(&ink, case);
        assert_native_sample_ends(&ink, "marker3-v5-saved-3");
    }
}
