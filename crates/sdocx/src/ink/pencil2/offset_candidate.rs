/// Computes a Pencil2 V14 host grain uniform from explicit captured inputs.
/// Matrices are column-major. Only x, y and w lanes are computed. A native
/// matrix anomaly occurs in the unused z lane and does not affect the uniform;
/// a separate full-matrix test records it. No document-to-matrix
/// or point selection or coordinate frame is inferred. This is pure computation.
/// Evidence covers finite moderate synthetic inputs under ARM64 native
/// emulation with host Mesa, including Drawing lifecycle captures. This does
/// not establish saved-page or device fidelity.
/// Conservatively returns `None` for any non-finite input, non-finite computed
/// intermediate or output, zero homogeneous w, or zero dimensions. This
/// validation is a helper policy; exceptional inputs have no native parity claim.
/// The calculator can return offsets outside the canvas paint API's +/-16384
/// pixel guard; such offsets are rejected by `paint_pencil2_with_grain_offset`.
pub fn pencil2_v14_host_grain_offset(
    b: [f32; 16],
    d: [f32; 16],
    point: [f32; 2],
    dimensions: [u32; 2],
) -> Option<[f32; 2]> {
    if b.iter().chain(&d).chain(&point).any(|v| !v.is_finite()) || dimensions.contains(&0) {
        return None;
    }
    let mut m = [0.0; 16];
    for j in 0..4 {
        for i in [0, 1, 3] {
            let mut a = b[4 + i] * d[4 * j + 1];
            a = b[i].mul_add(d[4 * j], a);
            a = b[8 + i].mul_add(d[4 * j + 2], a);
            a = b[12 + i].mul_add(d[4 * j + 3], a);
            if !a.is_finite() {
                return None;
            }
            m[4 * j + i] = a;
        }
    }
    let mut v = [0.0; 4];
    for i in [0, 1, 3] {
        let mut a = m[4 + i] * point[1];
        a = m[i].mul_add(point[0], a);
        a = m[8 + i].mul_add(0.0, a);
        a = m[12 + i].mul_add(1.0, a);
        if !a.is_finite() {
            return None;
        }
        v[i] = a;
    }
    if v[3] == 0.0 {
        return None;
    }
    let mut out = [0.0; 2];
    for i in 0..2 {
        out[i] = ((v[i] / v[3] + 1.0) * 0.5) * dimensions[i] as f32;
        if !out[i].is_finite() {
            return None;
        }
    }
    Some(out)
}

/// Experimental model of the observed start-point dirty/cache lifecycle.
/// The caller owns the start-point updates, native draw matrices and framebuffer
/// dimensions; this does not derive them from a document or model native queue
/// scheduling. A draw with no pending start point returns the previous offset,
/// even if the supplied matrices or dimensions have changed. Before the first
/// successful computation it returns `None`.
#[derive(Default)]
pub struct Pencil2V14HostGrainState {
    pending: Option<[f32; 2]>,
    cached: Option<[f32; 2]>,
}

impl Pencil2V14HostGrainState {
    pub fn set_start_point(&mut self, point: [f32; 2]) {
        self.pending = Some(point);
    }

