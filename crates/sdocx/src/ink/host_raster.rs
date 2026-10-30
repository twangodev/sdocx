//! Triangle varying planes for the captured Mesa llvmpipe host profile.
//! See conformance/pencil2-v14-host-pipeline.json for active setup and FMA IR.

use crate::InkRasterError;

/// Preserve the captured setup order when a strip's output Y axis is reflected.
/// Equivalent triangle permutations can choose a different nearest texel in F32.
#[cfg(test)]
pub(crate) fn strip_triangles(y_up: bool) -> [[usize; 3]; 2] {
    if y_up {
        [[0, 1, 2], [2, 1, 3]]
    } else {
        [[1, 0, 2], [1, 2, 3]]
    }
}

pub(crate) fn linear_coeff(pos: [(f32, f32); 3], values: [f32; 3]) -> (f32, f32, f32) {
    let [(x0, y0), (x1, y1), (x2, y2)] = pos;
    let [a0, a1, a2] = values;
    let x0_center = x0 - 0.5;
    let y0_center = y0 - 0.5;
    let dx01 = x0 - x1;
    let dy01 = y0 - y1;
    let dx20 = x2 - x0;
    let dy20 = y2 - y0;
    let area = dx01 * dy20 - dy01 * dx20;
    let ooa = 1.0 / area;
    let dy20_ooa = dy20 * ooa;
    let dy01_ooa = dy01 * ooa;
    let dx20_ooa = dx20 * ooa;
    let dx01_ooa = dx01 * ooa;
    let da01 = a0 - a1;
    let da20 = a2 - a0;
    let dadx = da01 * dy20_ooa - da20 * dy01_ooa;
    let dady = da20 * dx01_ooa - da01 * dx20_ooa;
    let origin_offset = dadx * x0_center + dady * y0_center;
    (a0 - origin_offset, dadx, dady)
}

/// Axis-aligned strip setup for the captured Pencil2/ColoredPencil host paths.
/// The caller supplies post-viewport coordinates and vertex-hoisted varyings.
#[cfg(test)]
pub(crate) struct AxisRect<const N: usize> {
    x: [f32; 2],
    y: [f32; 2],
    height: usize,
    bounds: [usize; 4],
    planes: [[(f32, f32, f32); N]; 2],
}

#[cfg(test)]
impl<const N: usize> AxisRect<N> {
    pub(crate) fn new(
        x: [f32; 2],
        y: [f32; 2],
        frame: [u32; 2],
        values: [[f32; 4]; N],
        y_up: bool,
    ) -> Option<Self> {
        let [width, height] = frame.map(|n| n as f32);
        if x.iter().chain(&y).any(|v| !v.is_finite()) || x[0] >= x[1] || y[0] == y[1] {
            return None;
        }
        let bound = |value: f32, extent: f32| (value - 0.5).ceil().clamp(0., extent) as usize;
        let bounds = [
            bound(x[0], width),
            bound(height - y[0].max(y[1]), height),
            bound(x[1], width),
            bound(height - y[0].min(y[1]), height),
        ];
        let positions = [(x[0], y[0]), (x[0], y[1]), (x[1], y[0]), (x[1], y[1])];
        let planes = strip_triangles(y_up).map(|triangle| {
            values.map(|v| linear_coeff(triangle.map(|i| positions[i]), triangle.map(|i| v[i])))
        });
        if planes
            .iter()
            .flatten()
            .any(|&(a, dx, dy)| !a.is_finite() || !dx.is_finite() || !dy.is_finite())
        {
            return None;
        }
        Some(Self {
            x,
            y,
            height: frame[1] as usize,
            bounds,
            planes,
        })
    }

    pub(crate) fn bounds(&self) -> [usize; 4] {
        self.bounds
    }

    pub(crate) fn sample(&self, x: usize, y: usize) -> Option<[f32; N]> {
        let source_x = (x as f32 + 0.5 - self.x[0]) / (self.x[1] - self.x[0]);
        let source_y = (self.height as f32 - 0.5 - y as f32 - self.y[0]) / (self.y[1] - self.y[0]);
        if !(0. ..1.).contains(&source_x) || !(0. ..1.).contains(&source_y) {
            return None;
        }
        let triangle = usize::from(source_x + source_y > 1.);
        Some(
            self.planes[triangle].map(|(a, dx, dy)| {
                dy.mul_add((self.height - 1 - y) as f32, dx.mul_add(x as f32, a))
            }),
        )
    }
}

