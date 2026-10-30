//! Captured WaterColor RTV1 Mesa shader arithmetic; not a device profile.
pub(crate) use crate::ink::host_half::{half_add, half_mul, half_rne};
use crate::ink::host_rgba8::{premultiply, quantize as unorm, sample_unorm8_half_rtz};

pub(crate) use crate::ink::host_raster::ClipVertex as Vertex;

pub(crate) fn input_color(argb: u32) -> [f32; 4] {
    let lane = |shift: u32| ((argb >> shift) & 0xff_u32) as f32 / 255.0;
    [
        lane(16),
        lane(8),
        lane(0),
        lane(24).mul_add(f32::from_bits(0x3f66_6666), f32::from_bits(0x3dcc_cccd)),
    ]
}

pub(crate) fn source_shader_rgba(g: u8, color: [f32; 4], mask_r: u8) -> ([u8; 4], [u8; 4]) {
    // Program-4 non-eraser NIR formula. Texture conversion is source-proven
    // RTZ. Fragment half-ALU RNE is a model validated by these frames; V13
    // observes vertex-side rounding only, not fragment worker MXCSR.
    let sample = sample_unorm8_half_rtz(g);
    let threshold = half_rne(0.7);
    let max_alpha = half_rne(0.8);
    let input = color.map(half_rne);
    let (rgb, alpha) = if sample < threshold {
        let ratio = half_rne(sample / threshold);
        let alpha = half_mul(half_mul(ratio, max_alpha), input[3]);
        ([input[0], input[1], input[2]], alpha)
    } else {
        let mut delta = half_rne(sample + half_rne(-threshold));
        delta = half_mul(delta, half_rne(0.5));
        delta = half_mul(delta, sample_unorm8_half_rtz(mask_r));
        (
            [
                half_add(input[0], delta),
                half_add(input[1], delta),
                half_add(input[2], delta),
            ],
            half_mul(max_alpha, input[3]),
        )
    };
    let source = [unorm(rgb[0]), unorm(rgb[1]), unorm(rgb[2]), unorm(alpha)];
    let blended = premultiply(source);
    (source, blended)
}
pub(crate) fn vertices_with_projection(r: [f32; 6], matrix: [f32; 16]) -> [Vertex; 4] {
    let [x, y, lane2, lane3, radius, _] = r;
    let (x, y, lane2, lane3, radius) = (
        half_rne(x),
        half_rne(y),
        half_rne(lane2),
        half_rne(lane3),
        half_rne(radius),
    );
    let rect = [
        (-1.0f32, -1.0f32, 0.0f32, 1.0f32),
        (-1.0, 1.0, 0.0, 0.0),
        (1.0, -1.0, 1.0, 1.0),
        (1.0, 1.0, 1.0, 0.0),
    ];
    rect.map(|(rx, ry, u, v)| {
        let lx = half_mul(half_rne(rx), radius);
        let ly = half_mul(half_rne(ry), radius);
        let xx = half_mul(lane3, lx);
        let xy = half_mul(lane2, lx);
        let yx = half_mul(lane2, ly);
        let yy = half_mul(lane3, ly);
        let mx = half_add(half_add(xx, x), half_rne(-yx));
        let my = half_add(half_add(xy, y), yy);

        // Captured Mesa NIR source BLAKE3
        // 1fb6ed1d6f0d83fc69a07fd1fb1d803d8fb7a2055c5e79d19a9e71c7cf3f7c95
        // (debug stderr lines 1262-1357) shows f16 column products, translation
        // add, then other-axis add; `gl_Position` widens only afterward.
        let clip_x = half_add(
            half_add(half_mul(half_rne(matrix[0]), mx), half_rne(matrix[12])),
            half_mul(half_rne(matrix[4]), my),
        );
        let clip_y = half_add(
            half_add(half_mul(half_rne(matrix[1]), mx), half_rne(matrix[13])),
            half_mul(half_rne(matrix[5]), my),
        );
        Vertex {
            position: [clip_x, clip_y],
            uv: [u, v],
        }
    })
}
