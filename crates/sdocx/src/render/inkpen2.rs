//! Test-only replay of the captured InkPen2 RTV3 paint shaders.
//!
//! This evaluates the shader field at host pixel centers, then applies the
//! captured R8 max-mask, RGBA8 source-over, and final R8 composite stages.
//! It is an oracle checker for synthetic paint records, not a production path.

#[cfg(test)]
mod tests {
    use flate2::read::ZlibDecoder;
    use serde::Deserialize;
    use std::collections::BTreeMap;
    use std::io::Read;

    const FIXTURE: &str = include_str!("../../../../conformance/inkpen2-paint-native.json");
    const JOINED_SCALE_FIXTURE: &str =
        include_str!("../../../../conformance/inkpen2-joined-scale-native.json");
    const NATIVE_MAX_MASK_FIXTURE: &str =
        include_str!("../../../../conformance/inkpen2-max-mask-native.json");

    #[derive(Deserialize)]
    struct Fixture {
        schema: String,
        paint_model: BTreeMap<String, serde_json::Value>,
        cases: Vec<Case>,
    }

    #[derive(Deserialize)]
    struct Case {
        id: String,
        input: Input,
        input_vector_records: Vec<[f32; 9]>,
        shader_programs: BTreeMap<String, Program>,
        texture_allocations: Vec<TextureAllocation>,
        draw_calls: Vec<DrawCall>,
        uniform_calls: Vec<UniformCall>,
        stage_readbacks_before_invalidate: Vec<Readback>,
    }

    #[derive(Deserialize)]
    struct Input {
        canvas_width: usize,
        canvas_height: usize,
        inverse_scale: [f32; 2],
        points: Vec<PointInput>,
        set_pen_setting_data: SettingData,
    }

    #[derive(Deserialize)]
    struct PointInput {
        position: [f32; 2],
        rgb: [f32; 3],
    }

    #[derive(Deserialize)]
    struct SettingData {
        argb: u32,
        branch_flag: u8,
    }

    #[derive(Deserialize)]
    struct Program {
        linked: bool,
        shaders: Vec<Shader>,
    }

    #[derive(Deserialize)]
    struct Shader {
        #[serde(rename = "type")]
        kind: u32,
        compiled: bool,
        source: String,
    }

    #[derive(Deserialize)]
    struct TextureAllocation {
        internal_format: String,
        width: String,
        height: String,
    }

    #[derive(Deserialize)]
    struct DrawCall {
        program: u32,
        blend_equation: Vec<String>,
        blend_function: Vec<String>,
        blend_enabled_at_draw: Option<bool>,
    }

    #[derive(Deserialize)]
    struct UniformCall {
        call: String,
        values: Option<Vec<f32>>,
        state_before: UniformState,
    }

    #[derive(Deserialize)]
    struct UniformState {
        program: u32,
    }

    #[derive(Deserialize)]
    struct Readback {
        stage: String,
        rgba_bytes: String,
    }

    #[derive(Deserialize)]
    struct JoinedScaleFixture {
        schema: String,
        cases: BTreeMap<String, JoinedScaleCase>,
        partial_alpha_control: JoinedScaleCase,
    }

    #[derive(Deserialize)]
    struct JoinedScaleCase {
        draw_scale: f32,
        draw_matrix_f32_row_major: Vec<f32>,
        source_argb_u32: u32,
        producer: JoinedScaleProducer,
        paint: JoinedScalePaint,
    }

    #[derive(Deserialize)]
    struct JoinedScaleProducer {
        byte_count: usize,
        #[serde(rename = "native_AddPoint_vector_bytes_hex")]
        native_addpoint_vector_bytes_hex: String,
    }

    #[derive(Deserialize)]
    struct JoinedScalePaint {
        readback: JoinedScaleReadback,
        draw_pass: String,
        target_setup: JoinedScaleTarget,
    }

    #[derive(Deserialize)]
    struct JoinedScaleReadback {
        width: usize,
        height: usize,
        uncompressed_byte_count: usize,
        full_bytes_zlib_base64: String,
        draw_matrix_f32_row_major: Vec<f32>,
        actual_uniform_matrix_calls: Vec<JoinedMatrixCall>,
        actual_viewport_calls: Vec<Vec<i32>>,
        actual_instanced_draw_state: Vec<JoinedDrawState>,
        rt_plus_68_u8_before_draw: String,
        draw_rt_plus_68_branch_trace: Vec<serde_json::Value>,
    }

