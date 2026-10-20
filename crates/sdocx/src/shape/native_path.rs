use super::{NativePathCommand, visit_path};
use crate::Result;

const OVAL_DIAGONAL: f32 = f32::from_bits(0x3f35_04f3);
const OVAL_KAPPA: f32 = f32::from_bits(0x3ed4_13cd);
const MODEL_FULL_TURN: f32 = f32::from_bits(0x43b3_feb8);
const MODEL_LINK_EPSILON: f64 = 0.0005;
const MODEL_PI: f64 = f64::from_bits(0x4009_21fb_5444_2d18);

/// The common Model loader's derived path and orientation after normalizing a
/// saved shape path. `source` remains the original WDoc byte stream.
pub(crate) struct ModelPath {
    #[cfg_attr(not(feature = "render"), allow(dead_code))]
    pub(crate) commands: Vec<NativePathCommand>,
    #[allow(dead_code)] // Capture tests validate Model's derived orientation.
    pub(crate) horizontal_flip: bool,
    #[allow(dead_code)] // Parsed NativeShape retains serialized property bits.
    pub(crate) vertical_flip: bool,
}

/// Parse and normalize the ordinary saved-shape route through Model's common
/// loader. `input_frame` is true for paths read from a saved WDoc shape.
/// Unknown verbs, malformed counts, unrepresentable helper inputs, and
/// trailing bytes reject the whole path.
pub(crate) fn normalize_model_path(
    data: &[u8],
    bounds: [f64; 4],
    angle: f32,
    mut horizontal_flip: bool,
    mut vertical_flip: bool,
    input_frame: bool,
) -> Result<Option<ModelPath>> {
    if !angle.is_finite() {
        return Ok(None);
    }
    let mut source = Vec::new();
    let (consumed, _) = visit_path(data, |command| source.push(command))?;
    if data.len() < 4 {
        return Ok(None);
    }
    let declared = u32::from_le_bytes(data[..4].try_into().expect("four-byte count")) as usize;
    if declared == 0 || source.len() != declared || consumed != data.len() {
        return Ok(None);
    }

    let mut commands = Vec::with_capacity(source.len());
    let mut previous_source = None;
    for command in source {
        let can_link = previous_source.is_some_and(|previous| {
            !matches!(
                previous,
                NativePathCommand::Arc(_) | NativePathCommand::Oval(_) | NativePathCommand::Close
            )
        });
        match command {
            NativePathCommand::Arc(values) => {
                let values = values.map(|value| value as f32);
                if values.iter().any(|value| !value.is_finite()) {
                    return Ok(None);
                }
                let left = values[0];
                let top = values[1];
                let right = left + values[2];
                let bottom = top + values[3];
                let raw_sweep = values[5];
                let sweep = if raw_sweep.to_bits() == 360.0f32.to_bits() {
                    MODEL_FULL_TURN
                } else {
                    raw_sweep
                };
                let points =
                    match super::model_arc::expand([left, top, right, bottom, values[4], sweep]) {
                        Some(points) if !points.is_empty() => points,
                        _ => return Ok(None),
                    };
                let full_turn = raw_sweep.to_bits() == 360.0f32.to_bits();
                let allow_link = can_link && !commands.is_empty();
                append_model_arc(&mut commands, points, full_turn, allow_link);
            }
            NativePathCommand::Oval(values) => {
                let [x1, y1, x2, y2] = values.map(|value| value as f32);
                if ![x1, y1, x2, y2].into_iter().all(f32::is_finite) {
                    return Ok(None);
                }
                let left = x1.min(x2);
                let top = y1.min(y2);
                let right = x1.max(x2);
                let bottom = y1.max(y2);
                let points = match super::model_arc::expand([
                    left,
                    top,
                    right,
                    bottom,
                    0.0,
                    MODEL_FULL_TURN,
                ]) {
                    Some(points) if !points.is_empty() => points,
                    _ => return Ok(None),
                };
                append_model_arc(&mut commands, points, true, false);
            }
            command => match narrow_model_command(command) {
                Some(command) => commands.push(command),
                None => return Ok(None),
            },
        }
        previous_source = Some(command);
    }
    let mut normalized_bounds = bounds.map(|value| value as f32);
    if !normalized_bounds.iter().all(|value| value.is_finite()) {
        return Ok(None);
    }
    if normalized_bounds[0] > normalized_bounds[2] {
        normalized_bounds.swap(0, 2);
        horizontal_flip = !horizontal_flip;
    }
    if normalized_bounds[1] > normalized_bounds[3] {
        normalized_bounds.swap(1, 3);
        vertical_flip = !vertical_flip;
    }
    if input_frame && angle != 0.0 {
        rotate_model_commands(&mut commands, normalized_bounds, -angle);
    }
    rotate_model_commands(&mut commands, normalized_bounds, angle);
    if !commands.iter().all(model_command_finite) {
        return Ok(None);
    }
    Ok(Some(ModelPath {
        commands,
        horizontal_flip,
        vertical_flip,
    }))
}

