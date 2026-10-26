//! Conditional host-Mesa BrushPen V1 paint model over captured native VBO records.
//! Records are paint inputs from the native Drawing path; Rust does not generate them here.
//! The exact frame is capture-specific and not Android-device pixel parity.

#[derive(Clone, Copy)]
struct Rgba8<'a> {
    pub width: usize,
    pub height: usize,
    /// RGBA8 bytes in upload order; row zero is the GL texture's first row.
    pub bytes: &'a [u8],
}

#[derive(Clone, Copy)]
struct Context {
    pub width: usize,
    pub height: usize,
    /// Exact column-major uniform submitted to the native V1 program.
    pub projection: [f32; 16],
    pub rgb: [f32; 3],
    pub grain_period: [f32; 2],
    pub grain_offset: [f32; 2],
    pub num_tiles: f32,
}

#[derive(Debug)]
enum Error {
    InvalidTexture,
    InvalidContext,
    NonAxisAlignedProjection,
    InvalidRecord,
}

#[derive(Clone, Copy)]
struct ScreenQuad {
    left: f32,
    right: f32,
    bottom: f32,
    top: f32,
    s_dx: f32,
    t_dy: f32,
}

/// Paint captured 11-float RTV1 records into a transparent bottom-left RGBA8 target.
/// This reproduces one specific host-Mesa sampler profile, not portable GLES behavior.
fn paint(
    records: &[[f32; 11]],
    atlas: Rgba8<'_>,
    grain: Rgba8<'_>,
    context: Context,
    destination: &mut [u8],
) -> Result<(), Error> {
    validate_texture(atlas)?;
    validate_texture(grain)?;
    let byte_len = context
        .width
        .checked_mul(context.height)
        .and_then(|n| n.checked_mul(4));
    if context.width == 0
        || context.height == 0
        || byte_len != Some(destination.len())
        || context.projection.iter().any(|v| !v.is_finite())
        || context.rgb.iter().any(|v| !v.is_finite())
        || context
            .grain_period
            .iter()
            .any(|v| !v.is_finite() || *v <= 0.0)
        || context.grain_offset.iter().any(|v| !v.is_finite())
        || !context.num_tiles.is_finite()
        || context.num_tiles <= 0.0
    {
        return Err(Error::InvalidContext);
    }
    // The retained V7 program uses an axis-aligned projection. General affine
    // interpolation can be added from a new capture; do not silently approximate it.
    if context.projection[1] != 0.0 || context.projection[4] != 0.0 {
        return Err(Error::NonAxisAlignedProjection);
    }

    for record in records {
        if record.iter().any(|v| !v.is_finite()) || record[2] <= 0.0 {
            return Err(Error::InvalidRecord);
        }
        let quad = screen_quad(record, context);
        let tile = record[3].floor();
        if tile < 0.0 || tile >= context.num_tiles * context.num_tiles {
            return Err(Error::InvalidRecord);
        }
        let opacity = record[3].fract() * 1000.0 / 255.0;
        let tile_x = tile % context.num_tiles;
        let tile_y = (tile / context.num_tiles).floor();
        let x0 = (quad.left.floor() as isize)
            .max(0)
            .min(context.width as isize) as usize;
        let x1 = (quad.right.ceil() as isize)
            .max(0)
            .min(context.width as isize) as usize;
        let y0 = (quad.bottom.floor() as isize)
            .max(0)
            .min(context.height as isize) as usize;
        let y1 = (quad.top.ceil() as isize)
            .max(0)
            .min(context.height as isize) as usize;

        for py in y0..y1 {
            for px in x0..x1 {
                let fx = px as f32 + 0.5;
                let fy = py as f32 + 0.5;
                // This is the smooth varying produced by the source vertex shader:
                // aRectPoint.zw is (0,1),(0,0),(1,1),(1,0) on the four corners.
                let [s, t] = interpolate_frag_coord(fx, fy, quad);
                if !(0.0..=1.0).contains(&s) || !(0.0..=1.0).contains(&t) {
                    continue;
                }
                let atlas_u = s / context.num_tiles + tile_x / context.num_tiles;
                let atlas_v = (1.0 - t) / context.num_tiles + tile_y / context.num_tiles;
                let mask = sample_nearest(atlas, atlas_u, atlas_v, false);
                if mask == 0.0 {
                    continue;
                }

                let grain_u = (fx - context.grain_offset[0]) / context.grain_period[0];
                let grain_v = (fy - context.grain_offset[1]) / context.grain_period[1];
                let grain_alpha = sample_nearest(grain, grain_u, grain_v, true);
                if grain_alpha <= 0.0 {
                    continue;
                }

                let mag = ((s - 0.5) * record[4] + (0.5 - t) * record[5]) / 0.5;
                let mut grad_min = (record[6] * record[4] + record[7] * record[5]).abs() + 0.5;
                if grad_min > 1.0 {
                    grad_min = 1.0;
                }
                let grad_alpha = (grad_min - 1.0) / 2.0 * mag + (1.0 + grad_min) / 2.0;
                let mut alpha = mask * opacity * grad_alpha;
                if alpha <= 0.0 {
                    continue;
                }
                // Native shader: alpha = alpha*alpha + alpha*grain.r*(1-alpha).
                alpha = alpha * alpha + alpha * grain_alpha * (1.0 - alpha);
                if alpha <= 0.0 {
                    continue;
                }

                let dist = radial_distance(s, t);
                let fwidth = quad_fwidth(px, py, quad);
                if fwidth <= 0.0 {
                    continue;
                }
                let smooth = 1.0 - clamp01((dist - (0.5 - fwidth)) / fwidth);
                if smooth <= 0.0 {
                    continue;
                }
                blend_rgba8(
                    destination,
                    (py * context.width + px) * 4,
                    context.rgb,
                    alpha * smooth,
                );
            }
        }
    }
    Ok(())
}