    #[derive(Deserialize)]
    struct JoinedScaleTarget {
        width: usize,
        height: usize,
        format_name: String,
        host_created_destination: bool,
    }

    #[derive(Deserialize)]
    struct NativeMaxMaskFixture {
        schema: String,
        scope: String,
        producer: NativeMaxMaskProducer,
        planes_by_scale: BTreeMap<String, NativeMaxMaskPlane>,
        captures: Vec<NativeMaxMaskCapture>,
    }

    #[derive(Deserialize)]
    struct NativeMaxMaskProducer {
        byte_count: usize,
        record_count: usize,
        #[serde(rename = "native_AddPoint_vector_bytes_hex")]
        native_addpoint_vector_bytes_hex: String,
        sha256: String,
    }

    #[derive(Deserialize)]
    struct NativeMaxMaskPlane {
        width: usize,
        height: usize,
        internal_format: u32,
        rgba_sha256: String,
        red_zlib_base64: String,
    }

    #[derive(Deserialize)]
    struct NativeMaxMaskCapture {
        id: String,
        capture_sha256: String,
        stage0_rgba_sha256: String,
        stage0_red_sha256: String,
        scale: usize,
    }

    #[derive(Deserialize)]
    struct JoinedMatrixCall {
        program: u32,
        count: usize,
        transpose: u8,
        values: Vec<f32>,
    }

    #[derive(Deserialize)]
    struct JoinedDrawState {
        program: u32,
        args: Vec<u32>,
        viewport: Vec<i32>,
    }

    struct PaintReadbacks {
        max_mask: Vec<u8>,
        source_over: Vec<u8>,
        final_composite: Vec<u8>,
    }

    fn uniform_alpha(case: &Case) -> f32 {
        let alpha5 = case
            .uniform_calls
            .iter()
            .find(|row| row.call == "glUniform4fv" && row.state_before.program == 5)
            .unwrap()
            .values
            .as_ref()
            .unwrap()[3];
        if case.input.set_pen_setting_data.branch_flag == 0 {
            let packed_alpha = ((case.input.set_pen_setting_data.argb >> 24) & 0xff) as f32 / 255.0;
            assert_eq!(alpha5.to_bits(), packed_alpha.to_bits());
            return alpha5;
        }
        let alpha6 = case
            .uniform_calls
            .iter()
            .find(|row| row.call == "glUniform4fv" && row.state_before.program == 6)
            .unwrap()
            .values
            .as_ref()
            .unwrap()[3];
        assert_eq!(alpha5.to_bits(), alpha6.to_bits());
        let packed_alpha = ((case.input.set_pen_setting_data.argb >> 24) & 0xff) as f32 / 255.0;
        assert_eq!(alpha5.to_bits(), packed_alpha.to_bits());
        alpha5
    }

    fn decode_hex(value: &str) -> Vec<u8> {
        assert_eq!(value.len() % 2, 0);
        value
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }

    fn decode_base64_zlib(value: &str) -> Vec<u8> {
        use base64::Engine as _;
        let compressed = base64::engine::general_purpose::STANDARD
            .decode(value)
            .unwrap();
        let mut decoder = ZlibDecoder::new(compressed.as_slice());
        let mut bytes = Vec::new();
        decoder.read_to_end(&mut bytes).unwrap();
        bytes
    }

    fn joined_records(case: &JoinedScaleCase) -> Vec<[f32; 9]> {
        let bytes = decode_hex(&case.producer.native_addpoint_vector_bytes_hex);
        assert_eq!(bytes.len(), case.producer.byte_count);
        assert_eq!(bytes.len() % (9 * 4), 0);
        vector_records(&bytes)
    }

    fn vector_records(bytes: &[u8]) -> Vec<[f32; 9]> {
        bytes
            .as_chunks::<{ 9 * 4 }>()
            .0
            .iter()
            .map(|record| {
                std::array::from_fn(|lane| {
                    let offset = lane * 4;
                    f32::from_bits(u32::from_le_bytes(
                        record[offset..offset + 4].try_into().unwrap(),
                    ))
                })
            })
            .collect()
    }