    /// Computes once after a start-point update and otherwise returns the cache.
    /// A rejected computation leaves the start point pending for a later draw.
    pub fn draw(&mut self, b: [f32; 16], d: [f32; 16], dimensions: [u32; 2]) -> Option<[f32; 2]> {
        if let Some(point) = self.pending {
            let offset = pencil2_v14_host_grain_offset(b, d, point, dimensions)?;
            self.cached = Some(offset);
            self.pending = None;
        }
        self.cached
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_recomputation_preserves_cache_and_pending_point() {
        let mut identity = [0.; 16];
        for i in 0..4 {
            identity[5 * i] = 1.;
        }
        let mut state = Pencil2V14HostGrainState::default();
        state.set_start_point([1., 2.]);
        assert_eq!(
            state.draw(identity, identity, [192, 192]),
            Some([192., 288.])
        );
        state.set_start_point([2., 3.]);
        assert_eq!(state.draw(identity, identity, [0, 192]), None);
        assert_eq!(state.cached, Some([192., 288.]));
        assert_eq!(state.pending, Some([2., 3.]));
        assert_eq!(
            state.draw(identity, identity, [192, 192]),
            Some([288., 384.])
        );
    }
    #[test]
    fn captured_lifecycle_uniform_bits() {
        assert_eq!(
            pencil2_v14_host_grain_offset(
                [
                    0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0xb8000100,
                    0x0, 0xbf800000, 0xbf800000, 0x80000000, 0x3f800000
                ]
                .map(f32::from_bits),
                [
                    0x3f800000, 0x0, 0x0, 0x0, 0x0, 0x3f800000, 0x0, 0x0, 0x0, 0x0, 0x3f800000,
                    0x0, 0x40c00000, 0x41100000, 0x0, 0x3f800000
                ]
                .map(f32::from_bits),
                [0x0, 0x0].map(f32::from_bits),
                [192, 192]
            )
            .unwrap()
            .map(f32::to_bits),
            [0x40c00000, 0x41100000],
            "translation"
        );
        assert_eq!(
            pencil2_v14_host_grain_offset(
                [
                    0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0xb8000100,
                    0x0, 0xbf800000, 0xbf800000, 0x80000000, 0x3f800000
                ]
                .map(f32::from_bits),
                [
                    0x3f800000, 0x0, 0x0, 0x0, 0x0, 0x3f800000, 0x0, 0x0, 0x0, 0x0, 0x3f800000,
                    0x0, 0x41400000, 0x41400000, 0x0, 0x3f800000
                ]
                .map(f32::from_bits),
                [0x40a00000, 0x41100000].map(f32::from_bits),
                [192, 192]
            )
            .unwrap()
            .map(f32::to_bits),
            [0x4187ffff, 0x41a80000],
            "translation"
        );
        assert_eq!(
            pencil2_v14_host_grain_offset(
                [
                    0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0xb8000100,
                    0x0, 0xbf800000, 0xbf800000, 0x80000000, 0x3f800000
                ]
                .map(f32::from_bits),
                [
                    0x3f800000, 0x0, 0x0, 0x0, 0x0, 0x3f800000, 0x0, 0x0, 0x0, 0x0, 0x3f800000,
                    0x0, 0x41400000, 0x41400000, 0x0, 0x3f800000
                ]
                .map(f32::from_bits),
                [0x40a00000, 0x41100000].map(f32::from_bits),
                [192, 192]
            )
            .unwrap()
            .map(f32::to_bits),
            [0x4187ffff, 0x41a80000],
            "translation"
        );
        assert_eq!(
            pencil2_v14_host_grain_offset(
                [
                    0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0xb8000100,
                    0x0, 0xbf800000, 0xbf800000, 0x80000000, 0x3f800000
                ]
                .map(f32::from_bits),
                [
                    0x3f800000, 0x0, 0x0, 0x0, 0x0, 0x3f800000, 0x0, 0x0, 0x0, 0x0, 0x3f800000,
                    0x0, 0x42400000, 0x42100000, 0x0, 0x3f800000
                ]
                .map(f32::from_bits),
                [0xc0e80000, 0x40900000].map(f32::from_bits),
                [192, 192]
            )
            .unwrap()
            .map(f32::to_bits),
            [0x42230000, 0x42220000],
            "translation"
        );
        assert_eq!(
            pencil2_v14_host_grain_offset(
                [
                    0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0xb8000100,
                    0x0, 0xbf800000, 0xbf800000, 0x80000000, 0x3f800000
                ]
                .map(f32::from_bits),
                [
                    0x3f800000, 0x0, 0x0, 0x0, 0x0, 0x3f800000, 0x0, 0x0, 0x0, 0x0, 0x3f800000,
                    0x0, 0x40c00000, 0x41100000, 0x0, 0x3f800000
                ]
                .map(f32::from_bits),
                [0x0, 0x0].map(f32::from_bits),
                [192, 192]
            )
            .unwrap()
            .map(f32::to_bits),
            [0x40c00000, 0x41100000],
            "scale"
        );
        assert_eq!(
            pencil2_v14_host_grain_offset(
                [
                    0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0xb8000100,
                    0x0, 0xbf800000, 0xbf800000, 0x80000000, 0x3f800000
                ]
                .map(f32::from_bits),
                [
                    0x40000000, 0x0, 0x0, 0x0, 0x0, 0x3f000000, 0x0, 0x0, 0x0, 0x0, 0x3f800000,
                    0x0, 0x41400000, 0x41400000, 0x0, 0x3f800000
                ]
                .map(f32::from_bits),
                [0xc0a00000, 0xc0e00000].map(f32::from_bits),
                [192, 192]
            )
            .unwrap()
            .map(f32::to_bits),
            [0x3ffffff0, 0x41080002],
            "scale"
        );
        assert_eq!(
            pencil2_v14_host_grain_offset(
                [
                    0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0xb8000100,
                    0x0, 0xbf800000, 0xbf800000, 0x80000000, 0x3f800000
                ]
                .map(f32::from_bits),
                [
                    0x40000000, 0x0, 0x0, 0x0, 0x0, 0x3f000000, 0x0, 0x0, 0x0, 0x0, 0x3f800000,
                    0x0, 0x41400000, 0x41400000, 0x0, 0x3f800000
                ]
                .map(f32::from_bits),
                [0xc0a00000, 0xc0e00000].map(f32::from_bits),
                [192, 192]
            )
            .unwrap()
            .map(f32::to_bits),
            [0x3ffffff0, 0x41080002],
            "scale"
        );
        assert_eq!(
            pencil2_v14_host_grain_offset(
                [
                    0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0xb8000100,
                    0x0, 0xbf800000, 0xbf800000, 0x80000000, 0x3f800000
                ]
                .map(f32::from_bits),
                [
                    0x3f800000, 0x0, 0x0, 0x0, 0x0, 0x3f800000, 0x0, 0x0, 0x0, 0x0, 0x3f800000,
                    0x0, 0x42400000, 0x42100000, 0x0, 0x3f800000
                ]
                .map(f32::from_bits),
                [0xc0e80000, 0x40900000].map(f32::from_bits),
                [192, 192]
            )
            .unwrap()
            .map(f32::to_bits),
            [0x42230000, 0x42220000],
            "scale"
        );
        assert_eq!(
            pencil2_v14_host_grain_offset(
                [
                    0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0xb8000100,
                    0x0, 0xbf800000, 0xbf800000, 0x80000000, 0x3f800000
                ]
                .map(f32::from_bits),
                [
                    0x3f800000, 0x0, 0x0, 0x0, 0x0, 0x3f800000, 0x0, 0x0, 0x0, 0x0, 0x3f800000,
                    0x0, 0x40c00000, 0x41100000, 0x0, 0x3f800000
                ]
                .map(f32::from_bits),
                [0x0, 0x0].map(f32::from_bits),
                [192, 192]
            )
            .unwrap()
            .map(f32::to_bits),
            [0x40c00000, 0x41100000],
            "rotate"
        );
        assert_eq!(
            pencil2_v14_host_grain_offset(
                [
                    0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0xb8000100,
                    0x0, 0xbf800000, 0xbf800000, 0x80000000, 0x3f800000
                ]
                .map(f32::from_bits),
                [
                    0x0, 0x3f800000, 0x0, 0x0, 0xbf800000, 0x0, 0x0, 0x0, 0x0, 0x0, 0x3f800000,
                    0x0, 0x41400000, 0x41400000, 0x0, 0x3f800000
                ]
                .map(f32::from_bits),
                [0x40a00000, 0x41100000].map(f32::from_bits),
                [192, 192]
            )
            .unwrap()
            .map(f32::to_bits),
            [0x40400000, 0x4187ffff],
            "rotate"
        );
        assert_eq!(
            pencil2_v14_host_grain_offset(
                [
                    0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0xb8000100,
                    0x0, 0xbf800000, 0xbf800000, 0x80000000, 0x3f800000
                ]
                .map(f32::from_bits),
                [
                    0x0, 0x3f800000, 0x0, 0x0, 0xbf800000, 0x0, 0x0, 0x0, 0x0, 0x0, 0x3f800000,
                    0x0, 0x41400000, 0x41400000, 0x0, 0x3f800000
                ]
                .map(f32::from_bits),
                [0x40a00000, 0x41100000].map(f32::from_bits),
                [192, 192]
            )
            .unwrap()
            .map(f32::to_bits),
            [0x40400000, 0x4187ffff],
            "rotate"
        );
        assert_eq!(
            pencil2_v14_host_grain_offset(
                [
                    0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0xb8000100,
                    0x0, 0xbf800000, 0xbf800000, 0x80000000, 0x3f800000
                ]
                .map(f32::from_bits),
                [
                    0x3f800000, 0x0, 0x0, 0x0, 0x0, 0x3f800000, 0x0, 0x0, 0x0, 0x0, 0x3f800000,
                    0x0, 0x42400000, 0x42100000, 0x0, 0x3f800000
                ]
                .map(f32::from_bits),
                [0xc0e80000, 0x40900000].map(f32::from_bits),
                [192, 192]
            )
            .unwrap()
            .map(f32::to_bits),
            [0x42230000, 0x42220000],
            "rotate"
        );
        assert_eq!(
            pencil2_v14_host_grain_offset(
                [
                    0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0xb8000100,
                    0x0, 0xbf800000, 0xbf800000, 0x80000000, 0x3f800000
                ]
                .map(f32::from_bits),
                [
                    0x3f800000, 0x0, 0x0, 0x0, 0x0, 0x3f800000, 0x0, 0x0, 0x0, 0x0, 0x3f800000,
                    0x0, 0x40c00000, 0x41100000, 0x0, 0x3f800000
                ]
                .map(f32::from_bits),
                [0x0, 0x0].map(f32::from_bits),
                [192, 192]
            )
            .unwrap()
            .map(f32::to_bits),
            [0x40c00000, 0x41100000],
            "shear"
        );
        assert_eq!(
            pencil2_v14_host_grain_offset(
                [
                    0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0xb8000100,
                    0x0, 0xbf800000, 0xbf800000, 0x80000000, 0x3f800000
                ]
                .map(f32::from_bits),
                [
                    0x3f800000, 0x3e800000, 0x0, 0x0, 0x3f000000, 0x3f800000, 0x0, 0x0, 0x0, 0x0,
                    0x3f800000, 0x0, 0x41400000, 0x41400000, 0x0, 0x3f800000
                ]
                .map(f32::from_bits),
                [0x40500000, 0xc0900000].map(f32::from_bits),
                [192, 192]
            )
            .unwrap()
            .map(f32::to_bits),
            [0x41500002, 0x41050002],
            "shear"
        );
        assert_eq!(
            pencil2_v14_host_grain_offset(
                [
                    0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0xb8000100,
                    0x0, 0xbf800000, 0xbf800000, 0x80000000, 0x3f800000
                ]
                .map(f32::from_bits),
                [
                    0x3f800000, 0x3e800000, 0x0, 0x0, 0x3f000000, 0x3f800000, 0x0, 0x0, 0x0, 0x0,
                    0x3f800000, 0x0, 0x41400000, 0x41400000, 0x0, 0x3f800000
                ]
                .map(f32::from_bits),
                [0x40500000, 0xc0900000].map(f32::from_bits),
                [192, 192]
            )
            .unwrap()
            .map(f32::to_bits),
            [0x41500002, 0x41050002],
            "shear"
        );
        assert_eq!(
            pencil2_v14_host_grain_offset(
                [
                    0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0xb8000100,
                    0x0, 0xbf800000, 0xbf800000, 0x80000000, 0x3f800000
                ]
                .map(f32::from_bits),
                [
                    0x3f800000, 0x0, 0x0, 0x0, 0x0, 0x3f800000, 0x0, 0x0, 0x0, 0x0, 0x3f800000,
                    0x0, 0x42400000, 0x42100000, 0x0, 0x3f800000
                ]
                .map(f32::from_bits),
                [0xc0e80000, 0x40900000].map(f32::from_bits),
                [192, 192]
            )
            .unwrap()
            .map(f32::to_bits),
            [0x42230000, 0x42220000],
            "shear"
        );
        assert_eq!(
            pencil2_v14_host_grain_offset(
                [
                    0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0xb8000100,
                    0x0, 0xbf800000, 0xbf800000, 0x80000000, 0x3f800000
                ]
                .map(f32::from_bits),
                [
                    0x3f800000, 0x0, 0x0, 0x0, 0x0, 0x3f800000, 0x0, 0x0, 0x0, 0x0, 0x3f800000,
                    0x0, 0x40c00000, 0x41100000, 0x0, 0x3f800000
                ]
                .map(f32::from_bits),
                [0x0, 0x0].map(f32::from_bits),
                [192, 192]
            )
            .unwrap()
            .map(f32::to_bits),
            [0x40c00000, 0x41100000],
            "perspective"
        );
        assert_eq!(
            pencil2_v14_host_grain_offset(
                [
                    0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0xb8000100,
                    0x0, 0xbf800000, 0xbf800000, 0x80000000, 0x3f800000
                ]
                .map(f32::from_bits),
                [
                    0x3f800000, 0x3e4ccccd, 0x0, 0x3a83126f, 0x3e99999a, 0x3f800000, 0x0,
                    0xbb03126f, 0x0, 0x0, 0x3f800000, 0x0, 0x41400000, 0x41400000, 0x0, 0x3f800000
                ]
                .map(f32::from_bits),
                [0x40a00000, 0x41100000].map(f32::from_bits),
                [192, 192]
            )
            .unwrap()
            .map(f32::to_bits),
            [0x419facfe, 0x41b25171],
            "perspective"
        );
        assert_eq!(
            pencil2_v14_host_grain_offset(
                [
                    0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0xb8000100,
                    0x0, 0xbf800000, 0xbf800000, 0x80000000, 0x3f800000
                ]
                .map(f32::from_bits),
                [
                    0x3f800000, 0x3e4ccccd, 0x0, 0x3a83126f, 0x3e99999a, 0x3f800000, 0x0,
                    0xbb03126f, 0x0, 0x0, 0x3f800000, 0x0, 0x41400000, 0x41400000, 0x0, 0x3f800000
                ]
                .map(f32::from_bits),
                [0x40a00000, 0x41100000].map(f32::from_bits),
                [192, 192]
            )
            .unwrap()
            .map(f32::to_bits),
            [0x419facfe, 0x41b25171],
            "perspective"
        );
        assert_eq!(
            pencil2_v14_host_grain_offset(
                [
                    0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0x3c2aaaab, 0x0, 0x0, 0x0, 0x0, 0xb8000100,
                    0x0, 0xbf800000, 0xbf800000, 0x80000000, 0x3f800000
                ]
                .map(f32::from_bits),
                [
                    0x3f800000, 0x0, 0x0, 0x0, 0x0, 0x3f800000, 0x0, 0x0, 0x0, 0x0, 0x3f800000,
                    0x0, 0x42400000, 0x42100000, 0x0, 0x3f800000
                ]
                .map(f32::from_bits),
                [0xc0e80000, 0x40900000].map(f32::from_bits),
                [192, 192]
            )
            .unwrap()
            .map(f32::to_bits),
            [0x42230000, 0x42220000],
            "perspective"
        );
    }
    #[test]
    fn captured_off_target_start_points() {
        assert_eq!(
            pencil2_v14_host_grain_offset(
                [
                    0x3c2aaaab, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x3c2aaaab,
                    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0xb8000100, 0x00000000,
                    0xbf800000, 0xbf800000, 0x80000000, 0x3f800000
                ]
                .map(f32::from_bits),
                [
                    0x3f800000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x3f800000,
                    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x3f800000, 0x00000000,
                    0xc1c00000, 0x41000000, 0x00000000, 0x3f800000
                ]
                .map(f32::from_bits),
                [0xc2a00000, 0xc2700000].map(f32::from_bits),
                [192, 192]
            )
            .unwrap()
            .map(f32::to_bits),
            [0xc2d00002, 0xc2500002],
            "negative_origin"
        );
        assert_eq!(
            pencil2_v14_host_grain_offset(
                [
                    0x3c2aaaab, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x3c2aaaab,
                    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0xb8000100, 0x00000000,
                    0xbf800000, 0xbf800000, 0x80000000, 0x3f800000
                ]
                .map(f32::from_bits),
                [
                    0x3f000000, 0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x3fc00000,
                    0x00000000, 0x00000000, 0x00000000, 0x00000000, 0x3f800000, 0x00000000,
                    0x41800000, 0xc2000000, 0x00000000, 0x3f800000
                ]
                .map(f32::from_bits),
                [0x43390000, 0x43370000].map(f32::from_bits),
                [192, 192]
            )
            .unwrap()
            .map(f32::to_bits),
            [0x42d90000, 0x43727fff],
            "clipped_far_edge"
        );
    }
    #[test]
    fn rejects_invalid_and_overflow() {
        let mut id = [0.; 16];
        for i in 0..4 {
            id[5 * i] = 1.;
        }
        assert!(pencil2_v14_host_grain_offset(id, id, [0., 0.], [1, 1]).is_some());
        assert!(pencil2_v14_host_grain_offset(id, id, [f32::NAN, 0.], [1, 1]).is_none());
        assert!(pencil2_v14_host_grain_offset(id, id, [0., 0.], [0, 1]).is_none());
        let mut zero_w = id;
        zero_w[15] = 0.;
        assert!(pencil2_v14_host_grain_offset(id, zero_w, [0., 0.], [1, 1]).is_none());
        let mut big = id;
        big[0] = f32::MAX;
        assert!(pencil2_v14_host_grain_offset(big, big, [0., 0.], [1, 1]).is_none());
    }
    #[test]
    fn native_matrix_probe_decisive_case() {
        // First seeded case from the native ARM64 instruction block, including z.
        let b = [
            0x402a4789, 0x4069b4f7, 0x3f1e24c0, 0xbf6f1af3, 0xc03cd69d, 0xbfaa7213, 0x4046d724,
            0xc03a2f1f, 0x400e301f, 0xc0587994, 0x405acea7, 0x4010decf, 0x3ff99b4e, 0xbef9118b,
            0x3fa308f0, 0x3faa1f09,
        ]
        .map(f32::from_bits);
        let d = [
            0xc00dc618, 0xc05401c5, 0x3fb07c08, 0xc035178e, 0xbffa3025, 0x3fa35672, 0x3fa148db,
            0xc0463d16, 0xbfc7b8be, 0xc023e225, 0xbf059d5b, 0x3f0725ef, 0xc0436d9e, 0xc016c420,
            0x407246ff, 0x3fa1683e,
        ]
        .map(f32::from_bits);
        let expected = [
            0x3fb67fc2, 0xc0dee3c5, 0xc128cecf, 0x41310ee9, 0xc1434d9f, 0xc139788c, 0x4047aa46,
            0xc049ad97, 0x4051896a, 0xbf4786ae, 0xc1208174, 0x4106d488, 0x411b2205, 0xc1ab73e1,
            0x40bb8e7a, 0x419f99dc,
        ];
        let mut actual = [0.; 16];
        for j in 0..4 {
            for i in 0..4 {
                let a = b[4 + i] * d[4 * j + 1];
                let a = b[i].mul_add(if j == 3 && i == 2 { d[0] } else { d[4 * j] }, a);
                let a = b[8 + i].mul_add(d[4 * j + 2], a);
                actual[4 * j + i] = b[12 + i].mul_add(d[4 * j + 3], a);
            }
        }
        assert_eq!(actual.map(f32::to_bits), expected);
        let a = b[6] * d[13];
        let a = b[2].mul_add(d[12], a);
        let a = b[10].mul_add(d[14], a);
        let conventional = b[14].mul_add(d[15], a);
        assert_ne!(conventional.to_bits(), expected[14]);
    }
    #[test]
    fn unused_z_overflow_does_not_reject_xy() {
        let mut b = [0.; 16];
        let mut d = [0.; 16];
        for i in 0..4 {
            b[5 * i] = 1.;
            d[5 * i] = 1.;
        }
        b[2] = f32::MAX;
        d[0] = 2.;
        assert_eq!(
            pencil2_v14_host_grain_offset(b, d, [0., 0.], [1, 1]),
            Some([0.5, 0.5])
        );
    }
}
