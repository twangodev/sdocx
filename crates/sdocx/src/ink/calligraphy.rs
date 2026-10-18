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

/// Numeric inputs read by one RTV1 `AddPoint` call. These include its separate
/// near-zero gate center, initial-edge flag, and three cached edge pairs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct AddPointInput {
    pub(super) vector: [f32; 4],
    pub(super) extent: f32,
    pub(super) gate_center: [f32; 2],
    pub(super) initial_edge: bool,
    pub(super) previous_edges: [[[f32; 2]; 2]; 3],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct AddPointGeometry {
    pub(super) buffers: [MeshBuffer; 5],
    pub(super) rectangles: [MeshRect; 5],
    pub(super) next_edges: [[[f32; 2]; 2]; 3],
}

/// Reconstruct RTV1::AddPoint's gate and its core/fringe Rect arguments.
/// Previous centers and the cached fringe edges are explicit runtime inputs.
pub(super) fn add_point(input: AddPointInput) -> Option<AddPointGeometry> {
    let [x, y, signed_x, signed_y] = input.vector;
    let extent = input.extent.max(1.0);
    if !input.initial_edge {
        let dx = x - input.gate_center[0];
        let dy = y - input.gate_center[1];
        let distance_squared = dx.mul_add(dx, dy * dy);
        if distance_squared.sqrt() < f32::from_bits(0x3400_0000) {
            return None;
        }
    }
    let current = [
        [(-signed_x).mul_add(extent, x), signed_y.mul_add(extent, y)],
        [signed_x.mul_add(extent, x), (-signed_y).mul_add(extent, y)],
    ];
    let previous_core = if input.initial_edge {
        current
    } else {
        input.previous_edges[0]
    };
    let (normal, tangent) = if input.initial_edge {
        let diagonal = f32::from_bits(0x4010_d067);
        ([diagonal, diagonal], [diagonal, diagonal])
    } else {
        let dx = x - input.gate_center[0];
        let dy = y - input.gate_center[1];
        let normal_length = dy.mul_add(dy, dx * dx).sqrt();
        let tangent_length = dx.mul_add(dx, dy * dy).sqrt();
        let normal_inverse = 1.0_f32 / normal_length;
        let tangent_inverse = 1.0_f32 / tangent_length;
        let scale = 3.2_f32;
        (
            [
                (-dy * normal_inverse) * scale,
                (dx * normal_inverse) * scale,
            ],
            [
                (-dx * tangent_inverse) * scale,
                (-dy * tangent_inverse) * scale,
            ],
        )
    };
    let upper = current[0];
    let lower = current[1];
    let upper_plus = [upper[0] + normal[0], upper[1] + normal[1]];
    let upper_minus = [upper[0] - normal[0], upper[1] - normal[1]];
    let lower_minus = [lower[0] - normal[0], lower[1] - normal[1]];
    let lower_plus = [lower[0] + normal[0], lower[1] + normal[1]];
    let upper_tangent_minus = [upper[0] - tangent[0], upper[1] - tangent[1]];
    let upper_tangent_plus = [upper[0] + tangent[0], upper[1] + tangent[1]];
    let lower_tangent_minus = [lower[0] - tangent[0], lower[1] - tangent[1]];
    let lower_tangent_plus = [lower[0] + tangent[0], lower[1] + tangent[1]];
    let previous_lower = if input.initial_edge {
        [lower_minus, lower_plus]
    } else {
        input.previous_edges[1]
    };
    let previous_upper = if input.initial_edge {
        [upper_plus, upper_minus]
    } else {
        input.previous_edges[2]
    };
    Some(AddPointGeometry {
        buffers: [
            MeshBuffer::Core,
            MeshBuffer::FringeB,
            MeshBuffer::FringeB,
            MeshBuffer::FringeA,
            MeshBuffer::FringeA,
        ],
        rectangles: [
            MeshRect {
                corners: [previous_core[0], previous_core[1], current[0], current[1]],
            },
            MeshRect {
                corners: [
                    previous_upper[0],
                    previous_upper[1],
                    upper_plus,
                    upper_minus,
                ],
            },
            MeshRect {
                corners: [
                    previous_lower[0],
                    previous_lower[1],
                    lower_minus,
                    lower_plus,
                ],
            },
            MeshRect {
                corners: [upper_minus, upper_plus, lower_minus, lower_plus],
            },
            MeshRect {
                corners: [
                    upper_tangent_minus,
                    upper_tangent_plus,
                    lower_tangent_minus,
                    lower_tangent_plus,
                ],
            },
        ],
        next_edges: [
            current,
            [lower_minus, lower_plus],
            [upper_plus, upper_minus],
        ],
    })
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
        AddPointInput, MeshBuffer, MeshBuffers, MeshRect, NibState, add_point, advance_angle,
        orientation_target, repeat_count,
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

    fn rect_bits(rect: MeshRect) -> [u32; 8] {
        let [a, b, c, d] = rect.corners;
        [a[0], a[1], b[0], b[1], c[0], c[1], d[0], d[1]].map(f32::to_bits)
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

    #[test]
    fn captured_addpoints_reconstruct_core_edges_across_reused_drawable() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/calligraphy-saved-native.json"
        ))
        .unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            for (_run_name, run) in case["native"].as_object().unwrap() {
                let stamps = run["add_points"].as_array().unwrap();
                let all_build_calls = run["build_calls"].as_array().unwrap();
                let mut build_cursor = 0;
                for (index, stamp) in stamps.iter().enumerate() {
                    let vector: [f32; 4] = stamp["vector_bits"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|value| f32::from_bits(bits(value.as_str().unwrap())))
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap();
                    let extent = f32::from_bits(bits(stamp["extent_bits"].as_str().unwrap()));
                    let pre = &stamp["pre_state"];
                    let center: [f32; 2] = pre["center_bits"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|value| f32::from_bits(bits(value.as_str().unwrap())))
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap();
                    let edge: Vec<f32> = pre["edge_state_bits"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|value| f32::from_bits(bits(value.as_str().unwrap())))
                        .collect();
                    let previous_edges: [[[f32; 2]; 2]; 3] = edge
                        .chunks_exact(4)
                        .map(|v| [[v[0], v[1]], [v[2], v[3]]])
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap();
                    let input = AddPointInput {
                        vector,
                        extent,
                        gate_center: center,
                        initial_edge: pre["initial_flag"].as_u64().unwrap() != 0,
                        previous_edges,
                    };
                    let actual = add_point(input);
                    let expected_builds = stamp["build_calls"].as_u64().unwrap() as usize;
                    assert_eq!(
                        actual.is_some(),
                        expected_builds == 5,
                        "{} {} AddPoint {index} emission gate",
                        case["id"],
                        _run_name
                    );
                    if let Some(geometry) = actual {
                        assert_eq!(expected_builds, 5);
                        for slot in 0..5 {
                            let native_rect = &all_build_calls[build_cursor + slot];
                            let buffer = match geometry.buffers[slot] {
                                MeshBuffer::Core => "mesh_200",
                                MeshBuffer::FringeA => "mesh_208",
                                MeshBuffer::FringeB => "mesh_216",
                            };
                            assert_eq!(native_rect["buffer"], buffer);
                            let expected: [u32; 8] = native_rect["rect_bits"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|value| bits(value.as_str().unwrap()))
                                .collect::<Vec<_>>()
                                .try_into()
                                .unwrap();
                            if case["id"] == "five_history_phase_boundaries"
                                && index > 0
                                && slot == 0
                            {
                                let cached_core: [u32; 4] = previous_edges[0]
                                    .iter()
                                    .flat_map(|point| point.iter().copied())
                                    .map(f32::to_bits)
                                    .collect::<Vec<_>>()
                                    .try_into()
                                    .unwrap();
                                assert_eq!(&expected[..4], cached_core);
                                let current_core = [
                                    (-vector[2]).mul_add(extent, vector[0]),
                                    vector[3].mul_add(extent, vector[1]),
                                    vector[2].mul_add(extent, vector[0]),
                                    (-vector[3]).mul_add(extent, vector[1]),
                                ]
                                .map(f32::to_bits);
                                assert_ne!(cached_core, current_core);
                            }
                            assert_eq!(
                                rect_bits(geometry.rectangles[slot]),
                                expected,
                                "{} {} AddPoint {index} rect {slot}",
                                case["id"],
                                _run_name
                            );
                        }
                        if let Some(next_stamp) = stamps.get(index + 1) {
                            let next_edges: Vec<u32> = next_stamp["pre_state"]["edge_state_bits"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|value| bits(value.as_str().unwrap()))
                                .collect();
                            let actual_edges: Vec<u32> = geometry
                                .next_edges
                                .iter()
                                .flat_map(|pair| {
                                    pair.iter().flat_map(|point| point.iter().copied())
                                })
                                .map(f32::to_bits)
                                .collect();
                            assert_eq!(
                                actual_edges, next_edges,
                                "{} {} AddPoint {index} edge cache",
                                case["id"], _run_name
                            );
                        }
                    } else {
                        assert_eq!(expected_builds, 0);
                    }
                    build_cursor += expected_builds;
                }
                assert_eq!(build_cursor, all_build_calls.len());
            }
        }
    }
    #[test]
    fn direct_native_half_extent_is_clamped_by_addpoint() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/calligraphy-addpoint-direct-native.json"
        ))
        .unwrap();
        let native = &fixture["case"]["native"];
        let extent = f32::from_bits(bits(native["extent_bits"].as_str().unwrap()));
        assert_eq!(extent.to_bits(), 0.5_f32.to_bits());
        let vector: [f32; 4] = native["vector_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| f32::from_bits(bits(value.as_str().unwrap())))
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        let pre = &native["pre_state"];
        let center: [f32; 2] = pre["center_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| f32::from_bits(bits(value.as_str().unwrap())))
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        let edge: Vec<f32> = pre["edge_state_bits"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| f32::from_bits(bits(value.as_str().unwrap())))
            .collect();
        let previous_edges: [[[f32; 2]; 2]; 3] = edge
            .chunks_exact(4)
            .map(|v| [[v[0], v[1]], [v[2], v[3]]])
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        let actual = add_point(AddPointInput {
            vector,
            extent,
            gate_center: center,
            initial_edge: pre["initial_flag"].as_u64().unwrap() != 0,
            previous_edges,
        })
        .unwrap();
        let rects = native["rectangles"].as_array().unwrap();
        assert_eq!(actual.rectangles.len(), rects.len());
        for (slot, (actual_rect, native_rect)) in actual.rectangles.iter().zip(rects).enumerate() {
            let expected: [u32; 8] = native_rect["rect_bits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| bits(value.as_str().unwrap()))
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();
            assert_eq!(
                rect_bits(*actual_rect),
                expected,
                "direct AddPoint Rect {slot}"
            );
            let buffer = match actual.buffers[slot] {
                MeshBuffer::Core => "mesh_200",
                MeshBuffer::FringeA => "mesh_208",
                MeshBuffer::FringeB => "mesh_216",
            };
            assert_eq!(native_rect["buffer"], buffer);
        }
        assert_eq!(
            actual.rectangles[0].corners[2][1].to_bits(),
            1.0_f32.to_bits()
        );
        assert_ne!(
            actual.rectangles[0].corners[2][1].to_bits(),
            0.5_f32.to_bits()
        );
    }

}
