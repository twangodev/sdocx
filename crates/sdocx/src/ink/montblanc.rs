//! Test-only Montblanc GLV1 sampler for captured three-point routes.
//!
//! Source: libSPenMontblancFountainPen.so SHA-256
//! 4c808cf7a1c3685e6c99e654bb8132e3ad95db4d28fef931f6936bed3d5b8350.
//! Geometry reuses the production SmPath `Quad`; the 2,688-byte fixture is
//! the six-f32-lane RTV1 stream from frozen baseline/pressure/size/far captures.
//! Records are native AddStartPoint/AddPoint/AddEndPoint input tuples.
//! Expanded RTV1 triangle meshes are checked separately against four native captures.
//! GPU Update/Draw and painted output remain unverified by these tests.
//! This module does not enable a production Montblanc renderer.
use super::path::{P, Quad};
use sha2::{Digest, Sha256};
use std::fmt;

const NATIVE_STREAM: &[u8] = include_bytes!("../../../../conformance/montblanc-native-v1.bin");
const NATIVE_STREAM_SHA256: &str =
    "860aa1762e8a341e45a8cb9087a0c74960d9e34b304ccb19c2d09800347eaf3d";
const REPEAT_FACTOR_BITS: u32 = 0x3f99_999a; // DSO PT_LOAD literal RVA 0x101e8, loaded at code RVA 0x1f4a8
const MAX_RECORDS: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RtCall {
    Start,
    Point,
    End,
}

#[derive(Clone, Copy, Debug)]
struct RtRecord {
    call: RtCall,
    lanes: [f32; 6],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Error {
    PointCount,
    PressureCount,
    InvalidInput,
    UnsupportedWidthBranch,
    RepeatCount,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Clone, Copy)]
struct Input<'a> {
    points: &'a [[f32; 2]],
    pressure: &'a [f32],
    pen_size: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AlphaState {
    redraw_index: i32,
    end_index: i32,
    move_event_count: usize,
}

fn repeat_count(path_length: f32, previous_full_width: f32) -> Result<usize, Error> {
    // getRepeatEff(float,float), DSO RVA 0x1f3ec. Preserve the native
    // f32 divisions and multiplications before truncating the length.
    let path_length = if previous_full_width <= 2.0 {
        path_length
    } else if previous_full_width <= 3.0 {
        path_length / 1.5
    } else if previous_full_width <= 5.0 {
        path_length * 0.5
    } else if previous_full_width <= 10.0 {
        path_length / 3.0
    } else if previous_full_width <= 15.0 {
        path_length * 0.25
    } else if previous_full_width <= 20.0 {
        path_length / 5.0
    } else if previous_full_width <= 30.0 {
        path_length / 6.0
    } else if previous_full_width <= 40.0 {
        path_length / 7.0
    } else if previous_full_width <= 55.0 {
        path_length * 0.125
    } else if previous_full_width <= 80.0 {
        path_length / 9.0
    } else {
        path_length / 10.0
    };
    let count = (path_length.trunc() as i32)
        .max(2)
        .checked_add(1)
        .ok_or(Error::RepeatCount)?;
    let scaled = count as f32 * f32::from_bits(REPEAT_FACTOR_BITS);
    let repeat = scaled.trunc() as usize;
    if repeat == 0 || repeat > MAX_RECORDS {
        return Err(Error::RepeatCount);
    }
    Ok(repeat)
}

fn alpha_state(input: Input<'_>) -> AlphaState {
    // startPen initializes member+96 to 170. redrawLine computes
    // FCVTZS((170 + FCVTZS(pressure * 0.5 * 170)) / 2) from the middle point.
    let seed = 170_i32;
    let delta = ((input.pressure[1] * 0.5_f32) * seed as f32).trunc() as i32;
    let redraw_index = (seed + delta) / 2;

    // The captured 3-point saved route has one interior move event. backup_data
    // increments member+100 once per movePen; endPen initializes W24=true and
    // only recomputes it when count >= 2 (RVA 0x1d890..0x1d8e4). Thus this
    // profile takes the alpha*1.5 branch regardless of endpoint chord length.
    let move_event_count = input.points.len() - 2;
    debug_assert_eq!(move_event_count, 1);
    let end_index = ((redraw_index as f32 * 1.5_f32).min(255.0_f32)).trunc() as i32;
    AlphaState {
        redraw_index,
        end_index,
        move_event_count,
    }
}

