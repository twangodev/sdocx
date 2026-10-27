//! Saved DefaultPen geometry reconstructed from native RT AddPoint calls.
use super::path::{P, Quad};
use crate::Point;

pub(super) struct DefaultInk {
    pub points: Vec<[f32; 3]>,
    /// Modeled cumulative counts per redrawLine return, including empty first
    /// and last returns. The six-point native witness validates this schedule;
    /// broader native source attribution remains unproven.
    pub redraw_ends: Vec<usize>,
    pub terminal_start: usize,
}

const MAX_INPUT_POINTS: usize = 100_000;

fn repeat(length: f32, width: i32) -> usize {
    let scaled = if width <= 5 {
        length * 0.5
    } else if width <= 10 {
        length / 3.0
    } else if width <= 15 {
        length * 0.25
    } else if width <= 20 {
        length / 5.0
    } else if width <= 30 {
        length / 6.0
    } else if width <= 40 {
        length / 7.0
    } else if width <= 55 {
        length * 0.125
    } else {
        length / 10.0
    };
    (scaled.trunc().max(2.0) as usize) + 1
}

struct Cubic {
    points: [P; 4],
    segments: Vec<(f32, u16)>,
    length: f32,
}
impl Cubic {
    fn new(points: [P; 4]) -> Self {
        let mut path = Self {
            points,
            segments: Vec::new(),
            length: 0.0,
        };
        path.split(points, 0, 32767);
        path
    }
    fn split(&mut self, p: [P; 4], t0: u16, t1: u16) {
        let line = |a: P, b: P, t: f32| P {
            x: (b.x - a.x).mul_add(t, a.x),
            y: (b.y - a.y).mul_add(t, a.y),
        };
        let one = line(p[0], p[3], f32::from_bits(0x3eaa_aaab));
        let two = line(p[0], p[3], f32::from_bits(0x3f2a_aaab));
        let dev1 = (p[1].x - one.x).abs().max((p[1].y - one.y).abs());
        let dev2 = (p[2].x - two.x).abs().max((p[2].y - two.y).abs());
        if t1 - t0 >= 1024 && (dev1 > 0.5 || dev2 > 0.5) {
            let a = p[0].mid(p[1]);
            let b = p[1].mid(p[2]);
            let c = p[2].mid(p[3]);
            let d = a.mid(b);
            let e = b.mid(c);
            let f = d.mid(e);
            let t = (t0 + t1) / 2;
            self.split([p[0], a, d, f], t0, t);
            self.split([f, e, c, p[3]], t, t1);
        } else {
            let next = self.length + p[0].sub(p[3]).len();
            if next > self.length {
                self.length = next;
                self.segments.push((next, t1));
            }
        }
    }
    fn at(&self, distance: f32) -> P {
        let i = self
            .segments
            .partition_point(|&(end, _)| end < distance)
            .min(self.segments.len() - 1);
        let (start, t0) = if i == 0 {
            (0.0, 0)
        } else {
            self.segments[i - 1]
        };
        let (end, t1) = self.segments[i];
        let scale = f32::from_bits(0x3800_00fd);
        let t0 = t0 as f32 * scale;
        let t1 = t1 as f32 * scale;
        let t = t0 + (distance - start) * (t1 - t0) / (end - start);
        let eval = |a: f32, b: f32, c: f32, d: f32| {
            if t == 0.0 {
                return a;
            }
            let x = (b - a).mul_add(t, a);
            let y = (c - b).mul_add(t, b);
            let z = (d - c).mul_add(t, c);
            let q = (y - x).mul_add(t, x);
            let r = (z - y).mul_add(t, y);
            (r - q).mul_add(t, q)
        };
        P {
            x: eval(
                self.points[0].x,
                self.points[1].x,
                self.points[2].x,
                self.points[3].x,
            ),
            y: eval(
                self.points[0].y,
                self.points[1].y,
                self.points[2].y,
                self.points[3].y,
            ),
        }
    }
}

const MAX_SAMPLES_PER_PATH: usize = 1_000_000;

fn count(length: f32, size: f32) -> Option<usize> {
    if !length.is_finite() || length < 0.0 {
        return None;
    }
    let count = repeat(length, size.trunc() as i32);
    (count <= MAX_SAMPLES_PER_PATH).then_some(count)
}
fn emit_cubic(out: &mut Vec<[f32; 3]>, points: [P; 4], size: f32) -> Option<()> {
    let path = Cubic::new(points);
    if path.length == 0.0 {
        return Some(()); // Native getPosTan fails for an empty path.
    }
    let count = count(path.length, size)?;
    if out.len().checked_add(count)? > MAX_SAMPLES_PER_PATH {
        return None;
    }
    let step = path.length / count as f32;
    let radius = (size * 0.5).max(1.0);
    let mut distance = 0.0_f32;
    for _ in 0..count {
        let p = path.at(distance);
        out.push([p.x, p.y, radius + 0.5]);
        distance += step;
    }
    Some(())
}