    fn assert_joined_transform(case: &JoinedScaleCase, readback: &JoinedScaleReadback) {
        let matrix = &case.draw_matrix_f32_row_major;
        assert_eq!(matrix.len(), 16);
        assert_eq!(matrix[0].to_bits(), case.draw_scale.to_bits());
        assert_eq!(matrix[5].to_bits(), case.draw_scale.to_bits());
        assert_eq!(matrix[10].to_bits(), 1.0f32.to_bits());
        assert_eq!(matrix[12].to_bits(), 64.0f32.to_bits());
        assert_eq!(matrix[13].to_bits(), 48.0f32.to_bits());
        for (index, value) in matrix.iter().enumerate() {
            let expected = match index {
                0 | 5 => case.draw_scale,
                10 | 15 => 1.0,
                12 => 64.0,
                13 => 48.0,
                _ => 0.0,
            };
            assert_eq!(
                value.to_bits(),
                expected.to_bits(),
                "draw matrix lane {index}"
            );
        }
        assert_eq!(readback.draw_matrix_f32_row_major.len(), 16);
        for (index, (actual, expected)) in readback
            .draw_matrix_f32_row_major
            .iter()
            .zip(matrix)
            .enumerate()
        {
            assert_eq!(
                actual.to_bits(),
                expected.to_bits(),
                "readback matrix lane {index}"
            );
        }

        assert_eq!(readback.actual_uniform_matrix_calls.len(), 1);
        let projection = &readback.actual_uniform_matrix_calls[0];
        assert_eq!(projection.program, 5);
        assert_eq!(projection.count, 1);
        assert_eq!(projection.transpose, 0);
        assert_eq!(projection.values.len(), 16);
        assert_eq!(
            projection.values[0].to_bits(),
            (2.0f32 * case.draw_scale / 256.0).to_bits()
        );
        assert_eq!(
            projection.values[5].to_bits(),
            (2.0f32 * case.draw_scale / 192.0).to_bits()
        );
        assert_eq!(projection.values[12].to_bits(), (-0.5f32).to_bits());
        assert_eq!(projection.values[13].to_bits(), (-0.5f32).to_bits());
        assert_eq!(
            projection.values[10].to_bits(),
            (-3.051_851e-5_f32).to_bits()
        );
        assert_eq!(
            readback.actual_viewport_calls,
            [
                vec![0, 0, 256, 192],
                vec![0, 0, 128, 96],
                vec![0, 0, 128, 96]
            ]
        );
        assert_eq!(readback.rt_plus_68_u8_before_draw, "00");
        assert_eq!(readback.draw_rt_plus_68_branch_trace.len(), 2);
        assert_eq!(
            readback.draw_rt_plus_68_branch_trace[1]["branch_path"],
            "default"
        );
    }

    fn render_joined_scale(case: &JoinedScaleCase) -> Vec<u8> {
        let readback = &case.paint.readback;
        assert_eq!(
            case.paint.draw_pass,
            "program 5 GL_TRIANGLE_STRIP instanced radial stamps; no final composite pass claimed"
        );
        assert_eq!(case.paint.target_setup.width, readback.width);
        assert_eq!(case.paint.target_setup.height, readback.height);
        assert_eq!(case.paint.target_setup.format_name, "RGBA");
        assert!(case.paint.target_setup.host_created_destination);
        assert_eq!(readback.width, 256);
        assert_eq!(readback.height, 192);
        assert_eq!(
            readback.uncompressed_byte_count,
            readback.width * readback.height * 4
        );
        assert_joined_transform(case, readback);
        assert_eq!(readback.actual_instanced_draw_state.len(), 1);
        let draw = &readback.actual_instanced_draw_state[0];
        assert_eq!(draw.program, 5);
        assert_eq!(draw.args, [5, 0, 4, 25]);
        assert_eq!(draw.viewport, [0, 0, 256, 192]);

        let records = joined_records(case);
        assert_eq!(records.len(), 25);
        assert!(records.iter().flatten().all(|value| value.is_finite()));
        let scale = case.draw_scale;
        let tx = case.draw_matrix_f32_row_major[12];
        let ty = case.draw_matrix_f32_row_major[13];
        let uniform_alpha = ((case.source_argb_u32 >> 24) & 0xff) as f32 / 255.0;
        let mut rgba = vec![0u8; readback.uncompressed_byte_count];

        for y in 0..readback.height {
            let source_y = (y as f32 + 0.5 - ty) / scale;
            for x in 0..readback.width {
                let source_x = (x as f32 + 0.5 - tx) / scale;
                let pixel = (y * readback.width + x) * 4;
                for record in &records {
                    let dx = (source_x - record[0]) / (2.0 * record[2]);
                    let dy = (source_y - record[1]) / (2.0 * record[3]);
                    let distance = (dx * dx + dy * dy).sqrt();
                    let alpha = ((0.5 - distance) / (0.5 - record[4])).clamp(0.0, 1.0) * record[5];
                    if alpha <= 0.0 {
                        continue;
                    }

                    // The captured GL_RGBA8 target quantizes the shader output
                    // before ONE/ONE_MINUS_SRC_ALPHA blending each instance.
                    let source_alpha = quantize_unorm8(alpha * uniform_alpha);
                    let blend_alpha = f32::from(source_alpha) / 255.0;
                    for channel in 0..3 {
                        let source =
                            f32::from(quantize_unorm8(record[6 + channel] * alpha * uniform_alpha))
                                / 255.0;
                        let destination = f32::from(rgba[pixel + channel]) / 255.0;
                        rgba[pixel + channel] =
                            quantize_unorm8(source + destination * (1.0 - blend_alpha));
                    }
                    let source = blend_alpha;
                    let destination = f32::from(rgba[pixel + 3]) / 255.0;
                    rgba[pixel + 3] = quantize_unorm8(source + destination * (1.0 - blend_alpha));
                }
            }
        }
        rgba
    }

