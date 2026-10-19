//! Test-only replay of the captured InkPen2 RTV3 paint shaders.
//!
//! This evaluates the shader field at host pixel centers, then applies the
//! captured R8 max-mask, RGBA8 source-over, and final R8 composite stages.
//! It is an oracle checker for synthetic paint records, not a production path.

#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use std::collections::BTreeMap;

    const FIXTURE: &str = include_str!("../../../../conformance/inkpen2-paint-native.json");

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
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
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

    fn assert_capture_contract(fixture: &Fixture, case: &Case) {
        assert_eq!(fixture.schema, "sdocx.inkpen2.paint.native.v1");
        assert!(fixture.paint_model.contains_key("final_composite"));
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
        assert_eq!(fixture.cases.len(), 6);
        let mut cases = fixture.cases.iter().collect::<Vec<_>>();
        cases.sort_by_key(|case| case.id.as_str());

        for case in cases {
            assert_capture_contract(&fixture, case);
            let modeled = render_model(case);
            for (stage_name, modeled_bytes) in [
                ("max_alpha_mask", &modeled.max_mask),
                ("source_over_canvas", &modeled.source_over),
                ("final_composite", &modeled.final_composite),
            ] {
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
                    .chunks_exact(4)
                    .zip(captured_bytes.chunks_exact(4))
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
}