fn width_state(
    middle: [f32; 2],
    end: [f32; 2],
    size: f32,
    end_pressure: f32,
    previous: f32,
) -> (f32, f32, f32, f32) {
    let dx = end[0] - middle[0];
    let dy = end[1] - middle[1];
    let distance = dx.mul_add(dx, dy * dy).sqrt();
    let factor = if distance < 4.2 {
        1.0
    } else if distance < 9.5 {
        ((distance + -4.2) / -5.3).mul_add(0.4, 0.4) + 0.6
    } else if distance < 35.0 {
        ((distance + -9.5) / -25.5).mul_add(0.5, 0.5) + 0.1
    } else {
        0.1
    };
    let pressure = end_pressure.min(1.0);
    let pressure_term = ((pressure * 3.0) * 0.5) * size * 0.5;
    let intermediate = size / 3.0 + pressure_term;
    let candidate = factor * intermediate;
    let bounded = if (previous - candidate).abs() > 1.0 {
        previous + if previous > candidate { -1.0 } else { 1.0 }
    } else {
        candidate
    };
    let target = bounded.max(pressure * size).max(5.0);
    (intermediate, factor, target, target.min(previous))
}

fn sample(input: Input<'_>) -> Result<Vec<RtRecord>, Error> {
    sample_with_wide_model(input, false)
}

// Experimental extrapolation of the initialized redraw state observed in the
// native controls. Matching controls do not establish arbitrary wide strokes.
fn sample_wide_for_test(input: Input<'_>) -> Result<Vec<RtRecord>, Error> {
    sample_with_wide_model(input, true)
}