    fn quantize_unorm8(value: f32) -> u8 {
        (value.clamp(0.0, 1.0) * 255.0).round() as u8
    }

    fn fragment_alpha(record: &[f32; 9], pixel_center: [f32; 2]) -> Option<f32> {
        let dx = pixel_center[0] - record[0];
        let dy = pixel_center[1] - record[1];
        let half_width = record[2].abs();
        let half_height = record[3].abs();
        if half_width == 0.0
            || half_height == 0.0
            || dx.abs() > half_width
            || dy.abs() > half_height
        {
            return None;
        }

        // Native quad UVs interpolate from (0,0) to (1,1). At a pixel center,
        // this is the exact affine fragCoord produced by the captured varying.
        let u = 0.5 + dx / (2.0 * half_width);
        let v = 0.5 + dy / (2.0 * half_height);
        let ux = u - 0.5;
        let uy = v - 0.5;
        let distance = (ux * ux + uy * uy).sqrt();
        let mut alpha = record[5];
        if distance > record[4] {
            alpha *= 1.0 - (distance - record[4]) / (0.5 - record[4]);
        }
        Some(alpha)
    }

    fn render_model(case: &Case) -> PaintReadbacks {
        let width = case.input.canvas_width;
        let height = case.input.canvas_height;
        let pixels = width * height;
        let alpha_uniform = uniform_alpha(case);
        assert!(
            case.input
                .inverse_scale
                .iter()
                .all(|scale| scale.is_finite() && *scale > 0.0)
        );
        assert_eq!(case.input.points.len(), case.input_vector_records.len());

        if case.input.set_pen_setting_data.branch_flag == 0 {
            return render_direct(case, alpha_uniform);
        }
        let mut max_mask = vec![0u8; pixels];
        let mut source_over = vec![0u8; pixels * 4];

        for (point, record) in case.input.points.iter().zip(&case.input_vector_records) {
            assert_eq!(record[0].to_bits(), point.position[0].to_bits());
            assert_eq!(record[1].to_bits(), point.position[1].to_bits());
            assert_eq!(record[6].to_bits(), point.rgb[0].to_bits());
            assert_eq!(record[7].to_bits(), point.rgb[1].to_bits());
            assert_eq!(record[8].to_bits(), point.rgb[2].to_bits());

            for y in 0..height {
                for x in 0..width {
                    let center = [x as f32 + 0.5, y as f32 + 0.5];
                    let Some(alpha) = fragment_alpha(record, center) else {
                        continue;
                    };
                    let index = y * width + x;

                    // R8 + GL_MAX, with ONE/ONE factors. Quantization occurs
                    // when each fragment is written to the normalized target.
                    max_mask[index] = max_mask[index].max(quantize_unorm8(alpha));

                    // RGBA8 source-over, with the native source color already
                    // premultiplied by the shader's analytic alpha.
                    let dst = &mut source_over[index * 4..index * 4 + 4];
                    let source_alpha = quantize_unorm8(alpha * alpha_uniform);
                    let blend_alpha = f32::from(source_alpha) / 255.0;
                    for channel in 0..3 {
                        let src =
                            f32::from(quantize_unorm8(record[6 + channel] * alpha * alpha_uniform))
                                / 255.0;
                        let out = src + (f32::from(dst[channel]) / 255.0) * (1.0 - blend_alpha);
                        dst[channel] = quantize_unorm8(out);
                    }
                    let out_alpha = blend_alpha + (f32::from(dst[3]) / 255.0) * (1.0 - blend_alpha);
                    dst[3] = quantize_unorm8(out_alpha);
                }
            }
        }

        let mut final_composite = vec![0u8; pixels * 4];
        for index in 0..pixels {
            let source = &source_over[index * 4..index * 4 + 4];
            let mask = f32::from(max_mask[index]) / 255.0;
            let alpha = mask * alpha_uniform;
            if alpha <= 0.0 || source[3] == 0 {
                final_composite[index * 4 + 3] = 255;
                continue;
            }
            let source_alpha = f32::from(source[3]) / 255.0;
            let red = (f32::from(source[0]) / 255.0) / source_alpha * alpha;
            final_composite[index * 4] = quantize_unorm8(red);
            // The native final target is single-channel R8. Missing channels
            // read as zero and alpha reads as one through GL_RGBA readback.
            final_composite[index * 4 + 3] = 255;
        }

        PaintReadbacks {
            max_mask: max_mask.into_iter().flat_map(|r| [r, 0, 0, 255]).collect(),
            source_over,
            final_composite,
        }
    }

