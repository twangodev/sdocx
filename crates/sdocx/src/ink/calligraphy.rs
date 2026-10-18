//! Typed numeric core for the selected Montblanc calligraphy V1 path.
//!
//! This module is intentionally not connected to production pen dispatch.
//! Inputs distinguish saved history from drawable state retained by the
//! reused native object.

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct NibState {
    pub(super) angle_degrees: f32,
    pub(super) direction: [f32; 2],
    pub(super) initial_pending: bool,
    pub(super) movement_armed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct MeshRect {
    /// Four ordered corners from the native AddPoint call.
    pub(super) corners: [[f32; 2]; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MeshBuffer {
    Core,
    FringeA,
    FringeB,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct MeshVertex {
    pub(super) position: [f32; 2],
    pub(super) coverage: [f32; 2],
}

/// Exact six-vertex sequence appended by one native RTV1::buildMesh call.
/// The constants are two-component shader coverage values, not alpha opacity.
pub(super) fn build_mesh(rect: MeshRect) -> [MeshVertex; 6] {
    let [a, b, c, d] = rect.corners;
    [
        MeshVertex {
            position: b,
            coverage: [0.0, 0.0],
        },
        MeshVertex {
            position: a,
            coverage: [0.0, 1.0],
        },
        MeshVertex {
            position: d,
            coverage: [1.0, 0.0],
        },
        MeshVertex {
            position: a,
            coverage: [0.0, 1.0],
        },
        MeshVertex {
            position: d,
            coverage: [1.0, 0.0],
        },
        MeshVertex {
            position: c,
            coverage: [1.0, 1.0],
        },
    ]
}

/// Three output buffers produced by the callback, in the order returned by
/// the native RTV1 object. Their different append counts are part of the
/// captured contract: core gets one Rect per AddPoint, each fringe gets two.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct MeshBuffers {
    pub(super) core: Vec<MeshVertex>,
    pub(super) fringe_a: Vec<MeshVertex>,
    pub(super) fringe_b: Vec<MeshVertex>,
}

impl MeshBuffers {
    pub(super) fn append(&mut self, buffer: MeshBuffer, rect: MeshRect) {
        let output = match buffer {
            MeshBuffer::Core => &mut self.core,
            MeshBuffer::FringeA => &mut self.fringe_a,
            MeshBuffer::FringeB => &mut self.fringe_b,
        };
        output.extend(build_mesh(rect));
    }
}

impl NibState {
    /// The native V1 constructor initializes these once. A cached drawable can
    /// retain this state between records; callers must not silently recreate it.
    pub(super) const fn fresh() -> Self {
        Self {
            angle_degrees: 0.0,
            direction: [1.0, 0.0],
            initial_pending: true,
            movement_armed: false,
        }
    }
}

/// Apply the finite orientation conversion and fold used by redrawLine.
/// Multiplication rounds as f32 before widening for the division by pi.
pub(super) fn orientation_target(orientation_radians: f32) -> f32 {
    let degrees = (orientation_radians * 180.0_f32) as f64 / std::f64::consts::PI;
    let angle = degrees as f32;
    if (0.0..=180.0).contains(&angle) {
        90.0 - angle
    } else if (-180.0..0.0).contains(&angle) {
        -90.0 - angle
    } else {
        0.0
    }
}

/// Advance the smoothed nib angle and return the separate angle value passed
/// to getRepeat. The native helper uses min(old, next) for that argument.
pub(super) fn advance_angle(old: f32, target: f32) -> (f32, f32) {
    let next = if (old - target).abs() > 1.0 {
        let step = if target > old { 1.0 } else { -1.0 };
        let lower = if old > 0.0 { 0.0 } else { -90.0 };
        (old + step).max(lower)
    } else if old < target {
        target
    } else {
        old
    };
    (next, old.min(next))
}

/// Native getRepeat(path_length, second_argument), where the second argument
/// changes the sampling interval. Historical redraw passes half pen size;
/// terminal endPen passes full pen size.
pub(super) fn repeat_count(path_length: f32, second_argument: f32) -> Option<u32> {
    if !path_length.is_finite() || !second_argument.is_finite() {
        return None;
    }
    let scaled = if second_argument <= 5.0 {
        path_length * 0.5
    } else if second_argument <= 10.0 {
        path_length / 3.0
    } else if second_argument <= 15.0 {
        path_length * 0.25
    } else if second_argument <= 20.0 {
        path_length / 5.0
    } else if second_argument <= 30.0 {
        path_length * 0.125
    } else if second_argument <= 40.0 {
        path_length / 7.0
    } else if second_argument <= 55.0 {
        path_length / 9.0
    } else {
        path_length / 10.0
    };
    if scaled < 0.0 || scaled >= i32::MAX as f32 {
        return None;
    }
    Some((scaled.trunc() as u32).max(2) + 10)
}

#[cfg(test)]
mod tests {
    use super::{
        MeshBuffer, MeshBuffers, MeshRect, NibState, advance_angle, orientation_target,
        repeat_count,
    };

    fn bits(hex: &str) -> u32 {
        u32::from_le_bytes(
            hex.as_bytes()
                .chunks_exact(2)
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap(),
        )
    }

    fn mesh_bits(vertices: &[super::MeshVertex]) -> Vec<u32> {
        vertices
            .iter()
            .flat_map(|vertex| {
                [
                    vertex.position[0],
                    vertex.position[1],
                    vertex.coverage[0],
                    vertex.coverage[1],
                ]
            })
            .map(f32::to_bits)
            .collect()
    }

    #[test]
    fn native_saved_history_witness_locks_orientation_rounding_and_fresh_state() {
        // `calligraphy-saved-native.json`, produced by executing pinned APK
        // GLV1 RedrawPen through endPen. The 0.35 rad history target is exact.
        assert_eq!(orientation_target(0.35).to_bits(), 0x428b_e499);
        assert_eq!(orientation_target(0.0).to_bits(), 90.0_f32.to_bits());
        assert_eq!(NibState::fresh().angle_degrees.to_bits(), 0);
        assert_eq!(
            NibState::fresh().direction.map(f32::to_bits),
            [1.0_f32.to_bits(), 0]
        );
    }

    #[test]
    fn angle_step_and_repeat_use_the_native_second_argument() {
        let (next, repeat_angle) = advance_angle(69.94648, 49.892956);
        assert_eq!(next.to_bits(), 68.94648_f32.to_bits());
        assert_eq!(repeat_angle.to_bits(), next.to_bits());
        assert_eq!(repeat_count(12.0, 3.0), Some(16));
        assert_eq!(repeat_count(12.0, 6.0), Some(14));
        assert_eq!(repeat_count(12.0, 14.0), Some(13));
    }

    #[test]
    fn captured_build_mesh_rectangles_reproduce_all_native_buffers() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/calligraphy-saved-native.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            for (run_name, run) in case["native"].as_object().unwrap() {
                let calls = run["build_calls"].as_array().unwrap();
                let mut mesh = MeshBuffers::default();
                for call in calls {
                    let corners: Vec<f32> = call["rect_bits"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|value| f32::from_bits(bits(value.as_str().unwrap())))
                        .collect();
                    let rect = MeshRect {
                        corners: [
                            [corners[0], corners[1]],
                            [corners[2], corners[3]],
                            [corners[4], corners[5]],
                            [corners[6], corners[7]],
                        ],
                    };
                    let buffer = match call["buffer"].as_str().unwrap() {
                        "mesh_200" => MeshBuffer::Core,
                        "mesh_208" => MeshBuffer::FringeA,
                        "mesh_216" => MeshBuffer::FringeB,
                        label => panic!("unknown native output buffer {label}"),
                    };
                    mesh.append(buffer, rect);
                }
                for (label, actual) in [
                    ("mesh_200", mesh_bits(&mesh.core)),
                    ("mesh_208", mesh_bits(&mesh.fringe_a)),
                    ("mesh_216", mesh_bits(&mesh.fringe_b)),
                ] {
                    let expected: Vec<u32> = run["meshes"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|item| item["buffer"] == label)
                        .unwrap()["vertex_f32_bits"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|value| bits(value.as_str().unwrap()))
                        .collect();
                    assert_eq!(actual, expected, "{} {run_name} {label}", case["id"]);
                }
            }
        }
    }
}