fn append_model_arc(
    output: &mut Vec<NativePathCommand>,
    points: Vec<[f32; 2]>,
    close: bool,
    allow_link: bool,
) {
    let start = [f64::from(points[0][0]), f64::from(points[0][1])];
    let index = output.len();
    let linked = if allow_link && index > 0 {
        model_path_linked(output.last().unwrap(), start)
    } else {
        false
    };
    if linked {
        if let Some(previous) = output.last_mut() {
            snap_model_path_start(previous, start);
        }
    } else {
        output.push(NativePathCommand::Move(start));
    }
    let mut cursor = 1;
    while cursor + 1 < points.len() {
        output.push(NativePathCommand::Quadratic([
            [f64::from(points[cursor][0]), f64::from(points[cursor][1])],
            [
                f64::from(points[cursor + 1][0]),
                f64::from(points[cursor + 1][1]),
            ],
        ]));
        cursor += 2;
    }
    if close {
        output.push(NativePathCommand::Close);
    }
}

fn model_path_linked(previous: &NativePathCommand, start: [f64; 2]) -> bool {
    match previous {
        NativePathCommand::Move(point) | NativePathCommand::Line(point) => {
            (point[0] - start[0]).abs() < MODEL_LINK_EPSILON
                && (point[1] - start[1]).abs() < MODEL_LINK_EPSILON
        }
        NativePathCommand::Quadratic(points) => points[1] == start,
        NativePathCommand::Cubic(points) => points[2] == start,
        _ => false,
    }
}

fn snap_model_path_start(command: &mut NativePathCommand, start: [f64; 2]) {
    match command {
        NativePathCommand::Move(point) | NativePathCommand::Line(point) => *point = start,
        NativePathCommand::Quadratic(points) => points[1] = start,
        NativePathCommand::Cubic(points) => points[2] = start,
        _ => {}
    }
}

fn narrow_model_command(command: NativePathCommand) -> Option<NativePathCommand> {
    fn narrow(point: [f64; 2]) -> [f64; 2] {
        point.map(|value| f64::from(value as f32))
    }
    let command = match command {
        NativePathCommand::Move(point) => NativePathCommand::Move(narrow(point)),
        NativePathCommand::Line(point) => NativePathCommand::Line(narrow(point)),
        NativePathCommand::Quadratic(points) => NativePathCommand::Quadratic(points.map(narrow)),
        NativePathCommand::Cubic(points) => NativePathCommand::Cubic(points.map(narrow)),
        NativePathCommand::Close => NativePathCommand::Close,
        NativePathCommand::Arc(_) | NativePathCommand::Oval(_) => unreachable!(),
    };
    model_command_finite(&command).then_some(command)
}

fn model_command_finite(command: &NativePathCommand) -> bool {
    let finite = |point: &[f64; 2]| point.iter().all(|value| (*value as f32).is_finite());
    match command {
        NativePathCommand::Move(point) | NativePathCommand::Line(point) => finite(point),
        NativePathCommand::Quadratic(points) => points.iter().all(finite),
        NativePathCommand::Cubic(points) => points.iter().all(finite),
        NativePathCommand::Close => true,
        NativePathCommand::Arc(_) | NativePathCommand::Oval(_) => false,
    }
}

fn rotate_model_commands(commands: &mut [NativePathCommand], bounds: [f32; 4], angle: f32) {
    if angle == 0.0 {
        return;
    }
    let center = [(bounds[0] + bounds[2]) * 0.5, (bounds[1] + bounds[3]) * 0.5];
    let radians = (f64::from(angle) / 180.0) * MODEL_PI;
    let (sine, cosine) = libm::sincos(radians);
    let rotate = |point: &mut [f64; 2]| {
        let x = point[0] as f32;
        let y = point[1] as f32;
        let dx = f64::from(x - center[0]);
        let dy = f64::from(y - center[1]);
        let rx = dx.mul_add(cosine, -(sine * dy));
        let cosine_dy = cosine * dy;
        let ry = dx.mul_add(sine, cosine_dy);
        point[0] = f64::from(center[0] + rx as f32);
        point[1] = f64::from(center[1] + ry as f32);
    };
    for command in commands {
        match command {
            NativePathCommand::Move(point) | NativePathCommand::Line(point) => rotate(point),
            NativePathCommand::Quadratic(points) => points.iter_mut().for_each(rotate),
            NativePathCommand::Cubic(points) => points.iter_mut().for_each(rotate),
            NativePathCommand::Close => {}
            NativePathCommand::Arc(_) | NativePathCommand::Oval(_) => unreachable!(),
        }
    }
}