    fn render_direct(case: &Case, alpha_uniform: f32) -> PaintReadbacks {
        let width = case.input.canvas_width;
        let height = case.input.canvas_height;
        let pixels = width * height;
        let mut red = vec![0u8; pixels];
        for (point, record) in case.input.points.iter().zip(&case.input_vector_records) {
            assert_eq!(record[0].to_bits(), point.position[0].to_bits());
            assert_eq!(record[1].to_bits(), point.position[1].to_bits());
            assert_eq!(record[6].to_bits(), point.rgb[0].to_bits());
            assert_eq!(record[7].to_bits(), point.rgb[1].to_bits());
            assert_eq!(record[8].to_bits(), point.rgb[2].to_bits());
            for y in 0..height {
                for x in 0..width {
                    let center = [x as f32 + 0.5, y as f32 + 0.5];
                    let Some(alpha) = fragment_alpha(record, center) else {
                        continue;
                    };
                    let src_red = quantize_unorm8(record[6] * alpha * alpha_uniform);
                    let src_alpha = alpha * alpha_uniform;
                    let index = y * width + x;
                    let dst = f32::from(red[index]) / 255.0;
                    red[index] =
                        quantize_unorm8(f32::from(src_red) / 255.0 + dst * (1.0 - src_alpha));
                }
            }
        }
        let readback = red.into_iter().flat_map(|r| [r, 0, 0, 255]).collect();
        PaintReadbacks {
            max_mask: vec![0; pixels * 4],
            source_over: vec![0; pixels * 4],
            final_composite: readback,
        }
    }

