//! Saved Marker3 V5 centerline stamps, measured against the pinned native
//! ObjectStroke redraw. The mask and composite shaders remain separate.
use super::path::{P, Quad};
use crate::{Point, Stroke};

const PEN: &str = "com.samsung.android.sdk.pen.pen.preload.Marker3";
const STRAIGHT_PEN: &str = "com.samsung.android.sdk.pen.pen.preload.StraightMarker";
const SPACING: f32 = 1.0;
const REJECT_BELOW: f32 = 2.0;
const ALTERNATE_BELOW: f32 = 20.0;

pub(super) struct Ink {
    pub points: Vec<Point>,
    pub radii: Vec<f64>,
    /// Native redraw callback boundaries; a one-point stroke has `[1, 2]`
    /// because its initial drawPoint and endPen terminal point are separate.
    /// These are not PreparedStroke replay sample attribution boundaries.
    pub sample_ends: Vec<usize>,
}

pub(super) fn prepare(s: &Stroke) -> Option<Ink> {
    let r = s.rendering.as_ref()?;
    if !matches!(r.pen_name.as_deref(), Some(PEN | STRAIGHT_PEN))
        || r.advanced_settings.as_deref().is_some_and(|setting| setting != "5;")
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
        serde_json::from_str(include_str!("../../../../conformance/marker3-saved-native.json"))
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
}