/// Visit the path commands accepted by the native Drawing consumer.
///
/// Saved bytes remain untouched. Oval commands are expanded to the bundled
/// Skia path's Move/eight-Quadratic/Close sequence. Arc expands through the
/// recovered quadratic builder, always starting a new contour.
pub(crate) fn visit_drawing_path(
    data: &[u8],
    mut visitor: impl FnMut(NativePathCommand),
) -> Result<(usize, bool)> {
    if data.len() < 4 {
        return visit_path(data, visitor);
    }
    let declared = u32::from_le_bytes(data[..4].try_into().expect("four-byte count")) as usize;
    let mut visited = 0usize;
    let mut supported = true;
    let mut emit = |command| visitor(command);
    let (consumed, _) = visit_path(data, |command| {
        visited += 1;
        match command {
            NativePathCommand::Oval(bounds) => {
                if let Some(commands) = oval_commands(bounds) {
                    for command in commands {
                        emit(command);
                    }
                } else {
                    supported = false;
                }
            }
            NativePathCommand::Arc(values) => {
                if let Some(commands) = super::native_arc::expand(values) {
                    for command in commands {
                        emit(command);
                    }
                } else {
                    supported = false;
                }
            }
            _ => emit(command),
        }
    })?;
    Ok((consumed, supported && declared != 0 && visited == declared))
}

fn oval_commands(bounds: [f64; 4]) -> Option<[NativePathCommand; 10]> {
    let [left, top, right, bottom] = bounds.map(|value| value as f32);
    if ![left, top, right, bottom].into_iter().all(f32::is_finite) {
        return None;
    }

    // Keep the native f32 operation order. In particular, do not derive the
    // center from left + (right - left) / 2: Skia adds the corners first.
    let rx = (right - left) * 0.5;
    let ry = (bottom - top) * 0.5;
    let cx = (left + right) * 0.5;
    let cy = (top + bottom) * 0.5;
    let d = OVAL_DIAGONAL;
    let k = OVAL_KAPPA;
    let commands = [
        NativePathCommand::Move([f64::from(right), f64::from(cy)]),
        NativePathCommand::Quadratic([
            [f64::from(right), f64::from(cy + ry * k)],
            [f64::from(cx + rx * d), f64::from(cy + ry * d)],
        ]),
        NativePathCommand::Quadratic([
            [f64::from(cx + rx * k), f64::from(bottom)],
            [f64::from(cx), f64::from(bottom)],
        ]),
        NativePathCommand::Quadratic([
            [f64::from(cx - rx * k), f64::from(bottom)],
            [f64::from(cx - rx * d), f64::from(cy + ry * d)],
        ]),
        NativePathCommand::Quadratic([
            [f64::from(left), f64::from(cy + ry * k)],
            [f64::from(left), f64::from(cy)],
        ]),
        NativePathCommand::Quadratic([
            [f64::from(left), f64::from(cy - ry * k)],
            [f64::from(cx - rx * d), f64::from(cy - ry * d)],
        ]),
        NativePathCommand::Quadratic([
            [f64::from(cx - rx * k), f64::from(top)],
            [f64::from(cx), f64::from(top)],
        ]),
        NativePathCommand::Quadratic([
            [f64::from(cx + rx * k), f64::from(top)],
            [f64::from(cx + rx * d), f64::from(cy - ry * d)],
        ]),
        NativePathCommand::Quadratic([
            [f64::from(right), f64::from(cy - ry * k)],
            [f64::from(right), f64::from(cy)],
        ]),
        NativePathCommand::Close,
    ];
    let finite = commands.iter().all(|command| match command {
        NativePathCommand::Move(point) | NativePathCommand::Line(point) => {
            point.iter().all(|value| (*value as f32).is_finite())
        }
        NativePathCommand::Quadratic(points) => points
            .iter()
            .flatten()
            .all(|value| (*value as f32).is_finite()),
        NativePathCommand::Cubic(points) => points
            .iter()
            .flatten()
            .all(|value| (*value as f32).is_finite()),
        NativePathCommand::Arc(values) => values.iter().all(|value| (*value as f32).is_finite()),
        NativePathCommand::Close | NativePathCommand::Oval(_) => true,
    });
    finite.then_some(commands)
}

#[cfg(test)]
mod tests {
    use super::visit_drawing_path;
    use crate::shape::NativePathCommand;

    fn native_path(rect: [f32; 4]) -> Vec<u8> {
        let mut bytes = 1_u32.to_le_bytes().to_vec();
        bytes.push(7);
        for value in rect {
            bytes.extend(f64::from(value).to_le_bytes());
        }
        bytes
    }