fn sample_with_wide_model(input: Input<'_>, allow_wide: bool) -> Result<Vec<RtRecord>, Error> {
    if input.points.len() != 3 {
        return Err(Error::PointCount);
    }
    if input.pressure.len() != 3 {
        return Err(Error::PressureCount);
    }
    if input
        .points
        .iter()
        .flatten()
        .chain(input.pressure.iter())
        .chain([&input.pen_size])
        .any(|value| !value.is_finite())
        || input
            .pressure
            .iter()
            .any(|value| !(0.0..=1.0).contains(value))
        || input.pen_size <= 0.0
    {
        return Err(Error::InvalidInput);
    }

    if input.pen_size > 2.0 && !allow_wide {
        return Err(Error::UnsupportedWidthBranch);
    }

    // redrawLine copies PenGLData[0] through drawable+72 into drawable+104.
    // In the frozen size=1.5 and size=2.0 captures that value equals Pen::GetSize,
    // so selected pen size supplies the prior full width without an opaque state.
    let previous_full_width = input.pen_size;
    let alpha = alpha_state(input);
    let point = |xy: [f32; 2]| P { x: xy[0], y: xy[1] };
    let curve = Quad::new(
        point(input.points[0]),
        point(input.points[1]),
        point(input.points[2]),
    );
    if !curve.length.is_finite() || curve.length <= 0.0 {
        return Err(Error::InvalidInput);
    }
    let (_, _, target_width, repeat_width) = width_state(
        input.points[1],
        input.points[2],
        input.pen_size,
        input.pressure[2],
        previous_full_width,
    );
    let count = repeat_count(curve.length, repeat_width)?;

    // endPen increments the full width by
    // (target - previous)/count. drawPoint clamps the emitted half-width to 1.
    let width_step = (target_width - previous_full_width) / count as f32;
    let distance_step = curve.length / count as f32;
    let mut width = previous_full_width;
    let mut distance = 0.0_f32;
    let mut records = Vec::with_capacity(count);

    for index in 0..count {
        let position = curve.at(distance);
        let tangent = curve.normalized_tangent(distance);
        let call = if index == 0 {
            RtCall::Start
        } else if index + 1 == count {
            RtCall::End
        } else {
            RtCall::Point
        };
        let alpha = if call == RtCall::Point {
            alpha.end_index as f32 / 255.0_f32
        } else {
            1.0_f32
        };
        records.push(RtRecord {
            call,
            lanes: [
                position.x,
                position.y,
                (width * 0.5_f32).max(1.0_f32),
                tangent.x,
                tangent.y,
                alpha,
            ],
        });
        width += width_step;
        distance += distance_step;
    }
    Ok(records)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    struct NativeCase {
        name: &'static str,
        points: &'static [[f32; 2]],
        pressure: &'static [f32],
        size: f32,
        byte_offset: usize,
        records: usize,
        end_alpha: i32,
    }

    // Frozen receipts: baseline f6cffcf79bb2a950d22949c1ae19dc39f2496386d537cc121ac73c797404cdea;
    // pressure 18a0901c4ef0fd9514d224a9c81a386e17a118ebc795997a04f3da58d9827308;
    // size 566c3c6164beeedb87b9a8a9d1fe2c91adc35b40afeccc12cc68a693bd9c8cb6;
    // far endpoint 5bddda13c3a37f6518d0638e5fe7e727be752de676277dc9f730d579b81c5bd5.
    const CASES: [NativeCase; 4] = [
        NativeCase {
            name: "baseline",
            points: &[[0.0, 0.0], [10.0, 0.0], [20.0, 2.0]],
            pressure: &[0.3, 0.6, 1.0],
            size: 2.0,
            byte_offset: 0,
            records: 25,
            end_alpha: 165,
        },
        NativeCase {
            name: "pressure",
            points: &[[0.0, 0.0], [10.0, 0.0], [20.0, 2.0]],
            pressure: &[0.3, 0.85, 0.9],
            size: 2.0,
            byte_offset: 25 * 24,
            records: 25,
            end_alpha: 181,
        },
        NativeCase {
            name: "size",
            points: &[[0.0, 0.0], [10.0, 0.0], [20.0, 2.0]],
            pressure: &[0.3, 0.6, 1.0],
            size: 1.5,
            byte_offset: 50 * 24,
            records: 25,
            end_alpha: 165,
        },
        NativeCase {
            name: "far_endpoint",
            points: &[[0.0, 0.0], [10.0, 0.0], [30.0, 2.0]],
            pressure: &[0.3, 0.6, 1.0],
            size: 2.0,
            byte_offset: 75 * 24,
            records: 37,
            end_alpha: 165,
        },
    ];

    fn expected_lane(case: &NativeCase, record: usize, lane: usize) -> u32 {
        let offset = case.byte_offset + record * 24 + lane * 4;
        u32::from_le_bytes(NATIVE_STREAM[offset..offset + 4].try_into().unwrap())
    }

    #[test]
    fn four_saved_routes_match_all_native_rtv1_lanes_bit_exactly() {
        assert_eq!(NATIVE_STREAM.len(), 2_688);
        assert_eq!(
            format!("{:x}", Sha256::digest(NATIVE_STREAM)),
            NATIVE_STREAM_SHA256
        );
        assert_eq!(REPEAT_FACTOR_BITS, 0x3f99_999a);

        for case in &CASES {
            let records = sample(Input {
                points: case.points,
                pressure: case.pressure,
                pen_size: case.size,
            })
            .unwrap_or_else(|error| panic!("{}: {error}", case.name));
            assert_eq!(records.len(), case.records, "{} count", case.name);
            assert_eq!(
                alpha_state(Input {
                    points: case.points,
                    pressure: case.pressure,
                    pen_size: case.size,
                })
                .end_index,
                case.end_alpha,
                "{} derived alpha",
                case.name
            );
            assert_eq!(records[0].call, RtCall::Start, "{} start", case.name);
            assert!(
                records[1..records.len() - 1]
                    .iter()
                    .all(|record| record.call == RtCall::Point),
                "{} interior call types",
                case.name
            );
            assert_eq!(
                records[records.len() - 1].call,
                RtCall::End,
                "{} end",
                case.name
            );
            for (record_index, record) in records.iter().enumerate() {
                for (lane_index, lane) in record.lanes.iter().enumerate() {
                    assert_eq!(
                        lane.to_bits(),
                        expected_lane(case, record_index, lane_index),
                        "{} record {record_index} lane {lane_index}",
                        case.name
                    );
                }
            }
        }
    }

    // Expanded source vertices, independent of the saved-path sampler above.
    // Actual native Add* bodies executed; final vectors are packed x,y,u,v,scalar.
    type Vertex = [f32; 5];

    /// AddStartPoint and AddEndPoint use the same four transformed cap corners.
    /// Keep each f32 operation separate and in the order visible in both DSO bodies.
    fn cap_vertices(input: [f32; 6]) -> [Vertex; 6] {
        let [x, y, z, dx, dy, scalar] = input;
        let h = z + 0.5_f32;
        let dx_minus_dy = dx - dy;
        let dy_plus_dx = dy + dx;
        let x_a = x + h * dx_minus_dy;
        let y_a = y + h * dy_plus_dx;
        let x_b = x + h * dy_plus_dx;
        let y_b = y - h * dx_minus_dy;
        let x_c = x - h * dy_plus_dx;
        let y_c = y + h * dx_minus_dy;
        let x_d = x - h * dx_minus_dy;
        let y_d = y - h * dy_plus_dx;
        [
            [x_a, y_a, 0.0, 0.0, scalar],
            [x_b, y_b, 1.0, 0.0, scalar],
            [x_c, y_c, 0.0, 1.0, scalar],
            [x_c, y_c, 0.0, 1.0, scalar],
            [x_d, y_d, 1.0, 1.0, scalar],
            [x_b, y_b, 1.0, 0.0, scalar],
        ]
    }

    /// One AddPoint contributes exactly six vertices to the main vector.
    /// These equations and triangle/UV order follow the sequential fmul/fadd/fsub
    /// instructions in DSO RVA 0x20990..0x21a1c; no fused multiply-add is used.
    fn point_vertices(input: [f32; 6]) -> [Vertex; 6] {
        let [x, y, z, dx, dy, scalar] = input;

        let z_dx_minus_dy = z * (dx - dy);
        let half_dy = dy * 0.5_f32;
        let a_x = (x + z_dx_minus_dy) - half_dy;

        let z_dy_plus_dx = z * (dy + dx);
        let negative_half_dx = dx * -0.5_f32;
        let a_y = (y + z_dy_plus_dx) - negative_half_dx;

        let b_x = (x + z_dy_plus_dx) + half_dy;
        let z_dy_minus_dx = z * (dy - dx);
        let b_y = negative_half_dx + (y + z_dy_minus_dx);

        let c_x = (x - z_dy_plus_dx) - half_dy;
        let half_dx = dx * 0.5_f32;
        let c_y = (y + z_dx_minus_dy) + half_dx;

        let d_x = (x - z_dx_minus_dy) + half_dy;
        let z_dx_plus_dy = z * (dx + dy);
        let d_y = (y - z_dx_plus_dy) - half_dx;

        [
            [a_x, a_y, 0.0, 1.0, scalar],
            [b_x, b_y, 1.0, 1.0, scalar],
            [c_x, c_y, 0.0, 0.0, scalar],
            [c_x, c_y, 0.0, 0.0, scalar],
            [d_x, d_y, 1.0, 0.0, scalar],
            [b_x, b_y, 1.0, 1.0, scalar],
        ]
    }

    #[test]
    fn saved_routes_match_native_expanded_triangle_meshes() {
        // Root-native mesh capture V2; DSO SHA pinned at the module header.
        // Capture SHA-256s (baseline, pressure, size, far endpoint):
        // 7273ca60078c8fdd675d0beb874d9912b4ba08036425a234e544f49a77914e6c
        // 932996cae976178e9088e367f195b8a1ee8543ad45a525d2a148e9cc37dbe807
        // deb75a3941ca003a0af008107b8728755ccd1f554824d4a0ca43828a6688a9a7
        // b72a974e9760b1527e989b4b0d50a5337d4770306bcdd07aa7e5724220679034
        // Arrays are vector fields +200 (main), +208 (start), +216 (end).
        let expected_hashes = [
            [
                "b04896eb84c4f3b3861a5757040fffbaa3c479320f690f702add573443c64ea3",
                "ca9e3ddf742d9397424374dc0689fd4822f6f0ef20fbc63acf459c3d829dbb08",
                "9cfc4a174ce55974c2d8411793f88854517cf0a02c861e3649c7dd6330bf701f",
            ],
            [
                "f2a4d0ff16d3da795d8d5e382d77e7a56574a7717e400d28d2ebf314160b20d7",
                "ca9e3ddf742d9397424374dc0689fd4822f6f0ef20fbc63acf459c3d829dbb08",
                "9cfc4a174ce55974c2d8411793f88854517cf0a02c861e3649c7dd6330bf701f",
            ],
            [
                "d336c65a1c62e3896e0707035cbf35bf709dcd789049bfe5fabc760689583830",
                "ca9e3ddf742d9397424374dc0689fd4822f6f0ef20fbc63acf459c3d829dbb08",
                "20efc2c58578e8acf26e6e3a3e29cef3ea1b4423b5bbc8d27d9d89ed529e93c4",
            ],
            [
                "9fb904c77913eeabd72bd4dbaa2c05991eb6283a8838532eb46ee3d7a836ae83",
                "ca9e3ddf742d9397424374dc0689fd4822f6f0ef20fbc63acf459c3d829dbb08",
                "8328d840b6331fc319181d3cb68c9d45a77204dffb0eec1bef0d3294e9b806cf",
            ],
        ];
        for (case, expected) in CASES.iter().zip(expected_hashes) {
            let records = sample(Input {
                points: case.points,
                pressure: case.pressure,
                pen_size: case.size,
            })
            .unwrap();
            let mut meshes: [Vec<Vertex>; 3] = std::array::from_fn(|_| Vec::new());
            for record in records {
                let (index, vertices) = match record.call {
                    RtCall::Start => (1, cap_vertices(record.lanes)),
                    RtCall::Point => (0, point_vertices(record.lanes)),
                    RtCall::End => (2, cap_vertices(record.lanes)),
                };
                meshes[index].extend(vertices);
            }
            for (index, (mesh, expected_hash)) in meshes.iter().zip(expected).enumerate() {
                let expected_count = if index == 0 {
                    (case.records - 2) * 6
                } else {
                    6
                };
                assert_eq!(
                    mesh.len(),
                    expected_count,
                    "{} mesh {index} count",
                    case.name
                );
                let bytes: Vec<_> = mesh
                    .iter()
                    .flatten()
                    .flat_map(|v| v.to_le_bytes())
                    .collect();
                assert_eq!(
                    format!("{:x}", Sha256::digest(&bytes)),
                    expected_hash,
                    "{} mesh {index} bytes",
                    case.name
                );
            }
        }
    }

    #[test]
    fn source_repeat_and_alpha_controls_are_input_derived() {
        assert_eq!(repeat_count(20.09975, 2.0).unwrap(), 25);
        assert_eq!(repeat_count(20.09975, 1.5).unwrap(), 25);
        assert_eq!(repeat_count(30.076355, 2.0).unwrap(), 37);
        assert_eq!(
            alpha_state(Input {
                points: CASES[0].points,
                pressure: CASES[0].pressure,
                pen_size: 2.0,
            }),
            AlphaState {
                redraw_index: 110,
                end_index: 165,
                move_event_count: 1,
            }
        );
        assert_eq!(
            alpha_state(Input {
                points: CASES[1].points,
                pressure: CASES[1].pressure,
                pen_size: 2.0,
            }),
            AlphaState {
                redraw_index: 121,
                end_index: 181,
                move_event_count: 1,
            }
        );
        // A distant endpoint does not alter W24 for the one-move saved route.
        let far = &CASES[3];
        assert_eq!(
            alpha_state(Input {
                points: far.points,
                pressure: far.pressure,
                pen_size: far.size
            })
            .end_index,
            alpha_state(Input {
                points: CASES[0].points,
                pressure: CASES[0].pressure,
                pen_size: CASES[0].size
            })
            .end_index
        );
    }

    #[derive(Debug)]
    pub(super) struct EndAlpha {
        pub(super) start: i32,
        pub(super) step: f32,
    }

    // The no-move and upper-clamp paths remain disassembly-derived.
    pub(super) fn end_alpha(
        prior: i32,
        target: i32,
        last_move_distance: Option<f32>,
        repeat: u32,
    ) -> EndAlpha {
        assert!(repeat > 0);
        let (start, target) = if last_move_distance.is_none_or(|distance| distance < 15.0) {
            let boosted = ((prior as f32 * 1.5).min(255.0)).trunc() as i32;
            (boosted, boosted)
        } else {
            (prior, target)
        };
        EndAlpha {
            start,
            step: (target - start) as f32 / repeat as f32,
        }
    }

    #[test]
    fn end_alpha_interpolation_matches_native_registers() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/montblanc-end-alpha-trace.json"
        ))
        .unwrap();
        let cases = fixture["cases"].as_object().unwrap();
        assert_eq!(cases.len(), 9);
        for (name, case) in cases {
            let rows = case["rows"].as_array().unwrap();
            let row = |rva: &str| rows.iter().find(|r| r["rva"] == rva).unwrap();
            let integer = |r: &serde_json::Value, reg: &str| {
                u32::from_str_radix(
                    r["x_registers"][reg]
                        .as_str()
                        .unwrap()
                        .trim_start_matches("0x"),
                    16,
                )
                .unwrap() as i32
            };
            let float = |r: &serde_json::Value, reg: &str| {
                let hex = r["s_registers"][reg].as_str().unwrap();
                let bytes = std::array::from_fn(|i| {
                    u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap()
                });
                f32::from_bits(u32::from_le_bytes(bytes))
            };
            let branch = row("0x1dc68");
            let end = row("0x1dc8c");
            let distance = float(row("0x1d8dc"), "s0_f32_bits");
            let samples: Vec<_> = rows.iter().filter(|r| r["rva"] == "0x1dcfc").collect();
            let repeat = samples[0]["loop_repeat"].as_u64().unwrap() as u32;
            let alpha = end_alpha(
                integer(branch, "x8"),
                integer(branch, "x9"),
                Some(distance),
                repeat,
            );
            assert_eq!(alpha.start, integer(end, "x8"), "{name}");
            assert_eq!(
                alpha.step.to_bits(),
                float(row("0x1dcc0"), "s11_f32_bits").to_bits(),
                "{name}"
            );
            for sample in samples {
                let iteration = sample["loop_iteration"].as_u64().unwrap() as u32;
                let actual = (iteration as f32).mul_add(alpha.step, alpha.start as f32);
                assert_eq!(
                    actual.to_bits(),
                    float(sample, "s0_f32_bits").to_bits(),
                    "{name} iteration {iteration}"
                );
            }
        }
    }

    // Synthetic five-point redraw path only; the <4.2 and output-clamp branches
    // are inferred from disassembly and remain unobserved in these controls.
    pub(super) fn redraw_alpha_candidate(
        prior: i32,
        pressure: f32,
        distance: f32,
    ) -> (f32, f32, i32) {
        let factor = if distance < 4.2 {
            1.0_f32
        } else if distance < 9.5 {
            ((distance - 4.2) / -5.3).mul_add(0.4, 0.4) + 0.6
        } else if distance < 35.0 {
            ((distance - 9.5) / -25.5).mul_add(0.5, 0.5) + 0.1
        } else {
            0.1
        };
        let original_factor = factor;
        let factor = (factor - 1.0) + 1.0;
        let seed = (170.0 + ((pressure * 0.5) * 170.0) * 0.5).trunc();
        let candidate = seed * factor;
        let next = candidate.trunc() as i32;
        let target = if (prior - next).abs() >= 11 {
            prior + if prior > next { -10 } else { 10 }
        } else {
            next
        };
        (original_factor, candidate, target.clamp(60, 255))
    }

    #[test]
    fn redraw_alpha_thresholds_match_native_registers() {
        let receipt: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/montblanc-alpha-thresholds.json"
        ))
        .unwrap();
        let cases = receipt["cases"].as_object().unwrap();
        assert_eq!(cases.len(), 11);
        for (name, case) in cases {
            let points = case["input"]["points"].as_array().unwrap();
            let coordinate =
                |point: usize, axis: usize| points[point][axis].as_f64().unwrap() as f32;
            let dx = coordinate(3, 0) - coordinate(2, 0);
            let dy = coordinate(3, 1) - coordinate(2, 1);
            let distance = dx.mul_add(dx, dy * dy).sqrt();
            let pressure = case["input"]["pressure"][3].as_f64().unwrap() as f32;
            let word = |field: &str| {
                let bytes = case[field].as_str().unwrap();
                let bytes: Vec<u8> = (0..8)
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&bytes[i..i + 2], 16).unwrap())
                    .collect();
                u32::from_le_bytes(bytes.try_into().unwrap())
            };
            let prior = f32::from_bits(word("prior_f32_bits")) as i32;
            let (factor, candidate, target) = redraw_alpha_candidate(prior, pressure, distance);
            assert_eq!(factor.to_bits(), word("factor_f32_bits"), "{name}");
            assert_eq!(candidate.to_bits(), word("candidate_f32_bits"), "{name}");
            assert_eq!(target, case["target"].as_i64().unwrap() as i32, "{name}");
        }
    }

    #[test]
    fn repeat_width_branches_match_native_length_control() {
        let receipt: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/montblanc-width-branches-native.json"
        ))
        .unwrap();
        let cases = receipt["cases"].as_object().unwrap();
        assert_eq!(cases.len(), 12);
        for width in [
            "2", "2.01", "3", "5", "10", "15", "20", "30", "40", "55", "80", "81",
        ] {
            let case = &cases[width];
            let input_width = case["input"]["pen_size"].as_f64().unwrap() as f32;
            let length = case["length"].as_f64().unwrap() as f32;
            let repeat = case["repeat"].as_u64().unwrap() as usize;
            assert_eq!(
                repeat_count(length, input_width).unwrap(),
                repeat,
                "width {width}"
            );
        }
    }

    #[test]
    fn repeat_width_boundaries_match_direct_native_probes() {
        let receipt: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../conformance/montblanc-repeat-probe-native.json"
        ))
        .unwrap();
        let cases = receipt["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 40);
        for case in cases {
            let length = f32::from_bits(case["length_bits"].as_u64().unwrap() as u32);
            let width = f32::from_bits(case["width_bits"].as_u64().unwrap() as u32);
            let repeat = case["repeat"].as_u64().unwrap() as usize;
            assert_eq!(
                repeat_count(length, width).unwrap(),
                repeat,
                "length {length}, width {width}"
            );
        }
    }

    #[test]
    fn wide_streams_match_native() {
        for receipt in [
            include_str!("../../../../conformance/montblanc-wide-paths-native.json"),
            include_str!("../../../../conformance/montblanc-width-branches-native.json"),
            include_str!("../../../../conformance/montblanc-wide-low-pressure-native.json"),
            include_str!("../../../../conformance/montblanc-wide-validation-native.json"),
        ] {
            let data: serde_json::Value = serde_json::from_str(receipt).unwrap();
            for (name, case) in data["cases"].as_object().unwrap() {
                let input = &case["input"];
                let n = |v: &serde_json::Value| v.as_f64().unwrap() as f32;
                let points: Vec<[f32; 2]> = input["points"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| [n(&p[0]), n(&p[1])])
                    .collect();
                let pressure: Vec<f32> = input["pressure"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(n)
                    .collect();
                let actual = sample_wide_for_test(Input {
                    points: &points,
                    pressure: &pressure,
                    pen_size: n(&input["pen_size"]),
                })
                .unwrap();
                let stream = case["stream"].as_array().unwrap();
                assert_eq!(actual.len(), stream.len(), "{name}");
                for (i, (record, expected)) in actual.iter().zip(stream).enumerate() {
                    let symbol = expected[0].as_str().unwrap();
                    let call = if symbol.contains("AddStartPoint") {
                        RtCall::Start
                    } else if symbol.contains("AddEndPoint") {
                        RtCall::End
                    } else {
                        assert!(symbol.contains("AddPoint"));
                        RtCall::Point
                    };
                    assert_eq!(record.call, call, "{name} record {i}");
                    assert_eq!(
                        expected[1].as_array().unwrap().len(),
                        6,
                        "{name} record {i}"
                    );
                    for (lane, word) in expected[1].as_array().unwrap().iter().enumerate() {
                        let bytes = word.as_str().unwrap();
                        let bits = u32::from_le_bytes(std::array::from_fn(|j| {
                            u8::from_str_radix(&bytes[j * 2..j * 2 + 2], 16).unwrap()
                        }));
                        assert_eq!(
                            record.lanes[lane].to_bits(),
                            bits,
                            "{name} record {i} lane {lane}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn rejects_unobserved_point_branches() {
        assert_eq!(
            sample(Input {
                points: &[[0.0, 0.0], [10.0, 0.0], [20.0, 2.0]],
                pressure: &[0.3, 0.6, 1.0],
                pen_size: 2.5
            })
            .unwrap_err(),
            Error::UnsupportedWidthBranch
        );
        assert_eq!(
            sample(Input {
                points: &[[0.0, 0.0], [1.0, 0.0]],
                pressure: &[0.5, 0.5],
                pen_size: 2.0,
            })
            .unwrap_err(),
            Error::PointCount
        );
    }
    pub(crate) fn captured_control_buffers(
        stroke: &crate::Stroke,
        case: usize,
    ) -> [Vec<[f32; 5]>; 3] {
        let expected = &CASES[case];
        let points: Vec<_> = stroke
            .points
            .iter()
            .map(|point| [point.x as f32 - 128., 128. - point.y as f32])
            .collect();
        let pressure: Vec<_> = stroke.pressures.iter().map(|value| *value as f32).collect();
        assert_eq!(points, expected.points);
        assert_eq!(pressure, expected.pressure);
        assert_eq!(stroke.pen_width, expected.size);
        let records = sample(Input {
            points: &points,
            pressure: &pressure,
            pen_size: stroke.pen_width,
        })
        .unwrap();
        assert_eq!(records.len(), expected.records);
        let mut buffers: [Vec<[f32; 5]>; 3] = std::array::from_fn(|_| Vec::new());
        for (index, record) in records.iter().enumerate() {
            for (lane_index, lane) in record.lanes.iter().enumerate() {
                assert_eq!(lane.to_bits(), expected_lane(expected, index, lane_index));
            }
            let (buffer, vertices) = match record.call {
                RtCall::Start => (1, cap_vertices(record.lanes)),
                RtCall::Point => (0, point_vertices(record.lanes)),
                RtCall::End => (2, cap_vertices(record.lanes)),
            };
            buffers[buffer].extend(vertices);
        }
        buffers
    }
}

#[cfg(test)]
#[path = "montblanc_width_test.rs"]
mod width_state_tests;

#[cfg(test)]
#[path = "montblanc_four_test.rs"]
mod four_point_tests;

#[cfg(test)]
#[path = "montblanc_five_test.rs"]
mod five_point_tests;