fn emit_quad(out: &mut Vec<[f32; 3]>, start: P, control: P, end: P, size: f32) -> Option<()> {
    let path = Quad::new(start, control, end);
    if path.length == 0.0 {
        return Some(()); // Native getPosTan fails for an empty path.
    }
    let count = count(path.length, size)?;
    if out.len().checked_add(count)? > MAX_SAMPLES_PER_PATH {
        return None;
    }
    let step = path.length / count as f32;
    let radius = (size * 0.5).max(1.0);
    let mut distance = 0.0_f32;
    for _ in 0..count {
        let p = path.at(distance);
        // DefaultPen RT stores the AddPoint radius with its native +0.5 bias.
        out.push([p.x, p.y, radius + 0.5]);
        distance += step;
    }
    Some(())
}

/// Reconstructs the observed saved-redraw profile for supported source context.
/// `source` comes from the native ObjectStroke RedrawPen constructor path:
/// DefaultPen's ObjectStroke overload calls MotionEvent(int, int, ...), which
/// delegates to MotionEvent(), zeroing its source field before GetSource reads
/// it. The pinned Drawing and saved-redraw captures both returned zero.
pub(super) fn saved_with_boundaries(
    points: &[Point],
    size: f32,
    tool: u32,
    source: u32,
) -> Option<DefaultInk> {
    if points.is_empty()
        || points.len() > MAX_INPUT_POINTS
        || !size.is_finite()
        || size < 0.0
        || size >= i32::MAX as f32
    {
        return None;
    }
    if !matches!(tool, 0..=3) {
        return None;
    }
    // Coordinates are bounded so float32 squared-distance calculations cannot
    // overflow: 8 * bound^2 is at most f32::MAX for any point pair.
    let coordinate_bound = (f64::from(f32::MAX) / 8.0).sqrt();
    if points.iter().any(|p| {
        !p.x.is_finite()
            || !p.y.is_finite()
            || p.x.abs() > coordinate_bound
            || p.y.abs() > coordinate_bound
            || !(p.x as f32).is_finite()
            || !(p.y as f32).is_finite()
    }) {
        return None;
    }
    if points.len() == 1 {
        return None; // The shipped ObjectStroke wrapper returns error 7 without AddPoint.
    }
    let input: Vec<P> = points.iter().copied().map(P::from).collect();
    let radius = (size * 0.5).max(1.0) + 0.5;
    let redraw_count = points.len().saturating_sub(2);
    if points.len() == 2 {
        return Some(DefaultInk {
            points: vec![[input[0].x, input[0].y, radius]],
            redraw_ends: Vec::new(),
            terminal_start: 0,
        });
    }
    // The start gate compares each historical event with the original down
    // position, so small movements can accumulate before the first accepted
    // event seeds the control point.
    let first_move = input[1..input.len() - 1]
        .iter()
        .position(|point| {
            let delta = point.sub(input[0]);
            delta.x.abs() >= 1.5 || delta.y.abs() >= 1.5
        })
        .map(|index| index + 1);
    let Some(first_move) = first_move else {
        return Some(DefaultInk {
            points: vec![[input[0].x, input[0].y, radius]],
            redraw_ends: vec![0; redraw_count],
            terminal_start: 0,
        });
    };
    // Delayed start changes native terminal state; retain generic rendering
    // until that transition is reconstructed.
    if first_move > 1 {
        return None;
    }
    let threshold = if tool == 1 || (tool == 2 && source == 0x1002) {
        50.0_f32
    } else {
        5.0_f32
    };
    let mut output = Vec::new();
    let mut redraw_ends = Vec::with_capacity(redraw_count);
    let mut alternate = true;
    let mut start = input[0];
    let mut control = input[first_move];
    let mut retained = input[first_move];
    let mut endpoint = input[first_move];
    let mut cubic_end = false;
    // RedrawPen calls redrawLine once for the initial historical event. The
    // captured six-point caller emits no AddPoint at that first return.
    redraw_ends.resize(first_move, 0);

    // RedrawPen visits all but the terminal event as history. The first
    // historical event seeds the drawable; later accepted events emit a
    // midpoint quadratic and advance its retained start/control state.
    for &next in &input[first_move + 1..input.len() - 1] {
        let previous = endpoint;
        let distance = next.sub(previous).len();
        let accepted = if distance > threshold {
            alternate = true;
            true
        } else {
            let prior = alternate;
            alternate = !alternate;
            !prior
        };
        if accepted {
            let midpoint = retained.mid(next);
            emit_quad(&mut output, start, control, midpoint, size)?;
            start = midpoint;
            control = next;
            retained = next;
        }
        cubic_end = alternate;
        endpoint = next;
        redraw_ends.push(output.len());
    }
    let terminal_start = output.len();
    let terminal = *input.last()?;
    if cubic_end {
        let midpoint = control.mid(terminal);
        emit_cubic(&mut output, [start, control, midpoint, terminal], size)?;
    } else {
        emit_quad(&mut output, start, control, terminal, size)?;
    }
    Some(DefaultInk {
        points: output,
        redraw_ends,
        terminal_start,
    })
}