    fn assert_capture_contract(fixture: &Fixture, case: &Case) {
        assert_eq!(fixture.schema, "sdocx.inkpen2.paint.native.v1");
        assert!(fixture.paint_model.contains_key("final_composite"));
        if case.input.set_pen_setting_data.branch_flag == 0 {
            let program = &case.shader_programs["5"];
            assert!(program.linked);
            assert_eq!(program.shaders.len(), 2);
            for shader in &program.shaders {
                assert!(shader.compiled);
            }
            assert_eq!(program.shaders[0].kind, 35633);
            assert_eq!(program.shaders[1].kind, 35632);
            assert!(
                program.shaders[1]
                    .source
                    .contains("FragColor = vec4(fragColor, 1.0) * alpha * uInputColor.a")
            );
            assert_eq!(case.draw_calls.len(), 1);
            assert_eq!(case.draw_calls[0].program, 5);
            assert_eq!(case.draw_calls[0].blend_enabled_at_draw, Some(true));
            assert_eq!(case.draw_calls[0].blend_equation, ["0x8006", "0x8006"]);
            assert_eq!(
                case.draw_calls[0].blend_function,
                ["0x1", "0x303", "0x1", "0x303"]
            );
            assert_eq!(case.texture_allocations.len(), 2);
            assert_eq!(case.texture_allocations[0].internal_format, "0x8229");
            assert_eq!(case.texture_allocations[1].internal_format, "0x8058");
            return;
        }
        let program3 = &case.shader_programs["3"];
        let program5 = &case.shader_programs["5"];
        let program6 = &case.shader_programs["6"];
        assert!(program3.linked && program5.linked && program6.linked);
        assert_eq!(program3.shaders.len(), 2);
        assert_eq!(program5.shaders.len(), 2);
        assert_eq!(program6.shaders.len(), 2);
        for shader in program3
            .shaders
            .iter()
            .chain(&program5.shaders)
            .chain(&program6.shaders)
        {
            assert!(shader.compiled);
        }
        for program in [program3, program5, program6] {
            assert_eq!(program.shaders[0].kind, 35633);
            assert_eq!(program.shaders[1].kind, 35632);
        }
        assert!(
            program3.shaders[1]
                .source
                .contains("alpha *= (1.0 - (dist - innerRadius) / (0.5 - innerRadius))")
        );
        assert!(
            program5.shaders[1]
                .source
                .contains("vec4(fragColor, 1.0) * alpha * uInputColor.a")
        );
        assert!(
            program6.shaders[1]
                .source
                .contains("texture(uMaxTexture, vUV).r * uInputColor.a")
        );
        assert!(
            program6.shaders[1]
                .source
                .contains("srcOverTexel.rgb / srcOverTexel.a")
        );
        assert_eq!(case.draw_calls.len(), 3);
        assert_eq!(case.draw_calls[0].program, 3);
        assert_eq!(case.draw_calls[0].blend_equation, ["0x8008", "0x8008"]);
        assert_eq!(
            case.draw_calls[0].blend_function,
            ["0x1", "0x1", "0x1", "0x1"]
        );
        assert_eq!(case.draw_calls[1].program, 5);
        assert_eq!(case.draw_calls[1].blend_equation, ["0x8006", "0x8006"]);
        assert_eq!(
            case.draw_calls[1].blend_function,
            ["0x1", "0x303", "0x1", "0x303"]
        );
        assert_eq!(case.draw_calls[2].program, 6);
        assert_eq!(case.draw_calls[2].blend_equation, ["0x8006", "0x8006"]);
        assert_eq!(
            case.draw_calls[2].blend_function,
            ["0x1", "0x0", "0x1", "0x0"]
        );
        assert_eq!(case.texture_allocations.len(), 2);
        assert_eq!(case.texture_allocations[0].internal_format, "0x8229"); // GL_R8
        assert_eq!(case.texture_allocations[1].internal_format, "0x8058"); // GL_RGBA8
        assert_eq!(case.texture_allocations[0].width, "0x80");
        assert_eq!(case.texture_allocations[0].height, "0x60");
        assert_eq!(case.texture_allocations[1].width, "0x80");
        assert_eq!(case.texture_allocations[1].height, "0x60");

        let projection = case
            .uniform_calls
            .iter()
            .find(|row| row.call == "glUniformMatrix4fv" && row.state_before.program == 3)
            .unwrap()
            .values
            .as_ref()
            .unwrap();
        assert_eq!(
            projection[0].to_bits(),
            (2.0 / case.input.canvas_width as f32).to_bits()
        );
        assert_eq!(
            projection[5].to_bits(),
            (2.0 / case.input.canvas_height as f32).to_bits()
        );
        assert_eq!(projection[12].to_bits(), (-1.0f32).to_bits());
        assert_eq!(projection[13].to_bits(), (-1.0f32).to_bits());
        for program in [5, 6] {
            let color = case
                .uniform_calls
                .iter()
                .find(|row| row.call == "glUniform4fv" && row.state_before.program == program)
                .unwrap()
                .values
                .as_ref()
                .unwrap();
            assert_eq!(color.len(), 4);
            assert_eq!(color[3].to_bits(), uniform_alpha(case).to_bits());
        }
    }