/// Affine clip-space position and high-precision UV before viewport setup.
#[derive(Clone, Copy)]
pub(crate) struct ClipVertex {
    pub(crate) position: [f32; 2],
    pub(crate) uv: [f32; 2],
}

#[derive(Clone, Copy)]
pub(crate) struct ClippedTriangle {
    pub(crate) coverage: [(i64, i64); 3],
    pub(crate) uv: [(f32, f32, f32); 2],
}

/// Clip before viewport/varying-plane setup, preserving the captured F32
/// intersection and fan order. Affine W=1 profiles supply finite clip vertices.
pub(crate) fn clip_triangle(
    vertices: [ClipVertex; 3],
    [width, height]: [usize; 2],
) -> Result<smallvec::SmallVec<[ClippedTriangle; 6]>, InkRasterError> {
    let scale = [width as f32 * 0.5, height as f32 * 0.5];
    let mut polygon: smallvec::SmallVec<[ClipVertex; 8]> =
        smallvec::SmallVec::from_slice(&vertices);
    for (axis, sign) in [(0, -1.), (0, 1.), (1, -1.), (1, 1.)] {
        if polygon.is_empty() {
            break;
        }
        let mut output: smallvec::SmallVec<[ClipVertex; 8]> = smallvec::SmallVec::new();
        for (i, &current) in polygon.iter().enumerate() {
            let previous = polygon[(i + polygon.len() - 1) % polygon.len()];
            let a = 1. - sign * previous.position[axis];
            let b = 1. - sign * current.position[axis];
            if (a >= 0.) != (b >= 0.) {
                let t = a / (a - b);
                let interpolate = |a: f32, b: f32| a + t * (b - a);
                let mut vertex = ClipVertex {
                    position: std::array::from_fn(|c| {
                        interpolate(previous.position[c], current.position[c])
                    }),
                    uv: std::array::from_fn(|c| interpolate(previous.uv[c], current.uv[c])),
                };
                vertex.position[axis] = sign;
                if output.len() == 8 {
                    return Err(InkRasterError::WorkLimit);
                }
                output.push(vertex);
            }
            if b >= 0. {
                if output.len() == 8 {
                    return Err(InkRasterError::WorkLimit);
                }
                output.push(current);
            }
        }
        polygon = output;
    }
    let mut result = smallvec::SmallVec::new();
    for i in 1..polygon.len().saturating_sub(1) {
        let mut triangle = [polygon[0], polygon[i], polygon[i + 1]];
        let mut position = triangle.map(|v| {
            (
                v.position[0].mul_add(scale[0], scale[0]),
                v.position[1].mul_add(scale[1], scale[1]),
            )
        });
        let [a, b, c] = position;
        let area = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
        if area == 0. {
            continue;
        }
        if area > 0. {
            triangle.swap(0, 1);
            position.swap(0, 1);
        }
        let uv = [0, 1].map(|axis| linear_coeff(position, triangle.map(|v| v.uv[axis])));
        if uv
            .iter()
            .any(|&(a, dx, dy)| !a.is_finite() || !dx.is_finite() || !dy.is_finite())
        {
            return Err(InkRasterError::InvalidStroke(
                "unrepresentable host varying plane",
            ));
        }
        let coverage = position.map(|(x, y)| {
            (
                (x * 256.).round_ties_even() as i64,
                (y * 256.).round_ties_even() as i64,
            )
        });
        result.push(ClippedTriangle { coverage, uv });
    }
    Ok(result)
}

pub(crate) fn triangle_contains(v: [(i64, i64); 3], (x, y): (i64, i64)) -> bool {
    let [a, b, c] = v;
    if (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0) == 0 {
        return false;
    }
    let mut positive = false;
    let mut negative = false;
    for i in 0..3 {
        let (xa, ya) = v[i];
        let (xb, yb) = v[(i + 1) % 3];
        let edge = (x - xa) * (yb - ya) - (y - ya) * (xb - xa);
        positive |= edge > 0;
        negative |= edge < 0;
    }
    !(positive && negative)
}
