//! Test-only Montblanc GLV1 sampler for the four captured three-point routes.
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
    // getRepeatEff(float,float), DSO RVA 0x1f3ec. For S1 <= 2.0, the
    // 0x1f48c..0x1f4b0 path computes FCVTZS(length), max(count,2), +1,
    // converts to f32, multiplies by the pinned 1.2f literal, then truncates.
    if previous_full_width > 2.0 {
        return Err(Error::UnsupportedWidthBranch);
    }
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

fn sample(input: Input<'_>) -> Result<Vec<RtRecord>, Error> {
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
    let count = repeat_count(curve.length, previous_full_width)?;

    // endPen raises the full-width target to at least 5 and increments it by
    // (target - previous)/count. drawPoint clamps the emitted half-width to 1.
    let target_width = (input.pen_size * input.pressure[1]).max(5.0_f32);
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
mod tests {
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

    #[test]
    fn rejects_unobserved_point_and_repeat_branches() {
        assert_eq!(
            sample(Input {
                points: &[[0.0, 0.0], [1.0, 0.0]],
                pressure: &[0.5, 0.5],
                pen_size: 2.0,
            })
            .unwrap_err(),
            Error::PointCount
        );
        assert_eq!(
            repeat_count(20.0, 2.5).unwrap_err(),
            Error::UnsupportedWidthBranch
        );
    }
}