    #[test]
    fn native_shader_field_and_quantized_rgba_readbacks_match() {
        let fixture: Fixture = serde_json::from_str(FIXTURE).unwrap();
        assert_eq!(fixture.cases.len(), 10);
        let mut cases = fixture.cases.iter().collect::<Vec<_>>();
        cases.sort_by_key(|case| case.id.as_str());

        for case in cases {
            assert_capture_contract(&fixture, case);
            let modeled = render_model(case);
            let stages = if case.input.set_pen_setting_data.branch_flag == 0 {
                vec![("direct_source_over", &modeled.final_composite)]
            } else {
                vec![
                    ("max_alpha_mask", &modeled.max_mask),
                    ("source_over_canvas", &modeled.source_over),
                    ("final_composite", &modeled.final_composite),
                ]
            };
            for (stage_name, modeled_bytes) in stages {
                let captured = case
                    .stage_readbacks_before_invalidate
                    .iter()
                    .find(|readback| readback.stage == stage_name)
                    .unwrap();
                assert_eq!(
                    modeled_bytes.len(),
                    case.input.canvas_width * case.input.canvas_height * 4,
                    "{} {stage_name}: complete framebuffer extent",
                    case.id
                );
                let captured_bytes = decode_hex(&captured.rgba_bytes);
                assert_eq!(captured_bytes.len(), modeled_bytes.len());
                for (index, (modeled_pixel, captured_pixel)) in modeled_bytes
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .zip(captured_bytes.as_chunks::<4>().0.iter())
                    .enumerate()
                {
                    assert_eq!(
                        modeled_pixel,
                        captured_pixel,
                        "{} {stage_name}: pixel ({}, {})",
                        case.id,
                        index % case.input.canvas_width,
                        index / case.input.canvas_width
                    );
                }
            }
        }

        let single = fixture
            .cases
            .iter()
            .find(|case| case.id == "single")
            .unwrap();
        let record = &single.input_vector_records[0];
        for y in [47usize, 48] {
            for x in [63usize, 64] {
                let alpha = fragment_alpha(record, [x as f32 + 0.5, y as f32 + 0.5]).unwrap();
                let modeled_alpha = quantize_unorm8(alpha);
                let capture = single
                    .stage_readbacks_before_invalidate
                    .iter()
                    .find(|readback| readback.stage == "max_alpha_mask")
                    .unwrap();
                let bytes = decode_hex(&capture.rgba_bytes);
                let offset = (y * single.input.canvas_width + x) * 4;
                assert_eq!(&bytes[offset..offset + 4], &[modeled_alpha, 0, 0, 255]);
                assert_eq!(modeled_alpha, 126);
            }
        }
    }

    #[test]
    fn joined_native_scale_field_matches_captured_readbacks() {
        let fixture: JoinedScaleFixture = serde_json::from_str(JOINED_SCALE_FIXTURE).unwrap();
        assert_eq!(fixture.schema, "sdocx-inkpen2-joined-scale-native-v1");
        assert_eq!(fixture.cases.len(), 3);

        let mut cases = fixture
            .cases
            .iter()
            .map(|(name, case)| (name.as_str(), case))
            .collect::<Vec<_>>();
        cases.sort_by_key(|(name, _)| *name);
        for (name, case) in cases.into_iter().chain(std::iter::once((
            "partial_alpha_control",
            &fixture.partial_alpha_control,
        ))) {
            let modeled = render_joined_scale(case);
            let captured = decode_base64_zlib(&case.paint.readback.full_bytes_zlib_base64);
            assert_eq!(captured.len(), case.paint.readback.uncompressed_byte_count);
            assert_eq!(modeled.len(), captured.len());

            if name == "partial_alpha_control" {
                assert_eq!(case.source_argb_u32, 0x8033_6699);
                assert_eq!(modeled, captured, "partial alpha full framebuffer");
            } else {
                assert_eq!(case.source_argb_u32, 0xff33_6699);
                let mut max_error = 0u8;
                for (pixel_index, (model, native)) in modeled
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .zip(captured.as_chunks::<4>().0.iter())
                    .enumerate()
                {
                    assert_eq!(
                        model[3] == 0,
                        native[3] == 0,
                        "{name} alpha support at pixel {pixel_index}"
                    );
                    assert_eq!(
                        model[..3].iter().any(|channel| *channel != 0),
                        native[..3].iter().any(|channel| *channel != 0),
                        "{name} color support at pixel {pixel_index}"
                    );
                    for channel in 0..4 {
                        max_error = max_error.max(model[channel].abs_diff(native[channel]));
                    }
                }
                assert!(max_error <= 1, "{name} maximum RGBA error {max_error}");
            }
        }
    }

