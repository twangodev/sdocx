//! Saved Marker3 V5 centerline stamps, measured against the pinned native
//! ObjectStroke redraw. The mask and composite shaders remain separate.
use super::path::{P, Quad};
use crate::{Point, Stroke};

const PEN: &str = "com.samsung.android.sdk.pen.pen.preload.Marker3";
const STRAIGHT_PEN: &str = "com.samsung.android.sdk.pen.pen.preload.StraightMarker";
const SPACING: f32 = 1.0;
const ALTERNATE_BELOW: f32 = 20.0;
const MAX_PAGE_STAMPS: usize = 65_536;

pub(crate) struct Ink {
    pub points: Vec<Point>,
    pub radii: Vec<f64>,
    /// Native redraw callback boundaries. Drawing does not select the Marker3
    /// V5 route for strokes with fewer than two source events; admitted streams
    /// therefore also delimit the captured replay prefixes.
    pub sample_ends: Vec<usize>,
    /// Source alpha is applied once to the compound stroke path.
    pub opacity: f64,
}

#[cfg(all(test, feature = "serde"))]
pub(crate) fn prepare(s: &Stroke) -> Option<Ink> {
    prepare_bounded(s, usize::MAX)
}

/// Saved V5 source geometry admitted for canonical-page rendering only.
pub(crate) fn prepare_for_page(s: &Stroke) -> Option<Ink> {
    let rendering = s.rendering.as_ref()?;
    let properties = &rendering.properties;
    let style = &rendering.style;
    let argb = style.color_argb?;
    if (rendering.tool_type_raw == 1
        && !style.pen_size.is_some_and(|size| (4.0..=9.0).contains(&size)))
        || !matches!(rendering.pen_name.as_deref(), Some(PEN | STRAIGHT_PEN))
        || !matches!(rendering.advanced_settings.as_deref(), Some("" | "5;"))
        || !matches!(rendering.tool_type_raw, 1..=3)
        || properties.replay_only
        || properties.stylus_channels
        || properties.eraser
        || properties.fixed_width
        || properties.millisecond_timestamps
        || properties.alpha_lock
        || properties.fixed_opacity
        || properties.rainbow_effect
        || properties.straighten
        || properties.generated
        || properties.reveal_mode
        || !matches!(style.stroke_type_raw, None | Some(0))
        || !matches!(style.line_type_raw, None | Some(0))
        || !matches!(style.color_type_raw, None | Some(0))
        // The captured Drawing caller rejects fewer than two source events
        // before selecting the Marker3 V5 drawable.
        || s.points.len() < 2
    {
        return None;
    }
    let mut ink = prepare_bounded(s, MAX_PAGE_STAMPS)?;
    ink.opacity = f64::from(((argb >> 24) as u8) as f32 / 255.0);
    Some(ink)
}

