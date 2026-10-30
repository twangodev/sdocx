//! Test-only triangle varying planes for the captured Mesa llvmpipe host profile.
//! Pencil2 device-profile tests compare the resulting frames with native receipts.

/// Preserve the captured setup order when a strip's output Y axis is reflected.
/// Equivalent triangle permutations can choose a different nearest texel in F32.
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
pub(crate) struct AxisRect<const N: usize> {
    x: [f32; 2],
    y: [f32; 2],
    height: usize,
    bounds: [usize; 4],
    planes: [[(f32, f32, f32); N]; 2],
}

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