    #[test]
    fn native_max_mask_matches_six_captured_r8_readbacks() {
        use sha2::{Digest, Sha256};

        let fixture: NativeMaxMaskFixture = serde_json::from_str(NATIVE_MAX_MASK_FIXTURE).unwrap();
        assert_eq!(fixture.schema, "sdocx-inkpen2-native-max-mask-v1");
        assert_eq!(
            fixture.scope,
            "Bounded emulated pinned-native producer/draw with host Mesa readback; not device or document appearance parity."
        );
        assert_eq!(
            format!("{:x}", Sha256::digest(NATIVE_MAX_MASK_FIXTURE.as_bytes())),
            "7ddc9d510e33f84774aa82aa61a8441b86641fc5c74f98a642397d091f9db162"
        );
        assert_eq!(fixture.producer.byte_count, 900);
        assert_eq!(fixture.producer.record_count, 25);
        let producer_bytes = decode_hex(&fixture.producer.native_addpoint_vector_bytes_hex);
        assert_eq!(producer_bytes.len(), fixture.producer.byte_count);
        assert_eq!(
            format!("{:x}", Sha256::digest(&producer_bytes)),
            fixture.producer.sha256
        );
        assert_eq!(
            fixture.producer.sha256,
            "570044bb059690f865cfa6f4f99c8ba7801573144242ee0913654973b0d0e987"
        );
        let records = vector_records(&producer_bytes);
        assert_eq!(records.len(), fixture.producer.record_count);

        assert_eq!(fixture.captures.len(), 6);
        let mut seen_cases = std::collections::BTreeSet::new();
        for capture in &fixture.captures {
            assert_eq!(capture.capture_sha256.len(), 64);
            assert!(
                capture
                    .capture_sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit())
            );
            assert!([1, 2, 4].contains(&capture.scale));
            let alpha = capture
                .id
                .strip_prefix(&format!("scale{}_aa1_alpha", capture.scale))
                .unwrap();
            assert!(matches!(alpha, "128" | "255"), "{}", capture.id);
            assert!(seen_cases.insert(capture.id.as_str()), "duplicate capture");
            let key = capture.scale.to_string();
            let plane = fixture.planes_by_scale.get(&key).unwrap();
            assert_eq!((plane.width, plane.height), (128, 96));
            assert_eq!(plane.internal_format, 33321, "GL_R8 source mask");
            let red = decode_base64_zlib(&plane.red_zlib_base64);
            assert_eq!(red.len(), plane.width * plane.height);
            let mut rgba = Vec::with_capacity(red.len() * 4);
            for &value in &red {
                rgba.extend_from_slice(&[value, 0, 0, 255]);
            }
            assert_eq!(
                format!("{:x}", Sha256::digest(&rgba)),
                capture.stage0_rgba_sha256,
                "{} RGBA channel contract/hash",
                capture.id
            );
            assert_eq!(capture.stage0_rgba_sha256, plane.rgba_sha256);
            assert_eq!(
                format!("{:x}", Sha256::digest(&red)),
                capture.stage0_red_sha256,
                "{} R plane hash",
                capture.id
            );

            let mut modeled = vec![0u8; red.len()];
            for y in 0..plane.height {
                for x in 0..plane.width {
                    let pixel_center = [
                        (x as f32 + 0.5) / capture.scale as f32,
                        (y as f32 + 0.5) / capture.scale as f32,
                    ];
                    let max_alpha = records
                        .iter()
                        .filter_map(|record| fragment_alpha(record, pixel_center))
                        .fold(0.0f32, f32::max);
                    modeled[y * plane.width + x] = quantize_unorm8(max_alpha);
                }
            }
            assert_eq!(modeled, red, "{} native stage-0 M bytes", capture.id);
        }
        assert_eq!(fixture.planes_by_scale.len(), 3);
        assert!(fixture.planes_by_scale.contains_key("1"));
        assert!(fixture.planes_by_scale.contains_key("2"));
        assert!(fixture.planes_by_scale.contains_key("4"));
        assert_eq!(seen_cases.len(), 6);
    }
}