#[cfg(test)]
fn saved(points: &[Point], size: f32, tool: u32, source: u32) -> Option<Vec<[f32; 3]>> {
    saved_with_boundaries(points, size, tool, source).map(|ink| ink.points)
}

/// Canonical-page admission for the native DefaultPen profile. The pinned
/// DefaultPen::SetAdvancedSetting is a no-op and GetStrokeDrawableGL always
/// constructs/reuses the same unsuffixed drawable. Saved geometry is observed
/// for tool types 0..=3; tools 1..=3 also have curve-on/off witnesses. Other
/// style/effect states fall back until their Drawing behavior is established.
pub(super) fn prepare(stroke: &crate::Stroke) -> Option<DefaultInk> {
    let rendering = stroke.rendering.as_ref()?;
    if rendering.pen_name.as_deref()? != "com.samsung.android.sdk.pen.pen.preload.DefaultPen"
        || !matches!(rendering.tool_type_raw, 0..=3)
        || rendering.properties.fixed_width
        || rendering.properties.replay_only
        || rendering.properties.eraser
        || rendering.properties.alpha_lock
        || rendering.properties.fixed_opacity
        || rendering.properties.rainbow_effect
        || rendering.properties.straighten
        || rendering.properties.reveal_mode
        || rendering.properties.top_layer_pen
        || rendering
            .style
            .stroke_type_raw
            .is_some_and(|value| value != 0)
        || rendering
            .style
            .color_type_raw
            .is_some_and(|value| value != 0)
        || rendering.style.fixed_width.is_some()
    {
        return None;
    }
    // The native Model defaults an omitted size to zero before Drawing clamps it.
    let size = rendering.style.pen_size.unwrap_or(0.0);
    if !size.is_finite() {
        return None;
    }
    // Native Drawing applies Pen::SetSize before selecting this drawable; its
    // recovered clamp is [0.4, 800].
    let size = size.clamp(0.4, 800.0);
    // The shipped ObjectStroke overload creates this event natively, and its
    // Base default constructor initializes source to zero. That differs from
    // tool-2's source-0x1002 threshold branch.
    let source = 0;
    let ink = saved_with_boundaries(
        &stroke.points,
        size,
        u32::from(rendering.tool_type_raw),
        source,
    )?;
    Some(ink)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn fixture() -> Value {
        serde_json::from_str(include_str!(
            "../../../../conformance/defaultpen-saved-native.json"
        ))
        .unwrap()
    }

    #[test]
    fn gradual_movement_uses_native_initial_position_gate() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/defaultpen-gradual-native.json"
        ))
        .unwrap();
        for (name, case) in fixture["cases"].as_object().unwrap() {
            let points: Vec<Point> = case["points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| Point {
                    x: p[0].as_f64().unwrap(),
                    y: p[1].as_f64().unwrap(),
                })
                .collect();
            let actual = saved(&points, 10.0, 1, 0);
            if name != "stationary" {
                assert!(actual.is_none(), "{name} must retain generic geometry");
                continue;
            }
            let actual = actual.unwrap();
            let expected = case["add_points"].as_array().unwrap();
            assert_eq!(actual.len(), expected.len(), "{name}");
            for (actual, expected) in actual.iter().zip(expected) {
                for component in 0..3 {
                    let bias = if component == 2 { 0.5 } else { 0.0 };
                    let expected = expected[component].as_f64().unwrap() as f32 + bias;
                    assert!(
                        (actual[component] - expected).abs() < 0.00001,
                        "{name}: {actual:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn saved_quadratic_samples_match_native_addpoint_vectors_for_all_variants() {
        let fixture = fixture();
        for case in fixture["cases"].as_array().unwrap() {
            let input = &case["input"];
            let points = input["points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| Point {
                    x: p[0].as_f64().unwrap(),
                    y: p[1].as_f64().unwrap(),
                })
                .collect::<Vec<_>>();
            let expected = case["native"]["triples"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| {
                    [
                        p[0].as_f64().unwrap() as f32,
                        p[1].as_f64().unwrap() as f32,
                        p[2].as_f64().unwrap() as f32,
                    ]
                })
                .collect::<Vec<_>>();
            let actual = saved(
                &points,
                input["size"].as_f64().unwrap() as f32,
                input["tool_type_raw"].as_u64().unwrap() as u32,
                0,
            );
            if expected.is_empty() {
                assert!(
                    actual.is_none(),
                    "{} should remain outside this sampler",
                    case["case"]
                );
                assert_eq!(case["native"]["error_codes"], serde_json::json!([7]));
                continue;
            }
            let actual = actual.unwrap();
            assert_eq!(actual.len(), expected.len(), "{} count", case["case"]);
            for (index, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
                for component in 0..3 {
                    assert_eq!(
                        actual[component].to_bits(),
                        expected[component].to_bits(),
                        "{}[{index}][{component}]: {:?} != {:?}",
                        case["case"],
                        actual,
                        expected
                    );
                }
            }
        }
    }

    #[test]
    fn saved_samples_match_native_drawing_caller_stream() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../conformance/defaultpen-saved-caller-native.json"
        ))
        .unwrap();
        let case = &fixture["cases"]["straight_curve_tool1_default"];
        let input = &case["input"];
        let float = |value: &Value| {
            let hex = value.as_str().unwrap();
            f32::from_le_bytes(std::array::from_fn(|index| {
                u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).unwrap()
            }))
        };
        let points = input["points_f32_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|pair| Point {
                x: f64::from(float(&pair[0])),
                y: f64::from(float(&pair[1])),
            })
            .collect::<Vec<_>>();
        let sources = case["motion_event_source_returns"].as_array().unwrap();
        assert_eq!(sources.len(), 2);
        for phase in ["draw_object_stroke", "saved_redraw"] {
            let source = sources.iter().find(|row| row["phase"] == phase).unwrap();
            let source = u32::try_from(source["source_u32"].as_u64().unwrap()).unwrap();
            let actual = saved(
                &points,
                float(&input["pen_size_f32_bits"]),
                input["tool_type"].as_u64().unwrap() as u32,
                source,
            )
            .unwrap();
            // The native caller asserts equal full streams in both phases;
            // the fixture retains all float bits once, without duplicating them.
            let expected = case["streams"]["draw_object_stroke"]["values_f32_bits"]
                .as_array()
                .unwrap();
            assert_eq!(actual.len() * 3, expected.len(), "{phase}");
            for (index, (actual, expected)) in actual.iter().flatten().zip(expected).enumerate() {
                assert_eq!(
                    actual.to_bits(),
                    float(expected).to_bits(),
                    "{phase} float {index}"
                );
            }
        }
    }

    #[test]
    fn native_getrepeat_width_buckets_match_executed_helper() {
        let fixture = fixture();
        for row in fixture["repeat_matrix"].as_array().unwrap() {
            let width = row["width"].as_i64().unwrap() as i32;
            let lengths = row["lengths"].as_array().unwrap();
            let counts = row["counts"].as_array().unwrap();
            for (length, expected) in lengths.iter().zip(counts) {
                assert_eq!(
                    repeat(length.as_f64().unwrap() as f32, width),
                    expected.as_u64().unwrap() as usize,
                    "width={width}, length={length}"
                );
            }
        }
    }

    #[test]
    fn unsupported_context_is_not_mapped_to_an_approximation() {
        let points = [
            Point { x: 0.0, y: 0.0 },
            Point { x: 10.0, y: 0.0 },
            Point { x: 20.0, y: 0.0 },
            Point { x: 30.0, y: 10.0 },
        ];
        assert!(saved(&points, f32::NAN, 1, 0).is_none());
        assert!(saved(&points, -1.0, 1, 0).is_none());
        assert!(saved(&points, 10.0, 8, 0).is_none());
        assert!(saved(&points[..2], 10.0, 1, 0).is_some());
        let huge = [
            Point {
                x: f64::MAX,
                y: 0.0,
            },
            points[1],
            points[2],
        ];
        assert!(saved(&huge, 10.0, 1, 0).is_none());
        let nan = [
            Point {
                x: f64::NAN,
                y: 0.0,
            },
            points[1],
            points[2],
        ];
        assert!(saved(&nan, 10.0, 1, 0).is_none());
    }
}