fn screen_quad(record: &[f32; 11], c: Context) -> ScreenQuad {
    // Execute the source vertex expression and projection, then use the GL
    // viewport's scale/translate form. This preserves f32 operation order instead
    // of reducing the varying to ideal world-center/radius ratios.
    let [x, y, radius] = [record[0], record[1], record[2]];
    let project = |rx: f32, ry: f32| {
        let world_x = x + rx * radius;
        let world_y = y + ry * radius;
        let clip_x = c.projection[0] * world_x + c.projection[4] * world_y + c.projection[12];
        let clip_y = c.projection[1] * world_x + c.projection[5] * world_y + c.projection[13];
        let half_width = c.width as f32 * 0.5;
        let half_height = c.height as f32 * 0.5;
        [
            clip_x * half_width + half_width,
            clip_y * half_height + half_height,
        ]
    };
    let bl = project(-1.0, -1.0);
    let tr = project(1.0, 1.0);
    let left = bl[0].min(tr[0]);
    let right = bl[0].max(tr[0]);
    let bottom = bl[1].min(tr[1]);
    let top = bl[1].max(tr[1]);
    // Source varying values at opposing vertices are s=0/1 and t=1/0.
    // Solve its affine plane in f32 once and evaluate that plane per fragment.
    ScreenQuad {
        left,
        right,
        bottom,
        top,
        s_dx: 1.0 / (right - left),
        t_dy: -1.0 / (top - bottom),
    }
}

fn interpolate_frag_coord(px: f32, py: f32, q: ScreenQuad) -> [f32; 2] {
    // Values at the quad's vertices are exactly s=(0,1) left/right and t=(1,0)
    // bottom/top. Evaluate the f32 affine interpolation plane at pixel centers.
    [0.0 + (px - q.left) * q.s_dx, 1.0 + (py - q.bottom) * q.t_dy]
}

fn radial_distance(s: f32, t: f32) -> f32 {
    ((s - 0.5) * (s - 0.5) + (0.5 - t) * (0.5 - t)).sqrt()
}

