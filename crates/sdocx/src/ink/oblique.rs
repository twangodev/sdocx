//! Test-only reconstruction of Oblique V4's native mesh helpers.
//!
//! This module intentionally models only the direct `lineToMesh` and
//! `makeTriangles` calls captured by the hash-pinned native fixture.

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

    #[test]
    fn native_line_mesh_and_triangle_buffers_match_fixture_bits() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../../conformance/oblique-v4-core.json"))
                .unwrap();
        assert_eq!(fixture["schema"], "sdocx-oblique-v4-native-mesh-v2");
        for case in fixture["cases"].as_array().unwrap() {
            let pair = |name: &str| {
                let a = case[name].as_array().unwrap();
                [a[0].as_f64().unwrap() as f32, a[1].as_f64().unwrap() as f32]
            };
            let size = case["native_stored_size_u32"].as_u64().unwrap() as u32;
            let k = case["runtime_K"].as_f64().unwrap() as f32;
            let scale = pair("inverse_scale");
            let mesh = line_to_mesh(size, k, scale, pair("previous"), pair("current"));
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
                "{} triangle vertex count",
                case["id"]
            );
            for (i, point) in triangles.iter().enumerate() {
                for axis in 0..3 {
                    assert_eq!(
                        point[axis].to_bits(),
                        bits(&expected[i][axis]),
                        "{} triangle {i} axis {axis}",
                        case["id"]
                    );
                }
            }
        }
    }
}
