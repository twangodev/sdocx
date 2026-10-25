//! Ignored native-capture audits for emitted radial PDF fields.

use super::{PdfRadialRecord, tests::run_calculator};

#[test]
#[ignore = "offline audit of the pinned emitted AS18 thin Type 4 PDFs and native Mesa pair"]
fn audit_emitted_thin_type4_functions_at_fixed_96dpi_pixel_centers() {
    use flate2::read::ZlibDecoder;
    use sha2::{Digest, Sha256};
    use std::{fs, io::Read, path::Path};

    const PAIR_SHA: &str = "0204f19ed9d325b78dce5e050e17082e6957d76d7f64efdaf0663aa42dc7cae8";
    const V9_MANIFEST_SHA: &str =
        "0dce8f1b707a9f268179968a25d3d90cebeffc2587d645b52ede2533bda7a322";
    const READER_REPORT_SHA: &str =
        "8359e0b6d9eae1202e37ece56199ed3cd6e73cbf17ee8f4ba05bac96eb01ac80";
    const V8_UPLOAD_SHA: &str = "621623590e5818b88167e910cce8dca9b0478790dcc7d95b2e55b0e7eb606a36";
    const V8_SELECTED_FRAGMENT_SHA: &str =
        "132fc7199c6faa0c214a929227ad865f283d0412a6770b9da0da1790e61d7fcb";
    const WIDTH: usize = 1920;
    const HEIGHT: usize = 1080;
    const PAGE_WIDTH_POINTS: f64 = 1440.0;
    const PAGE_HEIGHT_POINTS: f64 = 810.0;
    const PDF_RENDER_SCALE: f64 = 96.0 / 72.0;
    const PDF_POINT_PER_PIXEL: f64 = 72.0 / 96.0;

    fn sha256(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    fn decode_hex_bytes(value: &str) -> Vec<u8> {
        assert_eq!(value.len() % 2, 0, "odd-length hex");
        value
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| {
                u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).expect("hex byte")
            })
            .collect()
    }

    fn json_f64(value: &serde_json::Value) -> f64 {
        value.as_f64().expect("finite JSON number")
    }

    fn pdf_number(value: &lopdf::Object) -> f64 {
        match value {
            lopdf::Object::Integer(value) => *value as f64,
            lopdf::Object::Real(value) => f64::from(*value),
            other => panic!("expected PDF numeric object, got {other:?}"),
        }
    }

    fn normalized_records(records: &[PdfRadialRecord], bounds: [f64; 4]) -> Vec<[f64; 9]> {
        let width = bounds[2] - bounds[0];
        let height = bounds[3] - bounds[1];
        records
            .iter()
            .map(|record| {
                [
                    (f64::from(record[0]) - bounds[0]) / width,
                    1.0 - (f64::from(record[1]) - bounds[1]) / height,
                    f64::from(record[2]) / width,
                    f64::from(record[3]) / height,
                    f64::from(record[4]),
                    f64::from(record[5]),
                    f64::from(record[6]),
                    f64::from(record[7]),
                    f64::from(record[8]),
                ]
            })
            .collect()
    }

    fn direct_source_over_alpha(records: &[[f64; 9]], saved_alpha: f64, x: f64, y: f64) -> f64 {
        let mut q = 0.0;
        for record in records {
            let [cx, cy, rx, ry, edge, coverage, ..] = *record;
            let d = (((x - cx) / (2.0 * rx)).powi(2) + ((y - cy) / (2.0 * ry)).powi(2)).sqrt();
            let c = if d <= edge {
                coverage
            } else if d >= 0.5 {
                0.0
            } else {
                coverage * (0.5 - d) / (0.5 - edge)
            };
            let a = c * saved_alpha;
            q = a + q * (1.0 - a);
        }
        q
    }

    fn direct_source_over_alpha_unorm8(
        records: &[[f64; 9]],
        saved_alpha: f64,
        x: f64,
        y: f64,
    ) -> f64 {
        let quantize = |value: f64| (value.clamp(0.0, 1.0) * 255.0 + 0.5).floor() / 255.0;
        let mut q = 0.0;
        for record in records {
            let [cx, cy, rx, ry, edge, coverage, ..] = *record;
            let d = (((x - cx) / (2.0 * rx)).powi(2) + ((y - cy) / (2.0 * ry)).powi(2)).sqrt();
            let c = if d <= edge {
                coverage
            } else if d >= 0.5 {
                0.0
            } else {
                coverage * (0.5 - d) / (0.5 - edge)
            };
            let a = c * saved_alpha;
            q = quantize(a + q * (1.0 - a));
        }
        q
    }

    fn native_to_viewport_affine(matrix: &[f64], viewport: &[u32]) -> [f64; 6] {
        assert_eq!(matrix.len(), 16);
        assert_eq!(viewport.len(), 4);
        for index in [1, 2, 3, 4, 6, 7, 8, 9, 11, 14] {
            assert!(
                matrix[index].abs() < 1.0e-12,
                "unsupported matrix term {index}"
            );
        }
        assert!(matrix[10].is_finite());
        assert!((matrix[15] - 1.0).abs() < 1.0e-12);
        let width = f64::from(viewport[2]);
        let height = f64::from(viewport[3]);
        let scale_x = matrix[0] * width / 2.0;
        let scale_y = matrix[5] * height / 2.0;
        let translate_x = f64::from(viewport[0]) + (matrix[12] + 1.0) * width / 2.0;
        let translate_y_gl = f64::from(viewport[1]) + (matrix[13] + 1.0) * height / 2.0;
        [
            scale_x,
            0.0,
            0.0,
            -scale_y,
            translate_x,
            height - translate_y_gl,
        ]
    }

    fn inverse_affine(affine: [f64; 6], x: f64, y: f64) -> (f64, f64) {
        let [a, b, c, d, e, f] = affine;
        let determinant = a * d - b * c;
        assert!(determinant.is_finite() && determinant.abs() > 1.0e-12);
        let x = x - e;
        let y = y - f;
        (
            (d * x - c * y) / determinant,
            (-b * x + a * y) / determinant,
        )
    }

    fn inverse_pdf_ctm(ctm: [f64; 6], x: f64, y: f64) -> (f64, f64) {
        let [a, b, c, d, e, f] = ctm;
        let determinant = a * d - b * c;
        assert!(determinant.is_finite() && determinant.abs() > 1.0e-12);
        let x = x - e;
        let y = y - f;
        (
            (d * x - c * y) / determinant,
            (-b * x + a * y) / determinant,
        )
    }

    let artifact_dir = Path::new(
        &std::env::var("SDOCX_FOUNTAIN_V16_THIN_TYPE4_ARTIFACT_DIR")
            .expect("set root-v9 artifact directory"),
    )
    .canonicalize()
    .expect("root-v9 artifact directory exists");
    let pair_path = Path::new(
        &std::env::var("SDOCX_FOUNTAIN_V16_THIN_TYPE4_PAIR_JSON")
            .expect("set complete pinned V8 pair JSON"),
    )
    .canonicalize()
    .expect("pinned V8 pair exists");
    let output_dir = Path::new(
        &std::env::var("SDOCX_FOUNTAIN_V16_THIN_TYPE4_DIAGNOSTIC_OUT")
            .expect("set a fresh output directory"),
    )
    .to_path_buf();
    fs::create_dir(&output_dir).expect("diagnostic output directory must be new");

    let manifest_bytes = fs::read(artifact_dir.join("manifest.json")).unwrap();
    assert_eq!(sha256(&manifest_bytes), V9_MANIFEST_SHA);
    let manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes).unwrap();
    assert_eq!(manifest["source_pair_sha256"], PAIR_SHA);
    assert_eq!(manifest["native_upload_sha256"], V8_UPLOAD_SHA);
    assert_eq!(manifest["native_execution"], false);
    let pair_bytes = fs::read(&pair_path).unwrap();
    assert_eq!(sha256(&pair_bytes), PAIR_SHA);
    let pair: serde_json::Value = serde_json::from_slice(&pair_bytes).unwrap();
    assert_eq!(
        pair["capture_diagnostics"]["status"],
        "host_mesa_pair_captured_no_device_or_document_parity_claim"
    );
    let reader_report_path =
        Path::new("/tmp/sdocx-fountain-as18-v16-thin-readers-root-v5/report.json");
    let reader_bytes = fs::read(reader_report_path).expect("pinned existing reader report");
    assert_eq!(sha256(&reader_bytes), READER_REPORT_SHA);
    let reader_report: serde_json::Value = serde_json::from_slice(&reader_bytes).unwrap();
    assert_eq!(reader_report["manifest_sha256"], V9_MANIFEST_SHA);

    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let v8_cases = pair["cases"].as_array().expect("two actual V8 cases");
    let output_cases = manifest["cases"].as_array().expect("two V9 artifact cases");
    assert_eq!(v8_cases.len(), 2);
    assert_eq!(output_cases.len(), 2);
    assert_eq!(
        v8_cases[0]["renderer_context_identity"], v8_cases[1]["renderer_context_identity"],
        "the two alpha cases share one captured GL context"
    );
    let alpha128_program6_link = v8_cases[0]["renderer_shader_link_events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|event| event["program"] == 6)
        .collect::<Vec<_>>();
    assert!(
        alpha128_program6_link
            .iter()
            .any(|event| { event["call"] == "glCreateProgram" })
    );
    assert!(
        alpha128_program6_link
            .iter()
            .any(|event| { event["call"] == "glAttachShader" && event["shader"] == 2 })
    );
    assert!(
        alpha128_program6_link
            .iter()
            .any(|event| event["call"] == "glLinkProgram")
    );
    let mut case_receipts = Vec::new();

    for (case_index, expected_name, expected_pdf_sha, expected_rgb_fn_sha, expected_mask_fn_sha) in [
        (
            0,
            "alpha128",
            "f7bf24c538af161f2808df7cd4956845e6e8956b3ffc966942e2991d52897221",
            "51d6808257b50c5f7a92193d0d67d0954c179b950f212ef27df87b2144cd5711",
            "4aa748162490d902bd549c854f48c1dc4ba820a618e80f43e7f15222028d0d7c",
        ),
        (
            1,
            "alpha255",
            "288d11c6e660432c50fbb8baf7a3c1d0f0427ccb6bfc5e0540e3f694c78a712d",
            "7459e2928614fb914828da180eed10466e4baacaad873b2ba05442e75f6b39fa",
            "397f4f5645f8213eaa3b762ced4b54babd8bc571c926ade87795ec770150c2d7",
        ),
    ] {
        let case = &v8_cases[case_index];
        let output_case = &output_cases[case_index];
        assert_eq!(case["case"], expected_name);
        assert_eq!(output_case["case"], expected_name);
        let attached_selected_fragment = case["renderer_shader_link_events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| {
                event["call"] == "glAttachShader" && event["program"] == 6 && event["shader"] == 2
            });
        if case_index == 0 {
            assert!(attached_selected_fragment);
        } else {
            assert!(
                !attached_selected_fragment,
                "alpha255 reuses program 6's existing linked executable"
            );
            let alpha255_program6_events = case["renderer_program_lifecycle_events"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|event| event["program"] == 6)
                .collect::<Vec<_>>();
            assert_eq!(alpha255_program6_events.len(), 3);
            assert!(alpha255_program6_events.iter().all(|event| {
                event["call"] == "glUseProgram"
                    || event["call"] == "glUniform4fv"
                    || event["call"] == "glDrawArraysInstanced"
            }));
            assert!(!alpha255_program6_events.iter().any(|event| {
                event["call"] == "glDeleteProgram" || event["call"] == "glLinkProgram"
            }));
        }
        let selected_fragment_rows = case["renderer_shader_source_calls"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["shader"] == 2)
            .collect::<Vec<_>>();
        let selected_fragment = selected_fragment_rows.last().unwrap()["source"][0]
            .as_str()
            .unwrap();
        assert_eq!(
            sha256(selected_fragment.as_bytes()),
            V8_SELECTED_FRAGMENT_SHA
        );
        assert!(selected_fragment.contains("precision mediump float"));
        assert!(selected_fragment.contains("distance(center, fragCoord)"));
        assert!(!selected_fragment.contains("fwidth"));
        let type4 = &output_case["type4_direct_source_over_experiment"];
        assert_eq!(type4["is_production_admission"], false);
        assert_eq!(type4["record_count"], 76);
        let pdf_path = artifact_dir.join(type4["pdf_path"].as_str().unwrap());
        let pdf_bytes = fs::read(&pdf_path).unwrap();
        assert_eq!(sha256(&pdf_bytes), expected_pdf_sha);
        let pdf = lopdf::Document::load_mem(&pdf_bytes).expect("pinned Type 4 candidate PDF");
        assert_eq!(pdf.get_pages().len(), 1);
        let page_id = *pdf.get_pages().values().next().unwrap();
        let page = pdf.get_object(page_id).unwrap().as_dict().unwrap();
        let media_box = page.get(b"MediaBox").unwrap().as_array().unwrap();
        let media_box = media_box.iter().map(pdf_number).collect::<Vec<_>>();
        assert_eq!(
            media_box,
            vec![0.0, 0.0, PAGE_WIDTH_POINTS, PAGE_HEIGHT_POINTS]
        );

        let mut mask_functions = Vec::new();
        let mut rgb_functions = Vec::new();
        let mut function4_count = 0;
        let mut type1_shading_count = 0;
        let mut unit_form_bboxes = 0;
        for object in pdf.objects.values() {
            if let Ok(stream) = object.as_stream() {
                if stream
                    .dict
                    .get(b"Subtype")
                    .ok()
                    .and_then(|v| v.as_name().ok())
                    == Some(b"Form".as_slice())
                    && let Ok(bbox) = stream.dict.get(b"BBox").and_then(lopdf::Object::as_array)
                    && bbox.iter().map(pdf_number).collect::<Vec<_>>() == vec![0.0, 0.0, 1.0, 1.0]
                {
                    unit_form_bboxes += 1;
                }
                if stream
                    .dict
                    .get(b"FunctionType")
                    .ok()
                    .and_then(|v| v.as_i64().ok())
                    == Some(4)
                {
                    function4_count += 1;
                    let domain = stream.dict.get(b"Domain").unwrap().as_array().unwrap();
                    assert_eq!(
                        domain.iter().map(pdf_number).collect::<Vec<_>>(),
                        vec![0.0, 1.0, 0.0, 1.0]
                    );
                    let range = stream.dict.get(b"Range").unwrap().as_array().unwrap();
                    let decoded = stream.decompressed_content().unwrap();
                    match range.len() {
                        2 => mask_functions.push(decoded),
                        6 => rgb_functions.push(decoded),
                        length => panic!("unexpected Type 4 output range length {length}"),
                    }
                }
            }
            if let Ok(dict) = object.as_dict()
                && dict.get(b"ShadingType").ok().and_then(|v| v.as_i64().ok()) == Some(1)
            {
                type1_shading_count += 1;
                let domain = dict.get(b"Domain").unwrap().as_array().unwrap();
                assert_eq!(
                    domain.iter().map(pdf_number).collect::<Vec<_>>(),
                    vec![0.0, 1.0, 0.0, 1.0]
                );
            }
        }
        assert_eq!(function4_count, 2);
        assert_eq!(mask_functions.len(), 1);
        assert_eq!(rgb_functions.len(), 1);
        assert_eq!(type1_shading_count, 2);
        assert!(
            unit_form_bboxes >= 2,
            "outer field and luminosity-mask Forms use unit BBoxes"
        );
        let mask_code = &mask_functions[0];
        let rgb_code = &rgb_functions[0];
        assert_eq!(sha256(rgb_code), expected_rgb_fn_sha);
        assert_eq!(sha256(mask_code), expected_mask_fn_sha);

        let v9_case_bounds = type4["bounds_f64_exact_union_of_f32_records"]
            .as_array()
            .unwrap();
        let bounds = [
            json_f64(&v9_case_bounds[0]),
            json_f64(&v9_case_bounds[1]),
            json_f64(&v9_case_bounds[2]),
            json_f64(&v9_case_bounds[3]),
        ];
        let upload = &case["native_upload_checkpoint"];
        assert_eq!(upload["vector_sha256"], V8_UPLOAD_SHA);
        let upload_bytes = decode_hex_bytes(upload["vector_hex"].as_str().unwrap());
        assert_eq!(upload_bytes.len(), 76 * 7 * 4);
        assert_eq!(sha256(&upload_bytes), V8_UPLOAD_SHA);
        let records = upload_bytes
            .as_chunks::<28>()
            .0
            .iter()
            .map(|bytes| {
                std::array::from_fn(|lane| {
                    f32::from_bits(u32::from_le_bytes(
                        bytes[lane * 4..lane * 4 + 4].try_into().unwrap(),
                    ))
                })
            })
            .collect::<Vec<[f32; 7]>>();
        assert_eq!(records.len(), 76);
        let uniform_alpha = case["drawable_color_uniforms"][0]["values"][3]
            .as_f64()
            .unwrap() as f32;
        let expected_alpha = if expected_name == "alpha128" {
            128.0 / 255.0
        } else {
            1.0
        };
        assert_eq!(uniform_alpha.to_bits(), (expected_alpha as f32).to_bits());
        for channel in 0..3 {
            assert_eq!(
                case["drawable_color_uniforms"][0]["values"][channel].as_f64(),
                Some(0.0)
            );
        }
        assert_eq!(case["drawable_color_uniforms"][0]["program"], 6);
        assert_eq!(case["drawable_color_uniforms"][0]["location"], 1);
        assert_eq!(
            case["drawable_color_uniforms"][0]["program_create_generation"],
            1
        );
        assert_eq!(
            case["drawable_color_uniforms"][0]["program_link_generation"],
            1
        );
        assert_eq!(case["drawable_color_uniforms"][0]["program_link_status"], 1);
        assert_eq!(
            case["drawable_color_uniforms"][0]["matched_draw_sequences"]
                .as_array()
                .unwrap()
                .len(),
            1
        );

        let mut radial_records = Vec::with_capacity(records.len());
        for (index, lanes) in records.iter().enumerate() {
            let [cx, cy, rx, ry, transparency, edge, width_alpha] = *lanes;
            assert_eq!(transparency.to_bits(), 1.0_f32.to_bits());
            assert_eq!(
                f32::from_bits(u32::from_le_bytes(
                    decode_hex_bytes(case["native_rtv5_s3_f32_bits"][index].as_str().unwrap(),)
                        .try_into()
                        .unwrap(),
                ))
                .to_bits(),
                transparency.to_bits()
            );
            radial_records.push([
                cx,
                cy,
                rx,
                ry,
                edge,
                transparency * width_alpha,
                0.0,
                0.0,
                0.0,
            ]);
        }
        assert_eq!(type4["record_sha256"], {
            let bytes = radial_records
                .iter()
                .flat_map(|record| record.iter())
                .flat_map(|value| value.to_bits().to_le_bytes())
                .collect::<Vec<_>>();
            sha256(&bytes)
        });
        let computed_bounds = [
            radial_records
                .iter()
                .map(|r| f64::from(r[0]) - f64::from(r[2]))
                .fold(f64::INFINITY, f64::min),
            radial_records
                .iter()
                .map(|r| f64::from(r[1]) - f64::from(r[3]))
                .fold(f64::INFINITY, f64::min),
            radial_records
                .iter()
                .map(|r| f64::from(r[0]) + f64::from(r[2]))
                .fold(f64::NEG_INFINITY, f64::max),
            radial_records
                .iter()
                .map(|r| f64::from(r[1]) + f64::from(r[3]))
                .fold(f64::NEG_INFINITY, f64::max),
        ];
        assert_eq!(bounds, computed_bounds);
        let normalized = normalized_records(&radial_records, bounds);

        let pixel_artifact = pair["pixel_artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|artifact| artifact["key"] == case["pixel_artifact_key"])
            .unwrap();
        let artifact_path = Path::new(pixel_artifact["path"].as_str().unwrap());
        let artifact_path = if artifact_path.is_absolute() {
            artifact_path.to_path_buf()
        } else {
            repo.join(artifact_path)
        };
        let compressed = fs::read(artifact_path).unwrap();
        assert_eq!(
            sha256(&compressed),
            pixel_artifact["compressed_sha256"].as_str().unwrap()
        );
        let mut decoder = ZlibDecoder::new(compressed.as_slice());
        let mut native_lower_left = Vec::new();
        decoder.read_to_end(&mut native_lower_left).unwrap();
        assert_eq!(native_lower_left.len(), WIDTH * HEIGHT * 4);
        assert_eq!(
            sha256(&native_lower_left),
            pixel_artifact["raw_sha256"].as_str().unwrap()
        );
        let stride = WIDTH * 4;
        let mut native = vec![0_u8; native_lower_left.len()];
        for row in 0..HEIGHT {
            native[row * stride..(row + 1) * stride].copy_from_slice(
                &native_lower_left[(HEIGHT - 1 - row) * stride..(HEIGHT - row) * stride],
            );
        }
        let mut xmin = WIDTH;
        let mut ymin = HEIGHT;
        let mut xmax = 0;
        let mut ymax = 0;
        for row in 0..HEIGHT {
            for column in 0..WIDTH {
                if native[(row * WIDTH + column) * 4 + 3] != 0 {
                    xmin = xmin.min(column);
                    ymin = ymin.min(row);
                    xmax = xmax.max(column);
                    ymax = ymax.max(row);
                }
            }
        }
        assert_eq!([xmin, ymin, xmax, ymax], [115, 850, 133, 874]);

        let matrix_rows = case["renderer_gl_calls"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| {
                event["call"] == "glUniformMatrix4fv"
                    && event["program"] == 6
                    && event["location"] == 0
            })
            .collect::<Vec<_>>();
        assert_eq!(matrix_rows.len(), 1);
        let matrix = matrix_rows[0]["values"]
            .as_array()
            .unwrap()
            .iter()
            .map(json_f64)
            .collect::<Vec<_>>();
        let viewport = case["renderer_draw_context"]["viewport_at_readback"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_u64().unwrap() as u32)
            .collect::<Vec<_>>();
        assert_eq!(viewport, vec![0, 0, WIDTH as u32, HEIGHT as u32]);
        let affine = native_to_viewport_affine(&matrix, &viewport);
        let x_start = xmin.saturating_sub(1);
        let y_start = ymin.saturating_sub(1);
        let x_end = (xmax + 1).min(WIDTH - 1);
        let y_end = (ymax + 1).min(HEIGHT - 1);
        let mut marker_ctms = Vec::new();
        for content_id in pdf.get_page_contents(page_id) {
            let stream = pdf.get_object(content_id).unwrap().as_stream().unwrap();
            let content_bytes = stream.decompressed_content().unwrap();
            let decoded = lopdf::content::Content::decode(&content_bytes).unwrap();
            for operation in decoded.operations {
                if operation.operator == "cm" {
                    let values = operation
                        .operands
                        .iter()
                        .map(pdf_number)
                        .collect::<Vec<_>>();
                    if values.len() == 6 {
                        marker_ctms.push(values);
                    }
                }
            }
        }
        assert_eq!(marker_ctms.len(), 1, "one source field marker transform");
        let expected_ctm = [
            (bounds[2] - bounds[0]) * PDF_POINT_PER_PIXEL,
            0.0,
            0.0,
            -(bounds[3] - bounds[1]) * PDF_POINT_PER_PIXEL,
            bounds[0] * PDF_POINT_PER_PIXEL,
            bounds[3] * PDF_POINT_PER_PIXEL,
        ];
        let marker_ctm_max_abs_serialization_residual = marker_ctms[0]
            .iter()
            .zip(expected_ctm)
            .map(|(actual, expected)| (*actual - expected).abs())
            .fold(0.0_f64, f64::max);
        let serialized_marker_ctm: [f64; 6] = marker_ctms[0].as_slice().try_into().unwrap();
        for (actual, expected) in marker_ctms[0].iter().zip(expected_ctm) {
            assert!(
                (*actual - expected).abs() < 5.0e-5,
                "PDF marker CTM {actual} != {expected}"
            );
        }

        let mut phase_receipts = Vec::new();
        for (phase_name, phase_offset) in [("pixel_center_0_5", 0.5), ("integer_device_0_0", 0.0)] {
            let mut calculator_formula_max = 0.0_f64;
            let mut calculator_formula_sum = 0.0_f64;
            let mut ideal_native_sum = 0.0_f64;
            let mut emitted_native_sum = 0.0_f64;
            let mut emitted_unorm_native_sum = 0.0_f64;
            let mut ideal_intersection = 0_usize;
            let mut ideal_union = 0_usize;
            let mut emitted_intersection = 0_usize;
            let mut emitted_union = 0_usize;
            let mut ideal_support = 0_usize;
            let mut emitted_support = 0_usize;
            let mut native_support = 0_usize;
            let mut emitted_max_byte_error = 0.0_f64;
            let mut ideal_max_byte_error = 0.0_f64;
            let mut pixel_count = 0_usize;
            for row in y_start..=y_end {
                for column in x_start..=x_end {
                    // Both phase choices are explicit diagnostics. The
                    // installed PDFium 126 wheel's exact sampling rule is
                    // not asserted here. First map through the captured
                    // native affine; separately map through serialized cm.
                    let pdf_x = (column as f64 + phase_offset) * PDF_POINT_PER_PIXEL;
                    let pdf_y_bottom =
                        (HEIGHT as f64 - row as f64 - phase_offset) * PDF_POINT_PER_PIXEL;
                    let svg_x = pdf_x * PDF_RENDER_SCALE;
                    let svg_y = (PAGE_HEIGHT_POINTS - pdf_y_bottom) * PDF_RENDER_SCALE;
                    let (source_x, source_y) = inverse_affine(affine, svg_x, svg_y);
                    let ideal_u = (source_x - bounds[0]) / (bounds[2] - bounds[0]);
                    let ideal_v = 1.0 - (source_y - bounds[1]) / (bounds[3] - bounds[1]);
                    let ideal = direct_source_over_alpha(
                        &normalized,
                        f64::from(uniform_alpha),
                        ideal_u,
                        ideal_v,
                    );

                    // The emitted function sees coordinates after the
                    // actual serialized PDF cm, including its f32 rounding.
                    let (emitted_u, emitted_v) =
                        inverse_pdf_ctm(serialized_marker_ctm, pdf_x, pdf_y_bottom);
                    let (mask, stack) = run_calculator(mask_code, emitted_u, emitted_v);
                    assert_eq!(mask.len(), 1);
                    assert!(stack <= 8);
                    let emitted_formula = direct_source_over_alpha(
                        &normalized,
                        f64::from(uniform_alpha),
                        emitted_u,
                        emitted_v,
                    );
                    calculator_formula_max =
                        calculator_formula_max.max((mask[0] - emitted_formula).abs());
                    calculator_formula_sum += (mask[0] - emitted_formula).abs();
                    let emitted_unorm = direct_source_over_alpha_unorm8(
                        &normalized,
                        f64::from(uniform_alpha),
                        emitted_u,
                        emitted_v,
                    );
                    let actual = f64::from(native[(row * WIDTH + column) * 4 + 3]);
                    let ideal_error = (ideal * 255.0 - actual).abs();
                    let emitted_error = (mask[0] * 255.0 - actual).abs();
                    ideal_native_sum += ideal_error;
                    emitted_native_sum += emitted_error;
                    emitted_unorm_native_sum += (emitted_unorm * 255.0 - actual).abs();
                    ideal_max_byte_error = ideal_max_byte_error.max(ideal_error);
                    emitted_max_byte_error = emitted_max_byte_error.max(emitted_error);
                    let ideal_predicted = ideal > 0.0;
                    let emitted_predicted = mask[0] > 0.0;
                    let observed = actual > 0.0;
                    ideal_support += usize::from(ideal_predicted);
                    emitted_support += usize::from(emitted_predicted);
                    native_support += usize::from(observed);
                    ideal_intersection += usize::from(ideal_predicted && observed);
                    ideal_union += usize::from(ideal_predicted || observed);
                    emitted_intersection += usize::from(emitted_predicted && observed);
                    emitted_union += usize::from(emitted_predicted || observed);
                    pixel_count += 1;
                }
            }
            phase_receipts.push(serde_json::json!({
                "phase": phase_name,
                "phase_offset_device_coordinates": phase_offset,
                "sample_count": pixel_count,
                "roi_topdown_px": [x_start, y_start, x_end, y_end],
                "ideal_native_affine_formula_vs_native_alpha_mae_bytes_expanded_roi": ideal_native_sum / pixel_count as f64,
                "ideal_native_affine_formula_vs_native_alpha_mae_bytes_support_union": ideal_native_sum / ideal_union.max(1) as f64,
                "ideal_native_affine_formula_vs_native_max_abs_alpha_byte_error": ideal_max_byte_error,
                "ideal_support_pixels": ideal_support,
                "native_support_pixels_in_roi": native_support,
                "ideal_support_iou_in_roi": ideal_intersection as f64 / ideal_union.max(1) as f64,
                "emitted_pdf_ctm_function_vs_native_alpha_mae_bytes_expanded_roi": emitted_native_sum / pixel_count as f64,
                "emitted_pdf_ctm_function_vs_native_alpha_mae_bytes_support_union": emitted_native_sum / emitted_union.max(1) as f64,
                "emitted_pdf_ctm_function_vs_native_max_abs_alpha_byte_error": emitted_max_byte_error,
                "emitted_pdf_ctm_function_support_pixels": emitted_support,
                "emitted_pdf_ctm_function_support_iou_in_roi": emitted_intersection as f64 / emitted_union.max(1) as f64,
                "emitted_pdf_ctm_calculator_vs_formula_max_abs": calculator_formula_max,
                "emitted_pdf_ctm_calculator_vs_formula_mean_abs": calculator_formula_sum / pixel_count as f64,
                "per_draw_unorm8_half_up_q_vs_native_alpha_mae_bytes_diagnostic_only": emitted_unorm_native_sum / pixel_count as f64,
                "unorm8_tie_rule_proven_for_gpu": false
            }));
            assert!(calculator_formula_max < 1.0e-11);
        }

        let reader_rows = reader_report["rows"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| {
                row["case"] == expected_name
                    && row["variant"] == "type4"
                    && (row["engine"] == "mupdf" || row["engine"] == "pdfium")
            })
            .collect::<Vec<_>>();
        assert_eq!(reader_rows.len(), 2);
        case_receipts.push(serde_json::json!({
            "case": expected_name,
            "pdf_sha256": expected_pdf_sha,
            "rgb_function_type4_sha256": expected_rgb_fn_sha,
            "mask_function_type4_sha256": expected_mask_fn_sha,
            "page_media_box_points": media_box,
            "shading_type1_count": type1_shading_count,
            "unit_form_bbox_count": unit_form_bboxes,
            "marker_ctm_pdf_points": marker_ctms[0],
            "expected_marker_ctm_from_exact_record_bounds": expected_ctm,
            "marker_ctm_max_abs_serialization_residual_points": marker_ctm_max_abs_serialization_residual,
            "native_upload_sha256": V8_UPLOAD_SHA,
            "captured_program6_fragment_source_sha256": V8_SELECTED_FRAGMENT_SHA,
            "captured_program6_fragment_source_precision": "mediump float",
            "captured_program6_fragment_source_contains_fwidth": false,
            "record_count": radial_records.len(),
            "record_sha256": type4["record_sha256"],
            "bounds": bounds,
            "captured_projection_matrix_f64": matrix,
            "viewport": viewport,
            "native_support_bbox_topdown_px": [xmin, ymin, xmax, ymax],
            "sample_roi_expanded_by_one_pixel": [x_start, y_start, x_end, y_end],
            "pixel_sample_phases": phase_receipts,
            "reader_rows_from_pinned_fixed96dpi_report": reader_rows,
            "no_phase_or_scale_fit": true
        }));
    }

    let receipt = serde_json::json!({
        "schema": "sdocx-fountain-v16-thin-type4-function-numeric-audit-v2",
        "scope": "offline numeric audit of emitted pinned Type 4 functions at fixed 96 DPI pixel-center and integer-device-coordinate diagnostic phases, compared with independent normalized-record direct source-over math and the actual retained native Mesa frame; no PDF rendering or native execution performed",
        "v8_pair_sha256": PAIR_SHA,
        "v9_manifest_sha256": V9_MANIFEST_SHA,
        "reader_report_sha256": READER_REPORT_SHA,
        "upload_vector_sha256": V8_UPLOAD_SHA,
        "captured_program6_fragment_source_sha256": V8_SELECTED_FRAGMENT_SHA,
        "pixel_grid": [WIDTH, HEIGHT],
        "page_points": [PAGE_WIDTH_POINTS, PAGE_HEIGHT_POINTS],
        "render_scale": PDF_RENDER_SCALE,
        "source_field_law": "d=sqrt(((u-cx)/(2*rx))^2+((v-cy)/(2*ry))^2); c=coverage for d<=edge, 0 for d>=.5, otherwise coverage*(.5-d)/(.5-edge); q recurrence uses a=c*saved_alpha and q=a+q*(1-a)",
        "diagnostic_limit": "The retained V8 GLV16 fragment source is pinned and contains no fwidth call. Mesa byte differences remain unexplained by the analytic field; mediump arithmetic, interpolation/raster precision, and the exact GPU UNORM8 tie rule are not established here. The integer-device-coordinate phase is a comparison only, not a claim about the installed PDFium 126 wheel's sampling rule.",
        "cases": case_receipts
    });
    let bytes = serde_json::to_vec_pretty(&receipt).unwrap();
    let output = output_dir.join("type4-function-numeric-audit.json");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .unwrap();
    std::io::Write::write_all(&mut file, &bytes).unwrap();
    file.sync_all().unwrap();
}