    fn actual_commands(data: &[u8]) -> Vec<(u8, Vec<u32>)> {
        let mut commands = Vec::new();
        let (consumed, supported) = visit_drawing_path(data, |command| {
            let (verb, points): (u8, Vec<f32>) = match command {
                NativePathCommand::Move([x, y]) => (0, vec![x as f32, y as f32]),
                NativePathCommand::Quadratic([[cx, cy], [x, y]]) => {
                    (2, vec![cx as f32, cy as f32, x as f32, y as f32])
                }
                NativePathCommand::Close => (5, vec![]),
                _ => panic!("unexpected expanded oval command: {command:?}"),
            };
            commands.push((verb, points.into_iter().map(f32::to_bits).collect()));
        })
        .unwrap();
        assert_eq!(consumed, data.len());
        assert!(supported);
        commands
    }

    #[test]
    fn oval_expansion_matches_captured_skpath_commands() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../../conformance/shape-paths.json")).unwrap();
        let paths = fixture["capture"]["paths"].as_array().unwrap();
        let mut compared = 0;
        for path in paths {
            if path["operation"] != "add_oval"
                || path["seeded_with_move_line"] == true
                || path["direction_or_force_move"] != 1
            {
                continue;
            }
            let rect: [f32; 4] = path["rect_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|bits| f32::from_bits(bits.as_u64().unwrap() as u32))
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();
            let actual = actual_commands(&native_path(rect));
            let expected: Vec<(u8, Vec<u32>)> = path["commands"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|command| {
                    let verb = command["verb"].as_u64().unwrap() as u8;
                    if verb == 6 {
                        return None;
                    }
                    let mut bits: Vec<u32> = command["point_bits"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|value| value.as_u64().unwrap() as u32)
                        .collect();
                    if verb == 2 {
                        // RawIter's quad tuple begins with its current point;
                        // the drawing visitor exposes only control and end.
                        bits.drain(..2);
                    }
                    Some((verb, bits))
                })
                .collect();
            assert_eq!(actual, expected, "native oval capture {}", path["name"]);
            compared += 1;
        }
        assert!(
            compared >= 4,
            "expected broad Drawing-direction oval fixture coverage"
        );
    }

    #[test]
    fn arc_expands_and_narrowing_overflow_is_rejected() {
        let mut arc = 1_u32.to_le_bytes().to_vec();
        arc.push(5);
        for value in [0.0_f64; 6] {
            arc.extend(value.to_le_bytes());
        }
        assert!(visit_drawing_path(&arc, |_| {}).unwrap().1);

        // A true f64->f32 overflow is tested directly because the WDoc parser
        // itself admits only finite f64.
        let too_large = 1.0e300_f64;
        let mut overflow = 1_u32.to_le_bytes().to_vec();
        overflow.push(7);
        for value in [0.0, 0.0, too_large, too_large] {
            overflow.extend(value.to_le_bytes());
        }
        assert!(!visit_drawing_path(&overflow, |_| {}).unwrap().1);
    }

    #[test]
    fn inverted_arc_is_a_noop_and_preserves_a_following_line() {
        let mut arc = vec![5];
        for value in [40.0_f64, 0.0, 20.0, 40.0, 0.0, 90.0] {
            arc.extend(value.to_le_bytes());
        }
        let mut line = vec![2];
        line.extend(8.0_f64.to_le_bytes());
        line.extend(9.0_f64.to_le_bytes());
        let mut alone = 1_u32.to_le_bytes().to_vec();
        alone.extend(&arc);
        let mut commands = Vec::new();
        assert!(visit_drawing_path(&alone, |c| commands.push(c)).unwrap().1);
        assert!(commands.is_empty());

        let mut unseeded = 2_u32.to_le_bytes().to_vec();
        unseeded.extend(&arc);
        unseeded.extend(&line);
        let mut commands = Vec::new();
        assert!(
            visit_drawing_path(&unseeded, |c| commands.push(c))
                .unwrap()
                .1
        );
        assert!(matches!(
            commands.as_slice(),
            [crate::shape::NativePathCommand::Line([8., 9.])]
        ));

        let mut seeded = 3_u32.to_le_bytes().to_vec();
        seeded.push(1);
        seeded.extend(3.0_f64.to_le_bytes());
        seeded.extend(4.0_f64.to_le_bytes());
        seeded.extend(&arc);
        seeded.extend(&line);
        let mut commands = Vec::new();
        assert!(visit_drawing_path(&seeded, |c| commands.push(c)).unwrap().1);
        assert!(matches!(
            commands.as_slice(),
            [
                crate::shape::NativePathCommand::Move([3., 4.]),
                crate::shape::NativePathCommand::Line([8., 9.])
            ]
        ));
    }
}