fn quad_fwidth(px: usize, py: usize, q: ScreenQuad) -> f32 {
    // Mesa's captured 8-lane lowering has two 2x2 quads. For each quad, ddx
    // pairs lanes (0,1)/(2,3), while ddy pairs (0,2)/(1,3). Pixel grouping is
    // assumed aligned to framebuffer origin (x&!1,y&!1), as in this host path.
    // Neighbor values deliberately extrapolate the smooth varying beyond the stamp;
    // this represents derivative helper invocations on discarded/outside lanes.
    let qx = (px & !1) as f32;
    let qy = (py & !1) as f32;
    let d_x0 = radial_distance(
        interpolate_frag_coord(qx + 0.5, py as f32 + 0.5, q)[0],
        interpolate_frag_coord(qx + 0.5, py as f32 + 0.5, q)[1],
    );
    let d_x1 = radial_distance(
        interpolate_frag_coord(qx + 1.5, py as f32 + 0.5, q)[0],
        interpolate_frag_coord(qx + 1.5, py as f32 + 0.5, q)[1],
    );
    let d_y0 = radial_distance(
        interpolate_frag_coord(px as f32 + 0.5, qy + 0.5, q)[0],
        interpolate_frag_coord(px as f32 + 0.5, qy + 0.5, q)[1],
    );
    let d_y1 = radial_distance(
        interpolate_frag_coord(px as f32 + 0.5, qy + 1.5, q)[0],
        interpolate_frag_coord(px as f32 + 0.5, qy + 1.5, q)[1],
    );
    (d_x1 - d_x0).abs() + (d_y1 - d_y0).abs()
}

fn sample_nearest(tex: Rgba8<'_>, mut u: f32, mut v: f32, repeat: bool) -> f32 {
    if repeat {
        u -= u.floor();
        v -= v.floor();
    } else {
        u = clamp01(u);
        v = clamp01(v);
    }
    let x = ((u * tex.width as f32).floor() as usize).min(tex.width - 1);
    let y = ((v * tex.height as f32).floor() as usize).min(tex.height - 1);
    // The active host-Mesa sampler narrows normalized R8 to binary16 with RTZ.
    // Every nonzero R8/255 is normal in binary16, so clearing the low 13
    // mantissa bits gives that value widened back to f32 without another helper.
    let normalized = tex.bytes[(y * tex.width + x) * 4] as f32 / 255.0;
    f32::from_bits(normalized.to_bits() & !0x1fff)
}

fn validate_texture(tex: Rgba8<'_>) -> Result<(), Error> {
    if tex.width == 0
        || tex.height == 0
        || tex
            .width
            .checked_mul(tex.height)
            .and_then(|n| n.checked_mul(4))
            != Some(tex.bytes.len())
    {
        return Err(Error::InvalidTexture);
    }
    Ok(())
}

fn blend_rgba8(dst: &mut [u8], offset: usize, rgb: [f32; 3], alpha: f32) {
    // Mesa 25.0.7 llvmpipe's RGBA8 path converts fragment RGBA to an
    // 8-bit normalized blend row before applying these captured factors. The
    // retained V7 frame labels the target RGBA8; if its internal format differs,
    // this host-source model is not applicable.
    let src = [
        unorm8(rgb[0]),
        unorm8(rgb[1]),
        unorm8(rgb[2]),
        unorm8(alpha),
    ];
    let inv_alpha = 255 - src[3];
    for i in 0..3 {
        let src_term = mul_unorm8(src[i], src[3]);
        let dst_term = mul_unorm8(dst[offset + i], inv_alpha);
        dst[offset + i] = src_term.saturating_add(dst_term);
    }
    // Separate alpha factors are ONE and ONE_MINUS_SRC_ALPHA.
    let dst_term = mul_unorm8(dst[offset + 3], inv_alpha);
    dst[offset + 3] = src[3].saturating_add(dst_term);
}

fn unorm8(x: f32) -> u8 {
    (clamp01(x) * 255.0).round_ties_even().clamp(0.0, 255.0) as u8
}

