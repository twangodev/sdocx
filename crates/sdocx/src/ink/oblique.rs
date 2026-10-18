//! Typed Oblique V4 saved-redraw geometry; production preparation remains gated.

use super::path::{Quad, P};
use std::ops::Range;

pub(super) const DEFAULT_K: f32 = f32::from_bits(0x3f35_04f3);
// Defensive process budgets, not native admission thresholds.
const MAX_INPUT_POINTS: usize = 65_536;
const MAX_PATH_SAMPLES: usize = 1_000_000;
const MAX_OUTPUT_TRIANGLES: usize = 1_000_000;

/// Explicit runtime values read by the native redraw and AddPoint paths.
#[derive(Clone, Copy, Debug)]
pub(super) struct ObliqueRuntime {
    pub(super) k: f32,
    pub(super) inverse_scale: [f32; 2],
}

/// Saved inputs consumed by the reconstructed V4 redraw path.
#[derive(Clone, Copy, Debug)]
pub(super) struct ObliqueInput<'a> {
    pub(super) points: &'a [P],
    pub(super) size: f32,
    pub(super) curve_enabled: bool,
    pub(super) tool_type: u32,
    pub(super) runtime: ObliqueRuntime,
}

impl<'a> ObliqueInput<'a> {
    pub(super) fn new(
        points: &'a [P],
        size: f32,
        curve_enabled: bool,
        tool_type: u32,
        runtime: ObliqueRuntime,
    ) -> Self {
        Self {
            points,
            size,
            curve_enabled,
            tool_type,
            runtime,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct CoverageVertex {
    pub(super) position: P,
    pub(super) coverage: f32,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct CoverageTriangle(pub(super) [CoverageVertex; 3]);

/// Native callback triangles and boundaries, expressed in triangle units.
#[derive(Clone, Debug)]
pub(super) struct PreparedOblique {
    pub(super) triangles: Vec<CoverageTriangle>,
    /// Exclusive triangle count after each saved input-sample attempt.
    pub(super) sample_ends: Vec<usize>,
    /// Terminal endPen output, kept separate from saved-sample attribution.
    pub(super) terminal: Range<usize>,
}

fn repeat_count(width: f32, length: f32) -> Option<usize> {
    if !width.is_finite() || !length.is_finite() || width < 0.0 || length < 0.0 {
        return None;
    }
    let scaled = if width <= 5.0 {
        length * 0.5
    } else if width <= 10.0 {
        length / 3.0
    } else if width <= 15.0 {
        length * 0.25
    } else if width <= 20.0 {
        length / 5.0
    } else if width <= 30.0 {
        length * 0.125
    } else if width <= 40.0 {
        length / 7.0
    } else if width <= 55.0 {
        length / 9.0
    } else {
        length / 10.0
    };
    let count = (scaled.trunc().max(2.0) as i32).saturating_add(10);
    let count = usize::try_from(count).ok()?;
    (count <= MAX_PATH_SAMPLES).then_some(count)
}

fn reserve_samples(used: &mut usize, count: usize) -> Option<()> {
    *used = used.checked_add(count)?;
    (*used <= MAX_PATH_SAMPLES).then_some(())
}

fn quadratic_samples(
    start: P,
    control: P,
    end: P,
    width: f32,
    terminal: bool,
    used: &mut usize,
) -> Option<Vec<P>> {
    let path = Quad::new(start, control, end);
    // SmPath::getPosTan returns false for a zero-length path. The native
    // redrawLine caller still emits its saved endpoint once after exhausting
    // those failed samples; it does not evaluate a NaN parameter or emit the
    // requested repeat count.
    if path.length == 0.0 {
        reserve_samples(used, 1)?;
        return Some(vec![start]);
    }
    let count = repeat_count(width, path.length)?;
    let step = path.length / count as f32;
    let samples = if terminal { count + 1 } else { count };
    reserve_samples(used, samples + 1)?;
    let mut distance = 0.0_f32;
    let mut out = Vec::with_capacity(samples as usize + 1);
    for _ in 0..samples {
        out.push(path.at(distance));
        distance += step;
    }
    out.push(end);
    Some(out)
}

fn line_samples(start: P, end: P, width: f32, used: &mut usize) -> Option<Vec<P>> {
    let delta = end.sub(start);
    let length = delta.len();
    // The native no-curve loop skips getPosTan samples when SmPath reports a
    // zero-length line, then unconditionally draws the original endpoint.
    if length == 0.0 {
        reserve_samples(used, 1)?;
        return Some(vec![end]);
    }
    let count = repeat_count(width, length)?;
    reserve_samples(used, count + 1)?;
    let step = length / count as f32;
    let scale = 32767.0_f32 * f32::from_bits(0x3800_00fd);
    let mut distance = 0.0_f32;
    let mut out = Vec::with_capacity(count + 1);
    for _ in 0..count {
        let t = distance * scale / length;
        out.push(start.lerp(end, t));
        distance += step;
    }
    out.push(end);
    Some(out)
}

fn cubic_samples(start: P, c1: P, c2: P, end: P, width: f32, used: &mut usize) -> Option<Vec<P>> {
    let path = Cubic::new([start, c1, c2, end]);
    // endPen's zero-length terminal cubic takes its endpoint fallback instead
    // of calling getPosTan (which has no subdivision records to search).
    if path.length == 0.0 || path.segs.is_empty() {
        reserve_samples(used, 1)?;
        return Some(vec![end]);
    }
    let count = repeat_count(width, path.length)?;
    reserve_samples(used, count.checked_add(3)?)?;
    let step = path.length / count as f32;
    let mut distance = 0.0_f32;
    let mut out = Vec::with_capacity(count + 2);
    for _ in 0..=count {
        out.push(path.at(distance));
        distance += step;
    }
    out.push(end);
    Some(out)
}

fn saved_redraw_paths(input: ObliqueInput<'_>) -> Option<(Vec<Vec<P>>, Vec<P>)> {
    let source = input.points;
    let width = input.size;
    if source.len() < 2 || source.len() > MAX_INPUT_POINTS {
        return None;
    }
    let mut used_samples = 0;
    if !input.curve_enabled {
        let saved = source
            .windows(2)
            .map(|pair| line_samples(pair[0], pair[1], width, &mut used_samples))
            .collect::<Option<Vec<_>>>()?;
        return Some((saved, vec![*source.last()?]));
    }

    let threshold = if input.tool_type == 1 { 50.0 } else { 5.0 };
    let mut saved = Vec::new();
    let mut start = source[0];
    let mut control = P::default();
    let mut previous = source[0];
    let mut needs_control = true;
    let mut alternate = true;
    for &current in source.iter().skip(1) {
        if current.sub(previous).len() < threshold {
            let was_alternate = alternate;
            alternate = !alternate;
            if was_alternate {
                previous = current;
                saved.push(Vec::new());
                continue;
            }
        } else {
            alternate = true;
        }
        if needs_control {
            control = current;
            needs_control = false;
            previous = current;
            saved.push(Vec::new());
            continue;
        }
        let midpoint = control.mid(current);
        let points = quadratic_samples(start, control, midpoint, width, false, &mut used_samples)?;
        // Nondegenerate quadratics retain their last successful interior
        // sample as the next path's start. A zero-length native path has no
        // successful getPosTan sample, so its single fallback endpoint is the
        // only valid state to carry forward.
        start = if points.len() >= 2 {
            points[points.len() - 2]
        } else {
            points.last().copied().unwrap_or(start)
        };
        control = current;
        previous = current;
        saved.push(points);
    }

    let end = *source.last()?;
    if needs_control {
        Some((saved, vec![previous]))
    } else if alternate {
        let path = Cubic::new([start, control, control.mid(end), end]);
        if path.length == 0.0 {
            Some((saved, vec![end]))
        } else {
            Some((
                saved,
                cubic_samples(
                    start,
                    control,
                    control.mid(end),
                    end,
                    width,
                    &mut used_samples,
                )?,
            ))
        }
    } else {
        Some((
            saved,
            quadratic_samples(start, control, end, width, true, &mut used_samples)?,
        ))
    }
}

struct Cubic {
    p: [P; 4],
    segs: Vec<(f32, u16)>,
    length: f32,
}

impl Cubic {
    fn new(p: [P; 4]) -> Self {
        let mut path = Self {
            p,
            segs: Vec::new(),
            length: 0.0,
        };
        path.split(p, 0, 32767);
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
                self.segs.push((next, t1));
            }
        }
    }

    fn at(&self, d: f32) -> P {
        let i = self
            .segs
            .partition_point(|&(end, _)| end < d)
            .min(self.segs.len() - 1);
        let (start, t0) = if i == 0 { (0.0, 0) } else { self.segs[i - 1] };
        let (end, t1) = self.segs[i];
        let scale = f32::from_bits(0x3800_00fd);
        let t0 = t0 as f32 * scale;
        let t1 = t1 as f32 * scale;
        let t = t0 + (d - start) * (t1 - t0) / (end - start);
        let eval = |a: f32, b: f32, c: f32, e: f32| {
            if t == 0.0 {
                return a;
            }
            let x = (b - a).mul_add(t, a);
            let y = (c - b).mul_add(t, b);
            let z = (e - c).mul_add(t, c);
            let q = (y - x).mul_add(t, x);
            let r = (z - y).mul_add(t, y);
            (r - q).mul_add(t, q)
        };
        P {
            x: eval(self.p[0].x, self.p[1].x, self.p[2].x, self.p[3].x),
            y: eval(self.p[0].y, self.p[1].y, self.p[2].y, self.p[3].y),
        }
    }
}

#[derive(Clone, Debug)]
struct Mesh {
    corners: [[f32; 3]; 8],
}

fn line_to_mesh(
    stored_size: u32,
    k: f32,
    inverse_scale: [f32; 2],
    previous: [f32; 2],
    current: [f32; 2],
) -> Mesh {
    let half = ((stored_size as f32) * 0.5).max(1.0) * k;
    let [px, py] = previous;
    let [cx, cy] = current;
    let q = [
        [px + half, py - half, 1.0],
        [px - half, py + half, 1.0],
        [cx + half, cy - half, 1.0],
        [cx - half, cy + half, 1.0],
    ];
    let [ix, iy] = inverse_scale;
    let dx = cx - px;
    let dy = cy - py;
    let epsilon = f32::from_bits(0x3586_37bd);
    let fringe = if dx.abs() < epsilon && dy.abs() < epsilon {
        [
            [q[0][0], q[0][1] - (iy + iy), 0.0],
            [q[1][0] - (ix + ix), q[1][1], 0.0],
            [q[2][0] + (ix + ix), q[2][1], 0.0],
            [q[3][0], q[3][1] + (iy + iy), 0.0],
        ]
    } else {
        // Keep the sequence of divides, multiplies, and FMAs used by the
        // AArch64 routine; replacing these with a vector normalization changes
        // some output bits.
        let dxn = dx / ix;
        let dyn_ = dy / iy;
        let seg_len_sq = dxn.mul_add(dxn, dyn_ * dyn_);
        let seg_len = seg_len_sq.sqrt();
        let inv_seg_len = 1.0 / seg_len;
        let nseg_x = -(dyn_ * inv_seg_len);
        let nseg_y = dxn * inv_seg_len;
        let neg_dx_norm = -(dxn * inv_seg_len);

        let ex = (q[0][0] - q[1][0]) / ix;
        let ey = (q[0][1] - q[1][1]) / iy;
        let edge_len_sq = ex.mul_add(ex, ey * ey);
        let edge_len = edge_len_sq.sqrt();
        let edge_reciprocal = 1.0 / edge_len;
        let edge_y = ey * edge_reciprocal;
        let edge_x = ex * edge_reciprocal;

        let m0y = nseg_x + edge_y;
        let m0x = edge_x - nseg_y;
        let m0_len = m0x.mul_add(m0x, m0y * m0y).sqrt();
        let (m0x, m0y) = if m0_len > 0.0 {
            let reciprocal = 1.0 / m0_len;
            (m0x * reciprocal, m0y * reciprocal)
        } else {
            (m0x, m0y)
        };
        let ox0 = (ix * m0x) + (ix * m0x);
        let oy0 = (iy * m0y) + (iy * m0y);
        let m1x = neg_dx_norm - edge_x;
        let m1y = nseg_x - edge_y;
        let m1_len = m1x.mul_add(m1x, m1y * m1y).sqrt();
        let (m1x, m1y) = if m1_len > 0.0 {
            let reciprocal = 1.0 / m1_len;
            (m1x * reciprocal, m1y * reciprocal)
        } else {
            (m1x, m1y)
        };
        let ox1 = (ix * m1x) + (ix * m1x);
        let oy1 = (iy * m1y) + (iy * m1y);
        [
            [q[0][0] + ox0, q[0][1] + oy0, 0.0],
            [q[1][0] + ox1, q[1][1] + oy1, 0.0],
            [q[2][0] - ox1, q[2][1] - oy1, 0.0],
            [q[3][0] - ox0, q[3][1] - oy0, 0.0],
        ]
    };
    Mesh {
        corners: [
            q[0], q[1], q[2], q[3], fringe[0], fringe[1], fringe[2], fringe[3],
        ],
    }
}

fn make_triangles(c: &[[f32; 3]; 8]) -> Vec<[f32; 3]> {
    let [q0, q1, q2, q3, f0, f1, f2, f3] = *c;
    let mut out = Vec::with_capacity(30);
    let epsilon = f32::from_bits(0x3586_37bd);
    // The native gate compares the two endpoint corners q0 and q2; it omits
    // the core quad when the segment collapses within epsilon on every axis.
    let separated = (q0[0] - q2[0]).abs() >= epsilon
        || (q0[1] - q2[1]).abs() >= epsilon
        || (q0[2] - q2[2]).abs() >= epsilon;
    if separated {
        out.extend([q0, q1, q2, q2, q1, q3]);
    }
    out.extend([f0, f1, q0, q0, f1, q1]);
    out.extend([q2, q3, f2, f2, q3, f3]);
    out.extend([f0, q0, f2, f2, q0, q2]);
    out.extend([q1, f1, q3, q3, f1, f3]);
    out
}

fn append_segment(
    triangles: &mut Vec<CoverageTriangle>,
    size: u32,
    runtime: ObliqueRuntime,
    previous: P,
    current: P,
) -> Option<()> {
    let mesh = line_to_mesh(
        size,
        runtime.k,
        runtime.inverse_scale,
        [previous.x, previous.y],
        [current.x, current.y],
    );
    let vertices = make_triangles(&mesh.corners);
    if vertices.iter().flatten().any(|value| !value.is_finite()) {
        return None;
    }
    let added = vertices.len() / 3;
    if triangles.len().checked_add(added)? > MAX_OUTPUT_TRIANGLES {
        return None;
    }
    for chunk in vertices.chunks_exact(3) {
        let convert = |v: [f32; 3]| CoverageVertex {
            position: P { x: v[0], y: v[1] },
            coverage: v[2],
        };
        triangles.push(CoverageTriangle([
            convert(chunk[0]),
            convert(chunk[1]),
            convert(chunk[2]),
        ]));
    }
    Some(())
}

/// Reconstruct the captured V4 saved redraw for explicit saved/runtime state.
/// Inputs outside the measured finite numeric domain or the documented
/// allocation budget are rejected as `None` rather than producing partial
/// geometry.
pub(super) fn prepare(input: ObliqueInput<'_>) -> Option<PreparedOblique> {
    if input.points.len() < 2
        || input.points.len() > MAX_INPUT_POINTS
        || input
            .points
            .iter()
            .any(|p| !p.x.is_finite() || !p.y.is_finite())
        || !input.size.is_finite()
        || input.size < 0.0
        || input.size >= 4_294_967_296.0
        || !input.runtime.k.is_finite()
        || input
            .runtime
            .inverse_scale
            .iter()
            .any(|v| !v.is_finite() || *v == 0.0)
    {
        return None;
    }
    // Native RedrawPen executes FCVTZU W9,S0 at 0x409e4. For the accepted
    // finite nonnegative f32 range, Rust's saturating float-to-u32 cast has
    // the same truncation result. The same value is later stored as RT size.
    let stored_size = input.size as u32;
    let (saved_paths, terminal) = saved_redraw_paths(input)?;
    let mut triangles = Vec::new();
    let mut sample_ends = Vec::with_capacity(saved_paths.len());
    for path in &saved_paths {
        if let Some(&first) = path.first() {
            append_segment(&mut triangles, stored_size, input.runtime, first, first)?;
            for pair in path.windows(2) {
                append_segment(&mut triangles, stored_size, input.runtime, pair[0], pair[1])?;
            }
        }
        sample_ends.push(triangles.len());
    }
    let terminal_start = triangles.len();
    if let Some(&first) = terminal.first() {
        append_segment(&mut triangles, stored_size, input.runtime, first, first)?;
        for pair in terminal.windows(2) {
            append_segment(&mut triangles, stored_size, input.runtime, pair[0], pair[1])?;
        }
    }
    let terminal_end = triangles.len();
    Some(PreparedOblique {
        triangles,
        sample_ends,
        terminal: terminal_start..terminal_end,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn bits(hex: &Value) -> u32 {
        let bytes = hex.as_str().unwrap();
        u32::from_le_bytes([
            u8::from_str_radix(&bytes[0..2], 16).unwrap(),
            u8::from_str_radix(&bytes[2..4], 16).unwrap(),
            u8::from_str_radix(&bytes[4..6], 16).unwrap(),
            u8::from_str_radix(&bytes[6..8], 16).unwrap(),
        ])
    }

    fn pair(value: &Value) -> [f32; 2] {
        let a = value.as_array().unwrap();
        [a[0].as_f64().unwrap() as f32, a[1].as_f64().unwrap() as f32]
    }

    fn input<'a>(case: &'a Value, points: &'a [P]) -> ObliqueInput<'a> {
        let native = &case["native"];
        ObliqueInput::new(
            points,
            case["size"].as_f64().unwrap() as f32,
            case["curve_enabled"].as_bool().unwrap(),
            // The native harness explicitly initializes the absent tool field to zero.
            case["tool_type"].as_u64().unwrap_or(0) as u32,
            ObliqueRuntime {
                k: case
                    .get("runtime_K")
                    .and_then(Value::as_f64)
                    .map_or(DEFAULT_K, |v| v as f32),
                inverse_scale: pair(&native["inverse_scale"]),
            },
        )
    }

    #[test]
    fn native_line_mesh_and_triangle_buffers_match_fixture_bits() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../../conformance/oblique-v4-core.json"))
                .unwrap();
        assert_eq!(fixture["schema"], "sdocx-oblique-v4-native-mesh-v2");
        for case in fixture["cases"].as_array().unwrap() {
            let size = case["native_stored_size_u32"].as_u64().unwrap() as u32;
            let k = case["runtime_K"].as_f64().unwrap() as f32;
            let mesh = line_to_mesh(
                size,
                k,
                pair(&case["inverse_scale"]),
                pair(&case["previous"]),
                pair(&case["current"]),
            );
            let expected = case["line_to_mesh_all_f32_bits"].as_array().unwrap();
            for (i, corner) in mesh.corners.iter().enumerate() {
                for axis in 0..3 {
                    assert_eq!(
                        corner[axis].to_bits(),
                        bits(&expected[i][axis]),
                        "{} corner {i} axis {axis}",
                        case["id"]
                    );
                }
            }
            let triangles = make_triangles(&mesh.corners);
            let expected = case["native_triangle_vertex_f32_bits"].as_array().unwrap();
            assert_eq!(
                triangles.len(),
                expected.len(),
                "{} vertex count",
                case["id"]
            );
            for (i, point) in triangles.iter().enumerate() {
                for axis in 0..3 {
                    assert_eq!(
                        point[axis].to_bits(),
                        bits(&expected[i][axis]),
                        "{} triangle vertex {i} axis {axis}",
                        case["id"]
                    );
                }
            }
        }
    }

    #[test]
    fn typed_saved_redraw_matches_native_addpoints_boundaries_and_triangles() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../../conformance/oblique-redraw.json"))
                .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let id = case["id"].as_str().unwrap();
            let source: Vec<P> = case["points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|point| {
                    let p = pair(point);
                    P { x: p[0], y: p[1] }
                })
                .collect();
            let input = input(case, &source);
            let (saved_paths, terminal) = saved_redraw_paths(input).unwrap();
            let mut generated: Vec<P> = saved_paths.iter().flatten().copied().collect();
            generated.extend(terminal.iter().copied());
            let native = &case["native"];
            let add_calls = native["add_point_calls"].as_array().unwrap();
            assert_eq!(generated.len(), add_calls.len(), "{id} AddPoint count");
            for (i, (actual, expected)) in generated.iter().zip(add_calls).enumerate() {
                let xy = &expected["xy_bits"];
                let expected_x = u32::from_str_radix(xy[0].as_str().unwrap(), 16).unwrap();
                let expected_y = u32::from_str_radix(xy[1].as_str().unwrap(), 16).unwrap();
                assert_eq!(actual.x.to_bits(), expected_x, "{id} AddPoint {i} x");
                assert_eq!(actual.y.to_bits(), expected_y, "{id} AddPoint {i} y");
            }
            if let Some(last) = add_calls.last() {
                let final_state = native["rt_previous_xy"].as_array().unwrap();
                for axis in 0..2 {
                    let actual = (final_state[axis].as_f64().unwrap() as f32).to_bits();
                    let expected =
                        u32::from_str_radix(last["xy_bits"][axis].as_str().unwrap(), 16).unwrap();
                    assert_eq!(actual, expected, "{id} final RTV1 previous axis {axis}");
                }
            }

            let prepared = prepare(input).unwrap();
            assert_eq!(
                input.size as u32,
                native["size_u32"].as_u64().unwrap() as u32,
                "{id} FCVTZU"
            );
            let actual_ends: Vec<_> = prepared.sample_ends.iter().map(|&n| n * 3).collect();
            let expected_ends: Vec<_> = native["sample_ends_vector3"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as usize)
                .collect();
            assert_eq!(actual_ends, expected_ends, "{id} saved sample boundaries");
            assert_eq!(
                prepared.terminal.start * 3,
                native["terminal_start_vector3"].as_u64().unwrap() as usize,
                "{id} terminal start"
            );
            assert_eq!(
                prepared.terminal.end * 3,
                native["terminal_end_vector3"].as_u64().unwrap() as usize,
                "{id} terminal end"
            );
            let expected = native["callback_triple_f32_bits"].as_array().unwrap();
            assert_eq!(
                prepared.triangles.len() * 3,
                expected.len(),
                "{id} triangle count"
            );
            for (triangle_index, triangle) in prepared.triangles.iter().enumerate() {
                for (vertex_index, vertex) in triangle.0.iter().enumerate() {
                    let i = triangle_index * 3 + vertex_index;
                    for axis in 0..3 {
                        let value = match axis {
                            0 => vertex.position.x,
                            1 => vertex.position.y,
                            _ => vertex.coverage,
                        };
                        assert_eq!(
                            value.to_bits(),
                            bits(&expected[i][axis]),
                            "{id} vertex {i} axis {axis}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn rejects_malformed_contexts_and_geometry_without_partial_output() {
        let points = [P { x: 0.0, y: 0.0 }, P { x: 8.0, y: 2.0 }];
        let base = ObliqueInput::new(
            &points,
            4.0,
            true,
            0,
            ObliqueRuntime {
                k: DEFAULT_K,
                inverse_scale: [1.0, 1.0],
            },
        );
        assert!(prepare(ObliqueInput {
            size: f32::NAN,
            ..base
        })
        .is_none());
        assert!(prepare(ObliqueInput { size: -1.0, ..base }).is_none());
        assert!(prepare(ObliqueInput {
            size: 4_294_967_296.0,
            ..base
        })
        .is_none());
        assert!(prepare(ObliqueInput {
            runtime: ObliqueRuntime {
                k: f32::INFINITY,
                inverse_scale: [1.0, 1.0]
            },
            ..base
        })
        .is_none());
        assert!(prepare(ObliqueInput {
            runtime: ObliqueRuntime {
                k: DEFAULT_K,
                inverse_scale: [1.0, 0.0]
            },
            ..base
        })
        .is_none());
        let nonfinite = [
            P { x: 0.0, y: 0.0 },
            P {
                x: f32::INFINITY,
                y: 1.0,
            },
        ];
        assert!(prepare(ObliqueInput {
            points: &nonfinite,
            ..base
        })
        .is_none());
        let huge = [
            P { x: 0.0, y: 0.0 },
            P {
                x: f32::MAX,
                y: -f32::MAX,
            },
        ];
        assert!(prepare(ObliqueInput {
            points: &huge,
            ..base
        })
        .is_none());
    }

    #[test]
    fn zero_length_cubic_uses_native_terminal_endpoint_fallback() {
        let point = P { x: 17.0, y: 23.0 };
        let mut used = 0;
        let samples = cubic_samples(point, point, point, point, 4.0, &mut used).unwrap();
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].x.to_bits(), point.x.to_bits());
        assert_eq!(samples[0].y.to_bits(), point.y.to_bits());
        assert_eq!(used, 1);
    }
}