fn prepare_bounded(s: &Stroke, max_stamps: usize) -> Option<Ink> {
    let r = s.rendering.as_ref()?;
    // Drawing forwards ObjectStroke::GetPenSize, not the decoder's
    // presentation width. Admit only the captured saved-size range here.
    let saved_size = r.style.pen_size?;
    // Saved ObjectStroke redraw loads this field, defaulting omitted metadata to zero.
    let reject_below = r.style.initial_tolerance.unwrap_or(0.0);
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
        || !reject_below.is_finite()
        || reject_below < 0.0
        || !saved_size.is_finite()
        || !(0.0..=800.0).contains(&saved_size)
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

    let size = saved_size.trunc();
    // Captured RTV1 zero-width controls emit no coverage; intermediate sizes
    // extrapolate from the integer-width conversion.
    let radius = if size == 0.0 {
        0.0
    } else {
        f64::from(size.mul_add(0.5, 0.5))
    };
    let mut points = Vec::new();
    let mut sample_ends = vec![0; s.points.len().max(2)];
    let mut previous = P::from(s.points[0]);
    let mut midpoint = previous;
    let mut residual = SPACING;
    // Initial drawPoint sets the alternate flag before saved samples are read.
    let mut alternate = true;
    if max_stamps == 0 {
        return None;
    }
    points.push(s.points[0]);
    sample_ends[0] = 1;

    for (i, point) in s.points.iter().enumerate().skip(1) {
        let next = P::from(*point);
        let last = i + 1 == s.points.len();
        let distance = next.sub(previous).len();
        let draw = if distance < reject_below {
            false
        } else if last {
            // The captured endPen route samples the terminal segment even
            // when the tool-one alternation would skip a movement event.
            true
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
                if points.len() >= max_stamps {
                    return None;
                }
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
    if points.len() >= max_stamps {
        return None;
    }
    points.push(*s.points.last()?);
    if let Some(end) = sample_ends.last_mut() {
        *end = points.len();
    }
    let radii = vec![radius; points.len()];
    Some(Ink {
        points,
        radii,
        sample_ends,
        opacity: 1.0,
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
                    pen_size: Some(input.width),
                    initial_tolerance: Some(2.0),
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

    fn page_stroke(input: &Input) -> Stroke {
        let mut stroke = stroke(input);
        let rendering = stroke.rendering.as_mut().unwrap();
        rendering.properties.compressed = true;
        rendering.properties.top_layer_pen = false;
        stroke
    }

    #[test]
    fn saved_tolerance_controls_rejection_and_invalid_values_fall_back() {
        let input = Input {
            samples: vec![[0.0, 0.0], [2.0, 0.0], [4.0, 0.0]],
            width: 8.0,
            tool_type_raw: 2,
            is_shape: false,
            fixed_width: false,
            wrapper_flag: false,
        };
        let mut source = stroke(&input);
        let at_threshold = prepare(&source).unwrap();
        source.rendering.as_mut().unwrap().style.initial_tolerance = Some(2.0001);
        let above_threshold = prepare(&source).unwrap();
        assert!(at_threshold.sample_ends[1] > above_threshold.sample_ends[1]);
        for tolerance in [-1.0, f32::NAN, f32::INFINITY] {
            source.rendering.as_mut().unwrap().style.initial_tolerance = Some(tolerance);
            assert!(prepare_for_page(&source).is_none());
        }
    }

    #[test]
    fn model_tolerance_boundary_vectors_match_native_bits() {
        for (tool, fixture) in [
            (
                1,
                include_str!("../../../../conformance/marker3-tolerance-boundary-tool1.json"),
            ),
            (
                2,
                include_str!("../../../../conformance/marker3-tolerance-boundary-tool2.json"),
            ),
        ] {
            let cases: serde_json::Value = serde_json::from_str(fixture).unwrap();
            for name in ["zero", "one", "below_two", "two", "above_two"] {
                let case = &cases[name];
                let input = Input {
                    samples: vec![[0.0, 0.0], [2.0, 0.0], [4.0, 0.0]],
                    width: 9.0,
                    tool_type_raw: tool,
                    is_shape: false,
                    fixed_width: false,
                    wrapper_flag: false,
                };
                let mut source = stroke(&input);
                source.rendering.as_mut().unwrap().style.initial_tolerance =
                    Some(case["tolerance"].as_f64().unwrap() as f32);
                let ink = prepare_for_page(&source).unwrap();
                let actual: String = ink
                    .points
                    .iter()
                    .flat_map(|p| [p.x as f32, p.y as f32])
                    .flat_map(f32::to_le_bytes)
                    .map(|b| format!("{b:02x}"))
                    .collect();
                for phase in ["draw_object_stroke", "saved_redraw"] {
                    assert_eq!(
                        actual,
                        case["phase_final_vectors"][phase]["vector_bytes"]
                            .as_str()
                            .unwrap(),
                        "{tool} {name} {phase}"
                    );
                }
            }
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

    #[test]
    fn canonical_page_gate_preserves_native_prefixes_and_source_alpha() {
        let fixture = fixture();
        for id in ["marker3-v5-saved-1", "marker3-v5-saved-3"] {
            let case = fixture.cases.iter().find(|case| case.id == id).unwrap();
            let mut stroke = page_stroke(&case.input);
            let rendering = stroke.rendering.as_mut().unwrap();
            rendering.style.color_argb = Some(0x80_336699);
            let ink = prepare_for_page(&stroke).unwrap();
            assert_eq!(ink.sample_ends, case.native.sample_ends);
            assert_eq!(ink.sample_ends.len(), stroke.points.len());
            assert_eq!(ink.sample_ends.last(), Some(&ink.points.len()));
            assert_eq!(ink.points.len(), case.native.stamps.len());
            assert_eq!(ink.opacity, f64::from(128_f32 / 255.0));
            for ((point, radius), [x, y, native_radius]) in
                ink.points.iter().zip(&ink.radii).zip(&case.native.stamps)
            {
                assert_eq!((point.x as f32).to_bits(), x.to_bits());
                assert_eq!((point.y as f32).to_bits(), y.to_bits());
                assert_eq!((*radius as f32).to_bits(), native_radius.to_bits());
            }

            let prepared = crate::ink::prepare_stroke_for_context(
                &stroke,
                false,
                crate::ink::InkRenderContext::CanonicalPage,
            );
            assert_eq!(
                prepared.sample_ends.as_ref(),
                Some(&case.native.sample_ends)
            );
            assert_eq!(
                prepared.dot_radii.as_ref().unwrap().len(),
                case.native.stamps.len()
            );
            assert_eq!(prepared.opacity, f64::from(128_f32 / 255.0));
            assert_eq!(prepared.support, crate::InkSupport::Reconstructed);

            let context_free = crate::ink::prepare_stroke(&stroke, false);
            assert!(context_free.sample_ends.is_none());
            assert!(context_free.dot_radii.is_none());
            assert_eq!(context_free.support, crate::InkSupport::Approximate);
        }
    }

    #[test]
    fn straightmarker_page_route_preserves_captured_v5_geometry_and_gates() {
        // The literal-name native Drawing matrix selects the same Marker3
        // factory, V5 drawable and complete XY stream for empty and `5;`.
        // Null instead selects V1 and must stay outside this V5 route.
        let fixture = caller_fixture();
        let case = &fixture["cases"]["marker3_linked_saved1:5:tool2:curve1"];
        let canonical = caller_stroke(case);
        let expected = prepare_for_page(&canonical).unwrap();
        assert_caller_xy_matches(&expected, case);
        for setting in ["", "5;"] {
            let mut alias = canonical.clone();
            let rendering = alias.rendering.as_mut().unwrap();
            rendering.pen_name = Some(STRAIGHT_PEN.to_owned());
            rendering.advanced_settings = Some(setting.to_owned());
            let actual = prepare_for_page(&alias).unwrap();
            assert_eq!(actual.points, expected.points);
            assert_eq!(actual.radii, expected.radii);
            assert_eq!(actual.sample_ends, expected.sample_ends);
            assert_eq!(actual.opacity, expected.opacity);
            let prepared = crate::ink::prepare_stroke_for_context(
                &alias,
                false,
                crate::ink::InkRenderContext::CanonicalPage,
            );
            assert_eq!(prepared.support, crate::InkSupport::Reconstructed);
            assert_eq!(prepared.sample_ends.as_ref(), Some(&expected.sample_ends));

            alias.rendering.as_mut().unwrap().advanced_settings = None;
            assert!(prepare_for_page(&alias).is_none());
            alias.rendering.as_mut().unwrap().advanced_settings = Some("5;".to_owned());
            alias.points.truncate(1);
            assert!(prepare_for_page(&alias).is_none());
        }
    }

    #[test]
    fn tool_one_geometry_matches_native_drawing() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/marker3-tool1-native.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_object().unwrap().values() {
            let stroke = caller_stroke(case);
            let ink = prepare_for_page(&stroke).unwrap();
            assert_caller_xy_matches(&ink, case);
            for pen in [PEN, STRAIGHT_PEN] {
                for setting in ["", "5;"] {
                    let mut alias = stroke.clone();
                    let rendering = alias.rendering.as_mut().unwrap();
                    rendering.pen_name = Some(pen.into());
                    rendering.advanced_settings = Some(setting.into());
                    let actual = prepare_for_page(&alias).unwrap();
                    assert_eq!(actual.points, ink.points);
                    assert_eq!(actual.radii, ink.radii);
                    assert_eq!(actual.sample_ends, ink.sample_ends);
                }
            }
            for change in 0..4 {
                let mut rejected = stroke.clone();
                let rendering = rejected.rendering.as_mut().unwrap();
                match change {
                    0 => rendering.style.pen_size = Some(3.99),
                    1 => rendering.style.pen_size = Some(9.01),
                    2 => rendering.style.pen_size = Some(f32::NAN),
                    _ => rendering.style.color_argb = None,
                }
                assert!(prepare_for_page(&rejected).is_none());
            }
        }
    }

    #[test]
    fn tool_three_page_geometry_matches_native_drawing() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/marker3-tool3-native.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let stroke = caller_stroke(case);
            assert_eq!(stroke.rendering.as_ref().unwrap().tool_type_raw, 3);
            let ink = prepare_for_page(&stroke).unwrap();
            assert_caller_xy_matches(&ink, case);
            let mut alias = stroke.clone();
            alias.rendering.as_mut().unwrap().pen_name = Some(STRAIGHT_PEN.to_owned());
            let alias_ink = prepare_for_page(&alias).unwrap();
            assert_eq!(ink.points, alias_ink.points);
            assert_eq!(ink.radii, alias_ink.radii);
            assert_eq!(ink.opacity, alias_ink.opacity);
        }
    }

    #[test]
    fn canonical_page_gate_rejects_unproven_marker3_inputs() {
        let fixture = fixture();
        let case = fixture
            .cases
            .iter()
            .find(|case| case.id == "marker3-v5-saved-1")
            .unwrap();
        let valid = page_stroke(&case.input);
        assert!(prepare_for_page(&valid).is_some());

        let mut rejected = valid.clone();
        rejected.rendering.as_mut().unwrap().properties.eraser = true;
        assert!(prepare_for_page(&rejected).is_none());

        let mut rejected = valid.clone();
        rejected.rendering.as_mut().unwrap().advanced_settings = None;
        assert!(prepare_for_page(&rejected).is_none());

        let mut rejected = valid.clone();
        rejected.rendering.as_mut().unwrap().tool_type_raw = 4;
        assert!(prepare_for_page(&rejected).is_none());

        let mut rejected = valid.clone();
        rejected.rendering.as_mut().unwrap().pen_name =
            Some("com.samsung.android.sdk.pen.pen.preload.Marker".to_owned());
        assert!(prepare_for_page(&rejected).is_none());

        let mut rejected = valid.clone();
        rejected.rendering.as_mut().unwrap().properties.fixed_width = true;
        assert!(prepare_for_page(&rejected).is_none());

        let mut rejected = valid.clone();
        rejected.rendering.as_mut().unwrap().style.color_argb = None;
        assert!(prepare_for_page(&rejected).is_none());

        let mut rejected = valid;
        rejected.points.pop();
        assert!(prepare_for_page(&rejected).is_none());

        let mut tap = page_stroke(&case.input);
        tap.points.truncate(1);
        let prepared = crate::ink::prepare_stroke_for_context(
            &tap,
            false,
            crate::ink::InkRenderContext::CanonicalPage,
        );
        assert!(prepared.sample_ends.is_none());
        assert!(prepared.dot_radii.is_none());
    }

    #[test]
    fn canonical_page_curve_disabled_v5_matches_saved_drawing_caller() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/marker3-saved-caller-native.json"
        ))
        .unwrap();
        for (id, core_case) in [
            ("marker3_linked_saved1:5:tool2:curve0", "marker3-v5-saved-1"),
            (
                "marker3_nontrivial_saved3:5:tool2:curve0",
                "marker3-v5-saved-3",
            ),
        ] {
            let case = &fixture["cases"][id];
            assert_eq!(case["input"]["curve_enabled"], false);
            assert_eq!(
                case["native_calls"]["source_ObjectStroke_IsCurveEnabled"],
                0
            );
            assert_eq!(
                case["native_calls"]["route"]["drawing_selected_drawable_version"],
                "5"
            );
            assert_eq!(
                case["native_calls"]["ObjectStroke_GetAdvancedPenSetting"]["utf8"],
                "5;"
            );
            assert!(
                case["native_calls"]["curve_forward_virtual_calls"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
            for snapshot in case["native_calls"]["ObjectStroke_getter_snapshots"]
                .as_array()
                .unwrap()
            {
                assert_eq!(snapshot["curve_enabled"], 0);
            }

            let stroke = caller_stroke(case);
            assert!(!stroke.rendering.as_ref().unwrap().properties.compressed);
            let ink = prepare_for_page(&stroke).unwrap();
            assert_caller_xy_matches(&ink, case);
            assert_native_sample_ends(&ink, core_case);

            let prepared = crate::ink::prepare_stroke_for_context(
                &stroke,
                false,
                crate::ink::InkRenderContext::CanonicalPage,
            );
            assert_eq!(
                prepared.sample_ends.as_ref(),
                Some(&ink.sample_ends),
                "{id} V5 native sample boundaries"
            );
            assert_eq!(prepared.support, crate::InkSupport::Reconstructed);
            let context_free = crate::ink::prepare_stroke(&stroke, false);
            assert!(context_free.sample_ends.is_none());
            assert!(context_free.dot_radii.is_none());
            assert_eq!(context_free.support, crate::InkSupport::Approximate);
        }
    }

    #[test]
    fn canonical_page_sampler_stops_at_output_stamp_budget() {
        let fixture = fixture();
        let case = fixture
            .cases
            .iter()
            .find(|case| case.id == "marker3-v5-saved-1")
            .unwrap();
        let mut stroke = page_stroke(&case.input);
        stroke.points = vec![
            Point { x: 0.0, y: 0.0 },
            Point {
                x: 70_000.0,
                y: 0.0,
            },
        ];
        assert!(prepare_for_page(&stroke).is_none());
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

    #[test]
    fn alias_paint_sources_match_native_vector_hashes() {
        use serde_json::Value;
        use sha2::{Digest, Sha256};

        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/marker3-v5-scale-shell-native.json"
        ))
        .unwrap();
        let original = &fixture["alias_paint_original"];
        let scale: Value =
            serde_json::from_str(original["scale_receipt_json"].as_str().unwrap()).unwrap();
        let paths: Value =
            serde_json::from_str(original["paths_receipt_json"].as_str().unwrap()).unwrap();
        for (receipt, scale_controls) in [(&scale, true), (&paths, false)] {
            let cases = receipt["cases"].as_object().unwrap();
            assert_eq!(cases.len(), 3);
            for (label, case) in cases {
                let input = if scale_controls {
                    &case["admission"]["Marker3"]["input"]
                } else {
                    &case["input"]
                };
                let points = input["points_f32_bits"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|point| {
                        [
                            f64::from(f32_from_bits_hex(point[0].as_str().unwrap())),
                            f64::from(f32_from_bits_hex(point[1].as_str().unwrap())),
                        ]
                    })
                    .collect();
                let width = f32_from_bits_hex(input["pen_size_f32_bits"].as_str().unwrap());
                let base = Input {
                    samples: points,
                    width,
                    tool_type_raw: input["tool_type"].as_u64().unwrap() as u16,
                    is_shape: false,
                    fixed_width: false,
                    wrapper_flag: false,
                };
                for (name, pen) in [("Marker3", PEN), ("StraightMarker", STRAIGHT_PEN)] {
                    let factory = if scale_controls {
                        &case["admission"][name]["factory"]
                    } else {
                        &case["factory"][name]
                    };
                    assert_eq!(factory["requested_canonical_pen_name"], pen);
                    assert_eq!(factory["selected_v5_or_v1_actual"], "5");
                    let mut stroke = page_stroke(&base);
                    let rendering = stroke.rendering.as_mut().unwrap();
                    rendering.pen_name = Some(pen.to_owned());
                    rendering.advanced_settings =
                        input["advanced_setting"].as_str().map(str::to_owned);
                    rendering.style.initial_tolerance = None;
                    let ink = prepare_for_page(&stroke).expect("native alias source is admitted");
                    let vector: Vec<u8> = ink
                        .points
                        .iter()
                        .flat_map(|point| [point.x as f32, point.y as f32])
                        .flat_map(f32::to_le_bytes)
                        .collect();
                    assert_eq!(
                        format!("{:x}", Sha256::digest(&vector)),
                        case["vector_sha256"],
                        "{label}/{name}"
                    );
                    if let Some(count) = case["callback_count"].as_u64() {
                        assert_eq!(ink.points.len(), count as usize, "{label}/{name}");
                    }
                }
            }
        }
    }

    #[test]
    fn zigzag_source_and_host_frames_match_native_controls() {
        use base64::Engine;
        use serde_json::Value;
        use sha2::{Digest, Sha256};
        use std::io::Read;

        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/marker3-v5-scale-shell-native.json"
        ))
        .unwrap();
        let capture = &fixture["zigzag_capture_original"];
        let caller = &capture["caller_case"];
        let stroke = caller_stroke(caller);
        let ink = prepare_for_page(&stroke).expect("native zigzag is admitted");
        assert_caller_xy_matches(&ink, caller);
        let source = &caller["phase_final_vectors"]["saved_redraw"];
        let source_bytes = bytes_from_hex(source["vector_bytes"].as_str().unwrap());
        assert_eq!(
            format!("{:x}", Sha256::digest(&source_bytes)),
            source["sha256"]
        );

        let zero_data = fixture["zero_alpha_control"]["frame_zlib_base64"]
            .as_str()
            .unwrap();
        let mut total = 0;
        for (key, expected_cases) in [
            ("simple_fixture_template", 1_usize),
            ("expanded_fixture_template", 8),
        ] {
            let text = capture[key]
                .as_str()
                .unwrap()
                .replace("__SDOCX_ZERO_ALPHA__", zero_data);
            let captured: Value = serde_json::from_str(&text).unwrap();
            assert_eq!(
                captured["zero_alpha_control"],
                fixture["zero_alpha_control"]
            );
            let cases = captured["cases"].as_object().unwrap();
            assert_eq!(cases.len(), expected_cases);
            for control in cases
                .values()
                .chain(std::iter::once(&captured["zero_alpha_control"]))
            {
                assert_eq!(control["gl_error"], 0);
                let points = control["input"]["points"].as_array().unwrap();
                assert_eq!(points.len() * 8, control["native_vector_bytes"]);
                let compressed = base64::engine::general_purpose::STANDARD
                    .decode(control["frame_zlib_base64"].as_str().unwrap())
                    .unwrap();
                let mut frame = Vec::new();
                flate2::read::ZlibDecoder::new(compressed.as_slice())
                    .take((256 * 256 * 4 + 1) as u64)
                    .read_to_end(&mut frame)
                    .unwrap();
                assert_eq!(frame.len(), 256 * 256 * 4);
                assert_eq!(
                    format!("{:x}", Sha256::digest(&frame)),
                    control["frame_sha256"]
                );
                if control == &captured["zero_alpha_control"] {
                    assert!(frame.iter().all(|byte| *byte == 0));
                } else {
                    assert_eq!(
                        control["input"]["source_point_bytes_sha256"],
                        source["sha256"]
                    );
                    assert_eq!(points.len(), ink.points.len());
                    let mut upload = Vec::with_capacity(points.len() * 8);
                    for (point, actual) in points.iter().zip(&ink.points) {
                        let x = actual.x + 32.0;
                        let y = actual.y + 96.0;
                        assert!((point[0].as_f64().unwrap() - x).abs() < 1e-10);
                        assert!((point[1].as_f64().unwrap() - y).abs() < 1e-10);
                        upload.extend_from_slice(&(x as f32).to_le_bytes());
                        upload.extend_from_slice(&(y as f32).to_le_bytes());
                    }
                    assert_eq!(
                        format!("{:x}", Sha256::digest(&upload)),
                        control["input"]["native_addpoint_xy_upload_sha256"]
                    );
                }
            }
            total += cases.len();
        }
        assert_eq!(total, 9);
    }

    #[test]
    fn size_nine_diverse_retained_provenance_and_frames() {
        use serde_json::{Value, json};
        use sha2::{Digest, Sha256};
        use std::{collections::BTreeMap, io::Read};
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let read = |path: &str| std::fs::read(root.join(path)).unwrap();
        let hash = |bytes: &[u8]| format!("{:x}", Sha256::digest(bytes));
        fn canonical(value: &Value) -> String {
            match value {
                Value::Object(map) => format!(
                    "{{{}}}",
                    map.iter()
                        .collect::<BTreeMap<_, _>>()
                        .into_iter()
                        .map(|(k, v)| format!(
                            "{}:{}",
                            serde_json::to_string(k).unwrap(),
                            canonical(v)
                        ))
                        .collect::<Vec<_>>()
                        .join(",")
                ),
                Value::Array(values) => format!(
                    "[{}]",
                    values.iter().map(canonical).collect::<Vec<_>>().join(",")
                ),
                _ => serde_json::to_string(value).unwrap(),
            }
        }
        let canon_hash = |value: &Value| hash(canonical(value).as_bytes());
        let receipt: Value =
            serde_json::from_slice(&read("tools/marker3-size9-diverse/receipt.json")).unwrap();
        for path in [
            "conformance/marker3-size9-diverse-native.json",
            "tools/marker3-size9-diverse/capture.py",
            "tools/marker3-size9-diverse/caller-provenance.json",
        ] {
            assert_eq!(
                hash(&read(path)),
                receipt["input_sha256"][path].as_str().unwrap(),
                "{path}"
            );
        }
        let fixture: Value =
            serde_json::from_slice(&read("conformance/marker3-size9-diverse-native.json")).unwrap();
        let provenance: Value =
            serde_json::from_slice(&read("tools/marker3-size9-diverse/caller-provenance.json"))
                .unwrap();
        assert_eq!(
            provenance["schema"],
            "marker3-size9-caller-provenance-extraction-v1"
        );
        assert_eq!(
            provenance["canonical_fixture"]["path"],
            "conformance/marker3-size9-diverse-native.json"
        );
        assert_eq!(
            hash(&read("conformance/marker3-size9-diverse-native.json")),
            provenance["canonical_fixture"]["sha256"].as_str().unwrap()
        );
        type Control<'a> = (&'a str, &'a [[f32; 2]], &'a [f32], &'a str);
        let controls: [Control<'_>; 3] = [
            (
                "reverse",
                &[[14.25, 20.5], [44.5, 20.5], [17.5, 22.5], [48.25, 24.5]],
                &[1.0, 0.15, 1.0, 0.15],
                "0x80cc331a",
            ),
            (
                "cross",
                &[[18.25, 12.5], [45.5, 41.25], [18.25, 41.25], [45.5, 12.5]],
                &[0.2, 0.9, 0.4, 1.0],
                "0xff3366cc",
            ),
            (
                "short",
                &[[12.5, 18.25], [15.25, 20.5], [18.5, 17.75]],
                &[0.1, 0.8, 0.4],
                "0x6633aa55",
            ),
        ];
        for (name, points, pressures, color) in controls {
            let retained = &provenance["cases"][name];
            assert_eq!(
                retained["historical_gzip_sha256"],
                receipt["caller_gzip_sha256"][name]
            );
            let mut case = provenance["common_case_fields"].clone();
            case.as_object_mut()
                .unwrap()
                .extend(retained["case_fields"].as_object().unwrap().clone());
            case["input"] = fixture[name]["input"].clone();
            case["phase_final_vectors"] = fixture[name]["phase_final_vectors"].clone();
            case["native_calls"]["source_ObjectStroke_arrays"] =
                fixture[name]["native_calls"]["source_ObjectStroke_arrays"].clone();
            let mut vectors = BTreeMap::new();
            for (phase, vector) in case["phase_final_vectors"].as_object().unwrap() {
                let raw = bytes_from_hex(vector["vector_bytes"].as_str().unwrap());
                assert_eq!(
                    raw.len(),
                    8 * usize::try_from(vector["vector_count"].as_u64().unwrap()).unwrap()
                );
                assert_eq!(hash(&raw), vector["sha256"].as_str().unwrap());
                let xy: Vec<Value> = raw
                    .as_chunks::<8>()
                    .0
                    .iter()
                    .map(|pair| {
                        json!([
                            f32::from_le_bytes(pair[..4].try_into().unwrap()) as f64,
                            f32::from_le_bytes(pair[4..].try_into().unwrap()) as f64
                        ])
                    })
                    .collect();
                vectors.insert(phase.clone(), xy);
            }
            let mut state = serde_json::Map::new();
            let mut callbacks = Vec::new();
            for changes in retained["callback_changes"].as_array().unwrap() {
                state.extend(changes.as_object().unwrap().clone());
                let mut callback = Value::Object(state.clone());
                let count = usize::try_from(callback["vector_count"].as_u64().unwrap()).unwrap();
                let vector = &vectors[callback["phase"].as_str().unwrap()];
                assert!(count <= vector.len());
                callback["xy"] = Value::Array(vector[..count].to_vec());
                callbacks.push(callback);
            }
            case["callback_add_point_calls"] = Value::Array(callbacks);
            let mut state = json!({"regs": {}});
            let mut lifecycle = Vec::new();
            for changes in retained["lifecycle_changes"].as_array().unwrap() {
                for (key, value) in changes.as_object().unwrap() {
                    if key != "regs_changes" && key != "removed_fields" {
                        state[key] = value.clone();
                    }
                }
                for key in changes["removed_fields"].as_array().unwrap() {
                    assert!(
                        state
                            .as_object_mut()
                            .unwrap()
                            .remove(key.as_str().unwrap())
                            .is_some()
                    );
                }
                state["regs"]
                    .as_object_mut()
                    .unwrap()
                    .extend(changes["regs_changes"].as_object().unwrap().clone());
                lifecycle.push(state.clone());
            }
            case["native_calls"]["selected_drawable_redraw_lifecycle_method_entries"] =
                Value::Array(lifecycle);
            let mut cases = serde_json::Map::new();
            cases.insert(
                provenance["case_key"].as_str().unwrap().to_owned(),
                case.clone(),
            );
            let document = json!({"schema": provenance["root_schema"], "cases": cases});
            assert_eq!(
                canon_hash(&document),
                retained["original_structured_sha256"].as_str().unwrap(),
                "{name}"
            );
            assert_eq!(
                fixture[name],
                json!({"input": case["input"], "native_calls": {"source_ObjectStroke_arrays": case["native_calls"]["source_ObjectStroke_arrays"]}, "phase_final_vectors": case["phase_final_vectors"]})
            );
            let expected_points: Vec<u8> = points
                .iter()
                .flatten()
                .flat_map(|v| v.to_le_bytes())
                .collect();
            assert_eq!(
                bytes_from_hex(
                    case["native_calls"]["source_ObjectStroke_arrays"]["points"]["data_hex"]
                        .as_str()
                        .unwrap()
                ),
                expected_points
            );
            let fields = json!({
                "input_sha256": canon_hash(&case["input"]),
                "source_arrays_sha256": canon_hash(&case["native_calls"]["source_ObjectStroke_arrays"]),
                "drawing_vector_sha256": canon_hash(&case["phase_final_vectors"]["draw_object_stroke"]),
                "saved_redraw_vector_sha256": canon_hash(&case["phase_final_vectors"]["saved_redraw"]),
            });
            for suffix in ["", "1", "4"] {
                assert_eq!(
                    fields,
                    receipt["cross_scale_caller_fields_sha256"][format!("{name}{suffix}")]
                );
            }
            let input = &case["input"];
            assert_eq!(
                bytes_from_hex(input["pen_size_f32_bits"].as_str().unwrap()),
                9_f32.to_le_bytes()
            );
            assert_eq!(
                input["pressure_f32_bits"].as_array().unwrap().len(),
                pressures.len()
            );
            for (actual, expected) in input["pressure_f32_bits"]
                .as_array()
                .unwrap()
                .iter()
                .zip(pressures)
            {
                assert_eq!(
                    bytes_from_hex(actual.as_str().unwrap()),
                    expected.to_le_bytes()
                );
            }
            assert_eq!(input["color_argb"], color);
            assert_eq!(input["advanced_setting"], "5;");
            assert_eq!(input["tool_type"], 1);
            assert_eq!(input["curve_enabled"], true);
            assert_eq!(input["stroke_type"], 0);
            assert_eq!(
                input["timestamps_i32"],
                json!((0..points.len()).map(|i| 8 * i).collect::<Vec<_>>())
            );
            for key in ["tilt_f32_bits", "orientation_f32_bits"] {
                assert_eq!(input[key], json!(vec!["00000000"; points.len()]));
            }
        }
        let frame_hashes = receipt["native_rgba_sha256"].as_object().unwrap();
        assert_eq!(frame_hashes.len(), 9);
        for (name, expected) in frame_hashes {
            assert!(matches!(
                name.as_str(),
                "reverse"
                    | "cross"
                    | "short"
                    | "reverse1"
                    | "cross1"
                    | "short1"
                    | "reverse4"
                    | "cross4"
                    | "short4"
            ));
            let compressed = read(&format!("tools/marker3-size9-diverse/{name}.rgba.zlib"));
            let mut decoder = flate2::read::ZlibDecoder::new(compressed.as_slice());
            let mut frame = Vec::new();
            decoder
                .by_ref()
                .take(256 * 256 * 4 + 1)
                .read_to_end(&mut frame)
                .unwrap();
            assert_eq!(frame.len(), 256 * 256 * 4);
            assert_eq!(decoder.total_in(), compressed.len() as u64);
            assert_eq!(hash(&frame), expected.as_str().unwrap(), "{name}");
        }
    }

    #[test]
    fn size_nine_diverse_native_vectors_and_gate() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/marker3-size9-diverse-native.json"
        ))
        .unwrap();
        for name in ["reverse", "cross", "short"] {
            let stroke = caller_stroke(&cases[name]);
            let ink = prepare_for_page(&stroke).unwrap();
            assert_caller_xy_matches(&ink, &cases[name]);
            let mut rejected = stroke.clone();
            rejected.rendering.as_mut().unwrap().style.pen_size = Some(9.001);
            assert!(prepare_for_page(&rejected).is_none());
            rejected = stroke.clone();
            rejected.rendering.as_mut().unwrap().properties.eraser = true;
            assert!(prepare_for_page(&rejected).is_none());
            rejected = stroke.clone();
            rejected.rendering.as_mut().unwrap().advanced_settings = None;
            assert!(prepare_for_page(&rejected).is_none());
        }
    }

    #[test]
    fn size_nine_note_derived_native_vector() {
        use sha2::{Digest, Sha256};
        use std::io::Read;

        let case: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/marker3-size9-note-derived-native.json"
        ))
        .unwrap();
        let stroke = caller_stroke(&case);
        assert_caller_xy_matches(&prepare_for_page(&stroke).unwrap(), &case);
        let receipt: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tools/marker3-size9-note-derived/native-receipt.json"
        ))
        .unwrap();
        for (scale, compressed) in [
            (
                "1",
                include_bytes!("../../../../tools/marker3-size9-note-derived/note1.rgba.zlib")
                    .as_slice(),
            ),
            (
                "2",
                include_bytes!("../../../../tools/marker3-size9-note-derived/note2.rgba.zlib")
                    .as_slice(),
            ),
            (
                "4",
                include_bytes!("../../../../tools/marker3-size9-note-derived/note4.rgba.zlib")
                    .as_slice(),
            ),
        ] {
            let mut frame = Vec::new();
            flate2::read::ZlibDecoder::new(compressed)
                .read_to_end(&mut frame)
                .unwrap();
            assert_eq!(
                format!("{:x}", Sha256::digest(&frame)),
                receipt[scale]["rgba_sha256"].as_str().unwrap(),
                "scale {scale}"
            );
        }
    }

    #[test]
    fn terminal_segment_matches_expanded_native_vectors() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/marker3-terminal-native.json"
        ))
        .unwrap();
        for name in ["phase2", "phase4", "boundary"] {
            let case = &cases[name];
            let stroke = caller_stroke(case);
            if name == "boundary" {
                assert!(prepare_for_page(&stroke).is_none());
                assert_caller_xy_matches(&prepare(&stroke).unwrap(), case);
            } else {
                assert_caller_xy_matches(&prepare_for_page(&stroke).unwrap(), case);
            }
        }
        for name in ["inside", "near", "two"] {
            let case = &cases[name];
            let stroke = caller_stroke(case);
            assert_caller_xy_matches(&prepare_for_page(&stroke).unwrap(), case);
        }
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
                    compressed: input["curve_enabled"].as_bool().unwrap(),
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
                    pen_size: Some(width),
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
        for phase in ["draw_object_stroke", "saved_redraw"] {
            let output = bytes_from_hex(
                case["phase_final_vectors"][phase]["vector_bytes"]
                    .as_str()
                    .unwrap(),
            );
            assert_eq!(output.len() % 8, 0, "{phase} byte alignment");
            assert_eq!(ink.points.len(), output.len() / 8, "{phase} count");
            for (point, pair) in ink.points.iter().zip(output.as_chunks::<8>().0) {
                let x = f32::from_bits(u32::from_le_bytes(pair[..4].try_into().unwrap()));
                let y = f32::from_bits(u32::from_le_bytes(pair[4..].try_into().unwrap()));
                assert_eq!((point.x as f32).to_bits(), x.to_bits(), "{phase} x");
                assert_eq!((point.y as f32).to_bits(), y.to_bits(), "{phase} y");
            }
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
    fn saved_size_controls_radius_independently_of_presentation_width() {
        let fixture = caller_fixture();
        let case = &fixture["cases"]["marker3_linked_saved1:5:tool2:curve1"];
        let native_size = f32_from_bits_hex(
            case["native_calls"]["source_ObjectStroke_GetPenSize_f32_bits"]
                .as_str()
                .unwrap(),
        );
        assert_eq!(native_size, 8.0);
        let mut stroke = caller_stroke(case);
        stroke.pen_width = 1.07;
        let ink = prepare(&stroke).unwrap();
        assert_caller_xy_matches(&ink, case);
        assert!(ink.radii.iter().all(|&radius| radius == 4.5));

        for size in [None, Some(f32::NAN), Some(-1.0), Some(801.0)] {
            stroke.rendering.as_mut().unwrap().style.pen_size = size;
            assert!(prepare(&stroke).is_none());
        }
    }

    #[test]
    fn zero_width_admission_keeps_existing_version_and_style_gates() {
        let fixture = caller_fixture();
        let case = &fixture["cases"]["marker3_linked_saved1:5:tool2:curve1"];
        let mut stroke = caller_stroke(case);
        stroke.rendering.as_mut().unwrap().style.pen_size = Some(0.0);
        for tool in [2, 3] {
            stroke.rendering.as_mut().unwrap().tool_type_raw = tool;
            for setting in ["", "5;"] {
                stroke.rendering.as_mut().unwrap().advanced_settings = Some(setting.into());
                let paint = prepare_for_page(&stroke).unwrap();
                assert!(!paint.radii.is_empty() && paint.radii.iter().all(|radius| *radius == 0.0));
            }
        }
        for change in 0..6 {
            let mut rejected = stroke.clone();
            let rendering = rejected.rendering.as_mut().unwrap();
            match change {
                0 => rendering.tool_type_raw = 1,
                1 => rendering.advanced_settings = None,
                2 => rendering.advanced_settings = Some("4;".into()),
                3 => rendering.style.pen_size = Some(-0.01),
                4 => rendering.style.pen_size = Some(f32::NAN),
                _ => rendering.properties.eraser = true,
            }
            assert!(prepare_for_page(&rejected).is_none(), "gate {change}");
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