fn mul_unorm8(a: u8, b: u8) -> u8 {
    // Exact Mesa lp_build_mul_norm rounding: (ab + (ab >> 8) + 128) >> 8.
    let ab = a as u16 * b as u16;
    ((ab + (ab >> 8) + 128) >> 8).min(255) as u8
}
fn clamp01(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::read::ZlibDecoder;
    use serde_json::Value;
    use sha2::{Digest, Sha256};
    use std::io::Read;

    const FIXTURE_JSON: &[u8] = include_bytes!(
        "../../../../../conformance/brushpen-native-paint-v1/brushpen-native-paint.json"
    );
    const SOURCE_R8_ZLIB: &[u8] = include_bytes!(
        "../../../../../conformance/brushpen-native-paint-v1/intrinsic-mask-grain-r8.zlib"
    );
    const SPARSE_RGBA: &[u8] = include_bytes!(
        "../../../../../conformance/brushpen-native-paint-v1/native-sparse-readback-u16xy-rgba8.bin"
    );
    const NATIVE_VBO: &[u8] =
        include_bytes!("../../../../../conformance/brushpen-native-paint-v1/native-vbo-26x44.bin");
    const PROFILE: &[u8] = include_bytes!(
        "../../../../../conformance/brushpen-native-paint-v1/paint-conditional-profile.json"
    );

    const FIXTURE_SHA256: &str = "202ca56017ae462d387b102c0ef360379178286d4809d4b482afd41926dbb952";
    const SOURCE_R8_ZLIB_SHA256: &str =
        "7d10c1cb7ac6ab99a11eb72b91e565cf7e313d7c247da09700445151fb3a70f2";
    const MASK_R8_SHA256: &str = "8f1c09f0642bb63f9c6cfaa0bec1dcabf428aed6387f995fb64c7972bf064b20";
    const GRAIN_R8_SHA256: &str =
        "d6e0743f9343efac472d3d00ba44f4138572620b376fe6fd74d9b5ebf1aad2aa";
    const NATIVE_VBO_SHA256: &str =
        "57405d81981cd9deb40190fc185572dc9d5885ff5a92cf885e1ef53d652cee92";
    const SPARSE_SHA256: &str = "7139a59eea569950d913b1a395e6d3c6e470a9156cd4226bd57a35a5c2129fb7";
    const TARGET_RGBA_SHA256: &str =
        "f860fde550dbdaeb5968cc88fa928bc1d84a1a8e33d7d4cdf2dcbf0525396f54";
    const MASK_ATLAS_RGBA_SHA256: &str =
        "614dbc4cc7c4a784aa6581df2bd6d6acecfa33d5a7b5a239f868128213ec51b3";
    const GRAIN_RGBA_SHA256: &str =
        "ec10a82c17d4912c8d0611b0c87664c7f26662911ed284bbad017d6e3f444beb";

    fn sha256(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    fn rgba_red_upload(r8: &[u8]) -> Vec<u8> {
        let mut rgba = Vec::with_capacity(r8.len() * 4);
        for &red in r8 {
            // Native BrushPen setTextures widens each R8 byte to a uint32 in memory.
            rgba.extend_from_slice(&[red, 0, 0, 0]);
        }
        rgba
    }

    fn uniform<const N: usize>(material: &Value, key: &str) -> [f32; N] {
        let values = material["uniforms_actual_task2_gl_imports"][key]
            .as_array()
            .expect("captured uniform array");
        assert_eq!(values.len(), N, "uniform {key} length");
        std::array::from_fn(|i| values[i].as_f64().expect("numeric uniform") as f32)
    }

    fn read_native_records(bytes: &[u8]) -> Vec<[f32; 11]> {
        assert_eq!(bytes.len(), 26 * 44);
        bytes
            .as_chunks::<44>()
            .0
            .iter()
            .map(|record| {
                std::array::from_fn(|lane| {
                    let offset = lane * 4;
                    f32::from_le_bytes(record[offset..offset + 4].try_into().unwrap())
                })
            })
            .collect()
    }

    fn source_textures() -> (Vec<u8>, Vec<u8>) {
        let mut decoder = ZlibDecoder::new(SOURCE_R8_ZLIB);
        let mut decoded = Vec::with_capacity(89_316);
        decoder
            .read_to_end(&mut decoded)
            .expect("embedded zlib source assets");
        assert_eq!(decoded.len(), 89_316);
        let mask = &decoded[..16_128];
        let grain = &decoded[16_128..];
        assert_eq!(sha256(mask), MASK_R8_SHA256);
        assert_eq!(sha256(grain), GRAIN_R8_SHA256);

        let mut atlas_r8 = vec![0_u8; 144 * 144];
        for tile_index in 0..7 {
            let tile = &mask[tile_index * 48 * 48..(tile_index + 1) * 48 * 48];
            for y in 0..48 {
                for x in 0..48 {
                    let dx = (tile_index % 3) * 48 + x;
                    let dy = (tile_index / 3) * 48 + y;
                    atlas_r8[dy * 144 + dx] = tile[y * 48 + x];
                }
            }
        }
        let atlas_rgba = rgba_red_upload(&atlas_r8);
        let grain_rgba = rgba_red_upload(grain);
        assert_eq!(sha256(&atlas_rgba), MASK_ATLAS_RGBA_SHA256);
        assert_eq!(sha256(&grain_rgba), GRAIN_RGBA_SHA256);
        (atlas_rgba, grain_rgba)
    }

    fn native_full_frame(width: usize, height: usize) -> Vec<u8> {
        assert_eq!(sha256(SPARSE_RGBA), SPARSE_SHA256);
        assert_eq!(SPARSE_RGBA.len(), 10 * 8);
        let mut rgba = vec![0_u8; width * height * 4];
        let mut seen = std::collections::BTreeSet::new();
        for item in SPARSE_RGBA.as_chunks::<8>().0 {
            let x = u16::from_le_bytes([item[0], item[1]]) as usize;
            let y = u16::from_le_bytes([item[2], item[3]]) as usize;
            assert!(x < width && y < height);
            assert!(seen.insert((x, y)), "duplicate sparse pixel");
            let offset = (y * width + x) * 4;
            rgba[offset..offset + 4].copy_from_slice(&item[4..8]);
        }
        assert_eq!(seen.len(), 10);
        rgba
    }

    #[test]
    fn captured_native_vbo_paints_exact_v7_frame_and_checks_v17_observation() {
        assert_eq!(sha256(FIXTURE_JSON), FIXTURE_SHA256);
        assert_eq!(sha256(SOURCE_R8_ZLIB), SOURCE_R8_ZLIB_SHA256);
        assert_eq!(sha256(NATIVE_VBO), NATIVE_VBO_SHA256);
        assert_eq!(
            sha256(PROFILE),
            "991ce25ca92f9a492a4fb64c90687f033c36aa1f949be483b7d127a87f1ffe32"
        );
        let fixture: Value =
            serde_json::from_slice(FIXTURE_JSON).expect("native paint fixture JSON");
        let profile: Value = serde_json::from_slice(PROFILE).expect("conditional profile receipt");
        let draw = &fixture["draw"];
        let geometry = &fixture["geometry"];
        let material = &fixture["material"];
        let target = &draw["target"];

        assert_eq!(fixture["schema"], "brushpen-native-v7-paint-fixture-v1");
        assert_eq!(
            fixture["pins"]["v7_capture"]["sha256"],
            "4249e41e29ed810fd0ab28ea934640415a55a0a8eb57faec5b57ebd05fef96ec"
        );
        assert_eq!(
            fixture["pins"]["brushpen_dso"]["sha256"],
            "acf59300dbdf53942680761a6cfe57e5164e0e6c45076d30fc56b9efa04ea0ce"
        );
        assert_eq!(geometry["record_count"], 26);
        assert_eq!(geometry["stride_bytes"], 44);
        assert_eq!(geometry["sha256"], NATIVE_VBO_SHA256);
        assert_eq!(geometry["source_join"]["same_bytes"], true);
        assert_eq!(geometry["source_join"]["sha256"], NATIVE_VBO_SHA256);
        assert_eq!(
            fixture["pins"]["same_drawing_source_capture"]["sha256"],
            "fb14597bdccbe2b01583cded69d155e912358ccf00ede0f626b78fc26a4bb86e"
        );
        assert_eq!(
            fixture["pins"]["native_texture_init_capture_separate_run"]["sha256"],
            "827aee3f567fecdce3e04b4a29217dc1f5d140e130d11ef7720734ef4e7e992c"
        );

        assert_eq!(draw["phase"], "native PenGLRenderMsg task2");
        assert_eq!(
            draw["draw_args_glDrawArraysInstanced"],
            serde_json::json!([5, 0, 4, 26])
        );
        assert_eq!(draw["program_id"], 7);
        assert_eq!(draw["active_program_matches_adapter_program"], false);
        assert_eq!(target["dimensions"], serde_json::json!([321, 228]));
        assert_eq!(target["format"], "RGBA8");
        assert_eq!(target["draw_fbo"], 2);
        assert_eq!(target["read_fbo"], 2);
        assert_eq!(target["pre_draw_all_zero"], true);
        assert_eq!(
            target["pre_draw_sha256"],
            "f0cae1c3cf0c26f5eb5d3a6a1986263254718bb7ddf89b32d58edc0e4466fb89"
        );
        assert_eq!(target["post_draw_full_bytes"], 321 * 228 * 4);
        assert_eq!(target["post_draw_full_sha256"], TARGET_RGBA_SHA256);
        assert_eq!(
            material["shader_source"]["vertex_sha256"],
            "44072299681d558389d5a7e94e2c70929b2cf1399a493df777302c30b609fc3e"
        );
        assert_eq!(
            material["shader_source"]["fragment_sha256"],
            "4c45b50596922acd7b14e5dd56df2301b8b652e25e89bf466172558e929d44aa"
        );
        assert_eq!(material["texture_bindings_actual_task2"][0]["unit"], 0);
        assert_eq!(
            material["texture_bindings_actual_task2"][0]["texture_2d_id"],
            5
        );
        assert_eq!(
            material["texture_bindings_actual_task2"][0]["sampler_id"],
            4
        );
        assert_eq!(
            material["texture_bindings_actual_task2"][0]["effective_filter_wrap"],
            serde_json::json!({"min_filter":9728,"mag_filter":9728,"wrap_s":33071,"wrap_t":33071})
        );
        assert_eq!(material["texture_bindings_actual_task2"][1]["unit"], 1);
        assert_eq!(
            material["texture_bindings_actual_task2"][1]["texture_2d_id"],
            6
        );
        assert_eq!(
            material["texture_bindings_actual_task2"][1]["sampler_id"],
            5
        );
        assert_eq!(
            material["texture_bindings_actual_task2"][1]["effective_filter_wrap"],
            serde_json::json!({"min_filter":9728,"mag_filter":9728,"wrap_s":10497,"wrap_t":10497})
        );
        assert_eq!(
            material["source_upload_evidence"]["native_factory_roles"][1]["payload_sha256"],
            GRAIN_RGBA_SHA256
        );

        // V17 adds an independent same-context observation. Keep V7's fixture
        // and cross-capture history intact; this checks the additional direct joins.
        let v17 = &material["same_context_v17_observation"];
        assert_eq!(v17["capture_sha256"], "50d8a712daae286521c303f456e68719a404a1c28512f517e0153be495b05e6a");
        assert_eq!(v17["mesa_egl_context"], "0xf787c70");
        assert_eq!(v17["phase"], "native PenGLRenderMsg task2");
        assert_eq!(v17["draw_args_glDrawArraysInstanced"], serde_json::json!([5, 0, 4, 26]));
        assert_eq!(v17["gl_errors_before_queries"], serde_json::json!([]));
        assert_eq!(v17["gl_errors_after_queries"], serde_json::json!([]));
        assert_eq!(v17["native_instance_upload"]["bytes"], 26 * 44);
        assert_eq!(v17["native_instance_upload"]["sha256"], NATIVE_VBO_SHA256);
        assert_eq!(v17["native_instance_upload"]["matches_retained_26x44_vbo"], true);
        assert_eq!(v17["active_program"]["id"], 7);
        assert_eq!(v17["active_program"]["adapter_expected_id"], 3);
        assert_eq!(v17["active_program"]["matches_adapter_id"], false);
        assert_eq!(v17["active_program"]["link_status"], 1);
        assert_eq!(v17["active_program"]["attached_compiled_shader_count"], 2);
        assert_eq!(v17["active_program"]["source_hashes_match_pinned_fixture"], true);
        assert_eq!(v17["active_program"]["vertex_sha256"], material["shader_source"]["vertex_sha256"]);
        assert_eq!(v17["active_program"]["fragment_sha256"], material["shader_source"]["fragment_sha256"]);
        assert_eq!(v17["target"]["draw_fbo"], 2);
        assert_eq!(v17["target"]["read_fbo"], 2);
        assert_eq!(v17["target"]["attachment_texture"], 4);
        assert_eq!(v17["target"]["storage_internal_format"], 32856);
        assert_eq!(v17["target"]["storage_internal_format_name"], "GL_RGBA8");
        assert_eq!(v17["target"]["post_draw_format"], "RGBA8");
        assert_eq!(v17["target"]["post_draw_sha256"], TARGET_RGBA_SHA256);
        assert_eq!(v17["blend_state_at_draw"], serde_json::json!({"enabled":true,"equation_rgb":32774,"equation_alpha":32774,"src_rgb":770,"dst_rgb":771,"src_alpha":1,"dst_alpha":771,"color_write_mask":[1,1,1,1]}));
        assert_eq!(v17["sampled_textures"].as_array().unwrap().len(), 2);
        let pattern = &v17["sampled_textures"][0];
        let grain = &v17["sampled_textures"][1];
        assert_eq!(pattern["sampler_uniform"], "sampler0");
        assert_eq!(pattern["unit"], 0);
        assert_eq!(pattern["factory_descriptor_index"], 3);
        assert_eq!(pattern["factory_return_verified"], true);
        assert_eq!(pattern["mesa_texture_name"], 5);
        assert_eq!(pattern["storage_internal_format"], 32856);
        assert_eq!(pattern["upload_format"], 6408);
        assert_eq!(pattern["upload_type"], 5121);
        assert_eq!(pattern["payload_bytes"], 144 * 144 * 4);
        assert_eq!(pattern["initialized_tiles"], 7);
        assert_eq!(pattern["uninitialized_tiles"], serde_json::json!([7, 8]));
        assert_eq!(pattern["forwarded_host_owned_copy_verified"], true);
        assert_eq!(pattern["guest_payload_sha256"], "10363ec672243f7a0f09c0ce1aed7a69e8f4296153ec47691a469f513d344cf2");
        assert_eq!(pattern["guest_payload_sha256"], pattern["forwarded_host_copy_sha256"]);
        assert_eq!(pattern["guest_payload_sha256"], material["source_upload_evidence"]["native_factory_roles"][0]["payload_sha256_with_host_allocator_sentinels"]);
        assert_eq!(grain["sampler_uniform"], "sampler1");
        assert_eq!(grain["unit"], 1);
        assert_eq!(grain["factory_descriptor_index"], 4);
        assert_eq!(grain["factory_return_verified"], true);
        assert_eq!(grain["mesa_texture_name"], 6);
        assert_eq!(grain["storage_internal_format"], 32856);
        assert_eq!(grain["upload_format"], 6408);
        assert_eq!(grain["upload_type"], 5121);
        assert_eq!(grain["payload_bytes"], 321 * 228 * 4);
        assert_eq!(grain["forwarded_host_owned_copy_verified"], true);
        assert_eq!(grain["guest_payload_sha256"], grain["forwarded_host_copy_sha256"]);
        assert_eq!(grain["guest_payload_sha256"], GRAIN_RGBA_SHA256);
        assert_eq!(v17["sampled_textures"][0]["mesa_texture_name"], material["texture_bindings_actual_task2"][0]["texture_2d_id"]);
        assert_eq!(v17["sampled_textures"][1]["mesa_texture_name"], material["texture_bindings_actual_task2"][1]["texture_2d_id"]);

        // Paint input is the native Drawing-produced 26x44 VBO. This test does not run
        // Rust BrushPen geometry preparation or claim sampler geometry parity.
        let records = read_native_records(NATIVE_VBO);
        assert_eq!(records.len(), 26);
        assert!(records.iter().all(|r| (0.0..7.0).contains(&r[3].floor())));
        let (atlas_rgba, grain_rgba) = source_textures();
        let context = Context {
            width: 321,
            height: 228,
            projection: uniform::<16>(material, "ProjectionMatrix"),
            rgb: uniform::<4>(material, "inputColor")[..3]
                .try_into()
                .unwrap(),
            grain_period: uniform::<2>(material, "grain"),
            grain_offset: uniform::<2>(material, "grainOffset"),
            num_tiles: uniform::<1>(material, "numTiles")[0],
        };
        let atlas = Rgba8 {
            width: 144,
            height: 144,
            bytes: &atlas_rgba,
        };
        let grain = Rgba8 {
            width: 321,
            height: 228,
            bytes: &grain_rgba,
        };
        let mut candidate = vec![0_u8; 321 * 228 * 4];
        assert_eq!(
            sha256(&candidate),
            "f0cae1c3cf0c26f5eb5d3a6a1986263254718bb7ddf89b32d58edc0e4466fb89"
        );
        paint(&records, atlas, grain, context, &mut candidate).expect("captured profile paint");

        let native = native_full_frame(321, 228);
        assert_eq!(
            sha256(&native),
            TARGET_RGBA_SHA256,
            "sparse native bytes must reconstruct the retained native full-frame hash"
        );
        assert!(
            candidate == native,
            "all RGBA bytes must match the retained native frame"
        );
        assert_eq!(sha256(&candidate), TARGET_RGBA_SHA256);

        // The false V7 flags are historical. V17 independently confirms these
        // conditions for its same-context captured draw.
        assert_eq!(profile["profile_kind"], "conditional-captured-host-mesa-v1");
        assert_eq!(
            profile["mesa_blend_conversion"]["renderer"],
            "llvmpipe (LLVM 19.1.7, 256 bits)"
        );
        assert_eq!(
            profile["mesa_blend_conversion"]["version"],
            "OpenGL ES 3.2 Mesa 25.0.7-2+deb13u1"
        );
        assert_eq!(profile["target_internal_format_observed_in_v7"], false);
        assert_eq!(profile["blend_state_observed_in_v7"], false);
        assert_eq!(profile["target_internal_format_observed_in_v17"], true);
        assert_eq!(profile["blend_state_observed_in_v17"], true);
        assert_eq!(profile["same_context_uploads_observed_in_v17"], true);
        assert_eq!(profile["same_context_v17_observation"]["capture_sha256"], "50d8a712daae286521c303f456e68719a404a1c28512f517e0153be495b05e6a");
        assert_eq!(profile["same_context_v17_observation"]["mesa_egl_context"], "0xf787c70");
        assert_eq!(profile["same_context_v17_observation"]["active_program_id"], 7);
        assert_eq!(profile["same_context_v17_observation"]["adapter_expected_program_id"], 3);
        assert_eq!(profile["same_context_v17_observation"]["linked_shader_hashes_match_fixture"], true);
        assert_eq!(profile["same_context_v17_observation"]["pattern_texture_id"], 5);
        assert_eq!(profile["same_context_v17_observation"]["grain_texture_id"], 6);
        assert_eq!(profile["same_context_v17_observation"]["target_texture_id"], 4);
        assert_eq!(profile["same_context_v17_observation"]["target_internal_format_gl_enum"], 32856);
        assert_eq!(profile["same_context_v17_observation"]["target_internal_format_name"], "GL_RGBA8");
        assert_eq!(profile["same_context_v17_observation"]["draw_blend_state"], serde_json::json!({"blend_enabled":true,"color_write_mask":[1,1,1,1],"dst_alpha":771,"dst_rgb":771,"equation_alpha":32774,"equation_rgb":32774,"src_alpha":1,"src_rgb":770}));
        assert_eq!(profile["same_context_v17_observation"]["post_draw_rgba8_sha256"], TARGET_RGBA_SHA256);
        assert_eq!(profile["requires_native_v9_same_context_state_resource_observer"], false);
        assert!(profile["mesa_blend_conversion"]["v17_condition_status"].as_str().unwrap().contains("Confirmed for the V17 captured draw only"));
        assert_eq!(profile["same_drawing_geometry_and_output_join"], true);
        assert_eq!(
            profile["same_drawing_join"]["same_native_vbo_sha256"],
            NATIVE_VBO_SHA256
        );
        assert_eq!(
            profile["same_drawing_join"]["same_after_task2_full_rgba_sha256"],
            TARGET_RGBA_SHA256
        );
        assert_eq!(
            profile["same_drawing_join"]["blend_state_raw"],
            serde_json::json!({"blend_enabled":true,"color_write_mask":[1,1,1,1],"dst_alpha":771,"dst_rgb":771,"equation_alpha":32774,"equation_rgb":32774,"src_alpha":1,"src_rgb":770})
        );
        assert_eq!(
            profile["texture_source_scope"]["mask_upload_source_capture_sha256"],
            "827aee3f567fecdce3e04b4a29217dc1f5d140e130d11ef7720734ef4e7e992c"
        );
        assert_eq!(
            profile["mesa_blend_conversion"]["source_hashes"]["lp_state_fs.c"],
            "8c0696490eaada44b058c18c5552c4540c735508f0066c400f5e8f8b8dc503a6"
        );
        assert_eq!(
            profile["requires_native_v9_same_context_state_resource_observer"],
            false
        );
    }
}
