use super::*;

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

fn decode_hex(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0, "hex data has odd length");
    value
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let pair = std::str::from_utf8(pair).unwrap();
            u8::from_str_radix(pair, 16).unwrap()
        })
        .collect()
}

fn captured_radial_records(bytes: &[u8], scale: f32) -> Vec<PdfRadialRecord> {
    assert_eq!(bytes.len(), 25 * 9 * 4, "unexpected captured VBO size");
    let records = bytes
        .as_chunks::<36>()
        .0
        .iter()
        .map(|record| {
            let mut values = [0.0_f32; 9];
            for (index, value) in values.iter_mut().enumerate() {
                let start = index * 4;
                *value = f32::from_le_bytes(record[start..start + 4].try_into().unwrap());
            }
            values[0] *= scale;
            values[1] *= scale;
            values[2] *= scale;
            values[3] *= scale;
            values
        })
        .collect::<Vec<_>>();
    assert!(records.iter().flatten().all(|value| value.is_finite()));
    records
}

fn radial_bounds(records: &[PdfRadialRecord]) -> BoundingBox {
    let (x_min, y_min, x_max, y_max) = records.iter().fold(
        (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ),
        |(x_min, y_min, x_max, y_max), record| {
            let cx = f64::from(record[0]);
            let cy = f64::from(record[1]);
            let rx = f64::from(record[2]);
            let ry = f64::from(record[3]);
            (
                x_min.min(cx - rx),
                y_min.min(cy - ry),
                x_max.max(cx + rx),
                y_max.max(cy + ry),
            )
        },
    );
    BoundingBox {
        x_min: x_min - 0.001,
        y_min: y_min - 0.001,
        x_max: x_max + 0.001,
        y_max: y_max + 0.001,
    }
}

fn expand_source_bounds_by_page_points(bounds: BoundingBox, padding: f64) -> BoundingBox {
    BoundingBox {
        x_min: bounds.x_min - padding,
        y_min: bounds.y_min - padding,
        x_max: bounds.x_max + padding,
        y_max: bounds.y_max + padding,
    }
}

fn gl_up_records_to_svg_down(
    records: &[PdfRadialRecord],
    page_height: f32,
) -> Vec<PdfRadialRecord> {
    records
        .iter()
        .map(|record| {
            let mut converted = *record;
            converted[1] = page_height - record[1];
            converted
        })
        .collect()
}

#[test]
fn captured_gl_y_conversion_uses_full_page_height() {
    let records = [
        [12.0, 7.0, 2.0, 3.0, 0.1, 1.0, 0.2, 0.4, 0.6],
        [18.0, 13.0, 1.0, 2.0, 0.1, 1.0, 0.6, 0.4, 0.2],
    ];
    let local_bounds = radial_bounds(&records);
    assert_ne!(local_bounds.y_min + local_bounds.y_max, 96.0);
    let converted = gl_up_records_to_svg_down(&records, 96.0);
    assert_eq!(converted[0][1], 89.0);
    assert_eq!(converted[1][1], 83.0);
    assert_ne!(
        converted[0][1],
        (local_bounds.y_min + local_bounds.y_max) as f32 - 7.0
    );
    let converted_bounds = radial_bounds(&converted);
    assert!(converted_bounds.y_min > 80.0 && converted_bounds.y_max < 96.0);
}

#[test]
fn native_mask_page_to_field_matrix_handles_translated_asymmetric_bounds() {
    let bounds = BoundingBox {
        x_min: 11.125,
        y_min: 68.25,
        x_max: 24.875,
        y_max: 83.75,
    };
    let [a, b, c, d, e, f] = native_mask_image_to_field_matrix(bounds).unwrap();
    assert_ne!(e, 0.0);
    assert_ne!(f, 0.0);
    for (page_x, page_y_down) in [(13.5, 70.25), (18.75, 76.5), (22.25, 81.125)] {
        // PDF image coordinates have their origin at the lower left;
        // captured source records use page y-down coordinates.
        let image_x = page_x / 128.0;
        let image_y = 1.0 - page_y_down / 96.0;
        let mapped_x = a * image_x + c * image_y + e;
        let mapped_y = b * image_x + d * image_y + f;
        let source_x = (page_x - bounds.x_min) / (bounds.x_max - bounds.x_min);
        let source_y = 1.0 - (page_y_down - bounds.y_min) / (bounds.y_max - bounds.y_min);
        assert!((mapped_x - source_x).abs() < 1e-12);
        assert!((mapped_y - source_y).abs() < 1e-12);
    }
}

#[test]
fn expanded_source_bounds_preserve_page_geometry_after_inverse_mapping() {
    let bounds = BoundingBox {
        x_min: 13.125,
        y_min: 61.75,
        x_max: 32.875,
        y_max: 82.25,
    };
    let expanded = expand_source_bounds_by_page_points(bounds, 1.0);
    let record = [20.375_f32, 73.125, 2.625, 3.375, 0.125, 0.8, 0.2, 0.4, 0.6];
    let normalize = |bounds: BoundingBox| {
        let width = bounds.x_max - bounds.x_min;
        let height = bounds.y_max - bounds.y_min;
        [
            (f64::from(record[0]) - bounds.x_min) / width,
            1.0 - (f64::from(record[1]) - bounds.y_min) / height,
            f64::from(record[2]) / width,
            f64::from(record[3]) / height,
        ]
    };
    let inverse = |bounds: BoundingBox, normalized: [f64; 4]| {
        let width = bounds.x_max - bounds.x_min;
        let height = bounds.y_max - bounds.y_min;
        [
            bounds.x_min + normalized[0] * width,
            bounds.y_min + (1.0 - normalized[1]) * height,
            normalized[2] * width,
            normalized[3] * height,
        ]
    };
    let old_page_geometry = inverse(bounds, normalize(bounds));
    let expanded_page_geometry = inverse(expanded, normalize(expanded));
    for (old, expanded) in old_page_geometry.into_iter().zip(expanded_page_geometry) {
        assert!((old - expanded).abs() < 1e-12);
    }
    assert_ne!(normalize(bounds), normalize(expanded));
}

#[test]
fn native_mask_pdf_rows_reverse_captured_gl_rows_once() {
    let gl_bottom_up = [1, 2, 3, 4, 5, 6];
    assert_eq!(reverse_r8_rows(&gl_bottom_up, 3, 2), [4, 5, 6, 1, 2, 3]);
}

fn write_new_file(path: &std::path::Path, bytes: &[u8]) {
    use std::{fs::OpenOptions, io::Write};

    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    file.write_all(bytes).unwrap();
}

fn reverse_r8_rows(samples: &[u8], width: usize, height: usize) -> Vec<u8> {
    assert_eq!(samples.len(), width * height);
    let mut output = Vec::with_capacity(samples.len());
    for row in (0..height).rev() {
        let start = row * width;
        output.extend_from_slice(&samples[start..start + width]);
    }
    output
}

fn native_mask_form_ids(document: &Document) -> (ObjectId, ObjectId) {
    let images: Vec<_> = document
        .objects
        .iter()
        .filter_map(|(id, object)| {
            let stream = object.as_stream().ok()?;
            (stream.dict.get(b"Subtype").ok()?.as_name().ok()? == b"Image"
                && stream.dict.get(b"Width").ok()?.as_i64().ok()? == 128
                && stream.dict.get(b"Height").ok()?.as_i64().ok()? == 96)
                .then_some(*id)
        })
        .collect();
    assert_eq!(images.len(), 1, "one native M image expected");
    let image_id = images[0];
    let masks: Vec<_> = document
        .objects
        .iter()
        .filter_map(|(id, object)| {
            let stream = object.as_stream().ok()?;
            if stream.dict.get(b"Subtype").ok()?.as_name().ok()? != b"Form" {
                return None;
            }
            let resources = stream.dict.get(b"Resources").ok()?.as_dict().ok()?;
            let xobjects = resources.get(b"XObject").ok()?.as_dict().ok()?;
            (xobjects.get(b"M").ok()?.as_reference().ok()? == image_id).then_some(*id)
        })
        .collect();
    assert_eq!(masks.len(), 1, "one mask Form must own the native M image");
    let mask_id = masks[0];
    let outers: Vec<_> = document
        .objects
        .iter()
        .filter_map(|(id, object)| {
            let stream = object.as_stream().ok()?;
            if stream.dict.get(b"Subtype").ok()?.as_name().ok()? != b"Form" {
                return None;
            }
            let resources = stream.dict.get(b"Resources").ok()?.as_dict().ok()?;
            let states = resources.get(b"ExtGState").ok()?.as_dict().ok()?;
            states
                .iter()
                .any(|(_, state_ref)| {
                    let Ok(state_id) = state_ref.as_reference() else {
                        return false;
                    };
                    let Ok(state) = document.get_object(state_id).and_then(Object::as_dict) else {
                        return false;
                    };
                    let Ok(soft_mask) = state.get(b"SMask").and_then(Object::as_dict) else {
                        return false;
                    };
                    soft_mask.get(b"G").and_then(Object::as_reference).ok() == Some(mask_id)
                })
                .then_some(*id)
        })
        .collect();
    assert_eq!(
        outers.len(),
        1,
        "one outer Form must reference the M mask Form"
    );
    (mask_id, outers[0])
}

fn expand_native_mask_bboxes_for_test(
    bytes: &[u8],
    bounds: BoundingBox,
    expand_mask: bool,
    expand_outer: bool,
) -> Vec<u8> {
    assert!(expand_mask || expand_outer);
    let mut document = Document::load_mem(bytes).unwrap();
    let (mask_id, outer_id) = native_mask_form_ids(&document);
    let dx = 1.0 / (bounds.x_max - bounds.x_min);
    let dy = 1.0 / (bounds.y_max - bounds.y_min);
    for (object_id, expand) in [(mask_id, expand_mask), (outer_id, expand_outer)] {
        if !expand {
            continue;
        }
        let stream = document
            .get_object_mut(object_id)
            .unwrap()
            .as_stream_mut()
            .unwrap();
        let bbox = stream
            .dict
            .get_mut(b"BBox")
            .unwrap()
            .as_array_mut()
            .unwrap();
        assert_eq!(bbox.len(), 4);
        assert_eq!(bbox[0].as_float().unwrap(), 0.0);
        assert_eq!(bbox[1].as_float().unwrap(), 0.0);
        assert_eq!(bbox[2].as_float().unwrap(), 1.0);
        assert_eq!(bbox[3].as_float().unwrap(), 1.0);
        *bbox = vec![
            Object::Real(-dx as f32),
            Object::Real(-dy as f32),
            Object::Real((1.0 + dx) as f32),
            Object::Real((1.0 + dy) as f32),
        ];
    }
    let mut output = Vec::new();
    document.save_to(&mut output).unwrap();
    output
}

fn replace_native_mask_outer_shading_with_fill_for_test(bytes: &[u8]) -> Vec<u8> {
    let mut document = Document::load_mem(bytes).unwrap();
    let (_, outer_id) = native_mask_form_ids(&document);
    let stream = document
        .get_object_mut(outer_id)
        .unwrap()
        .as_stream_mut()
        .unwrap();
    stream.decompress().unwrap();
    assert_eq!(stream.content, b"/GS gs\n/C sh");
    stream.set_content(b"/GS gs\n0 0 0 rg\n0 0 1 1 re f".to_vec());
    stream.compress().unwrap();
    let mut output = Vec::new();
    document.save_to(&mut output).unwrap();
    output
}

/// Opt-in artifact writer consumed by the paired MuPDF/PDFium reader
/// harness. It routes the paired programs through production marker/Form
/// plumbing and emits separate mask-only diagnostic controls.
#[test]
#[ignore = "writes paired experiment PDFs when SDOCX_INKPEN2_QUANT_ARTIFACT_DIR is set"]
fn write_inkpen2_quantization_pdf_pairs() {
    use serde_json::{Value, json};
    use std::path::PathBuf;

    assert_eq!(
        sha256_hex(include_bytes!("radial_artifact_tests.rs")),
        super::RADIAL_ARTIFACT_TEST_MODULE_SHA256,
        "artifact writer source differs from the pinned test-module provenance"
    );

    let output_dir = std::env::var_os("SDOCX_INKPEN2_QUANT_ARTIFACT_DIR")
        .expect("set SDOCX_INKPEN2_QUANT_ARTIFACT_DIR to an output directory");
    let output_dir = PathBuf::from(output_dir);
    std::fs::create_dir_all(&output_dir).unwrap();
    assert!(
        std::fs::read_dir(&output_dir).unwrap().next().is_none(),
        "artifact output directory must be empty; use a fresh path"
    );
    let capture_dir = std::env::var_os("SDOCX_INKPEN2_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join("tmp/vector-parity-plan/inkpen2-rainbow-paint/captures")
        });
    let cases = [
        ("scale1_aa1_alpha128", 1.0_f32, 128_u8),
        ("scale1_aa1_alpha255", 1.0, 255),
        ("scale2_aa1_alpha128", 2.0, 128),
        ("scale2_aa1_alpha255", 2.0, 255),
        ("scale4_aa1_alpha128", 4.0, 128),
        ("scale4_aa1_alpha255", 4.0, 255),
    ];
    let mut manifest_cases = Vec::new();
    for (case_id, scale, alpha_byte) in cases {
        let capture_path = capture_dir.join(format!("{case_id}.json"));
        let capture_bytes = std::fs::read(&capture_path)
            .unwrap_or_else(|error| panic!("{}: {error}", capture_path.display()));
        let capture: Value = serde_json::from_slice(&capture_bytes).unwrap();
        let producer = &capture["joined_native_paint"]["producer"];
        let vbo = decode_hex(
            producer["native_AddPoint_vector_bytes_hex"]
                .as_str()
                .expect("captured VBO bytes"),
        );
        let vbo_sha256 = sha256_hex(&vbo);
        assert_eq!(vbo.len(), 25 * 9 * 4);
        assert_eq!(
            vbo_sha256,
            "570044bb059690f865cfa6f4f99c8ba7801573144242ee0913654973b0d0e987"
        );
        assert_eq!(producer["sha256"].as_str(), Some(vbo_sha256.as_str()));
        assert_eq!(
            capture["input"]["color_argb_u32"].as_u64().unwrap() >> 24,
            u64::from(alpha_byte)
        );
        let target =
            &capture["joined_native_paint"]["paint"]["readback"]["attachment"]["texture_level0"];
        assert_eq!(target["width"].as_u64(), Some(128));
        assert_eq!(target["height"].as_u64(), Some(96));
        assert_eq!(target["internal_format"].as_u64(), Some(32856));
        let stages = capture["joined_native_paint"]["native_stage_readbacks_before_invalidate"]
            .as_array()
            .expect("native M/C/final readbacks");
        assert_eq!(stages.len(), 3);
        let native_stage_rgba = decode_hex(
            stages[0]["rgba_bytes_hex"]
                .as_str()
                .expect("native R8 stage readback bytes"),
        );
        assert_eq!(native_stage_rgba.len(), 128 * 96 * 4);
        let stage_rgba_hash = sha256_hex(&native_stage_rgba);
        assert_eq!(
            Some(stage_rgba_hash.as_str()),
            stages[0]["rgba_sha256"].as_str()
        );
        let mut native_m = Vec::with_capacity(128 * 96);
        for pixel in native_stage_rgba.as_chunks::<4>().0 {
            assert_eq!(&pixel[1..], &[0, 0, 255], "R8 M readback expansion");
            native_m.push(pixel[0]);
        }
        let native_m_sha256 = sha256_hex(&native_m);
        let pdf_mask = reverse_r8_rows(&native_m, 128, 96);
        let raw_mask_sha256 = sha256_hex(&pdf_mask);
        let expected_formats = [33321_u64, 32856, 32856];
        let mut stage_textures = std::collections::HashSet::new();
        let mut stage_hashes = Vec::new();
        for (stage, expected_format) in stages.iter().zip(expected_formats) {
            let texture = stage["attachment_texture"]
                .as_u64()
                .expect("stage attachment texture id");
            assert!(stage_textures.insert(texture));
            assert_eq!(stage["texture_level0"]["width"].as_u64(), Some(128));
            assert_eq!(stage["texture_level0"]["height"].as_u64(), Some(96));
            assert_eq!(
                stage["texture_level0"]["internal_format"].as_u64(),
                Some(expected_format)
            );
            stage_hashes.push(
                stage["rgba_sha256"]
                    .as_str()
                    .expect("stage readback hash")
                    .to_owned(),
            );
        }
        assert_eq!(stage_textures.len(), 3);

        let gl_records = captured_radial_records(&vbo, scale);
        // Captures are GL-up. The production radial compiler consumes
        // page y-down source records; reflect against the full captured
        // target, then compute the marker bounds in that coordinate space.
        let records = gl_up_records_to_svg_down(&gl_records, 96.0);
        let bounds = radial_bounds(&records);
        let saved_alpha = f32::from(alpha_byte) / 255.0;
        let marker_id = 0x4950_5141_0000_0001;
        let baseline_field = PdfInkField::radial(
            marker_id,
            &records,
            saved_alpha,
            PdfRadialCoverage::EnhancedMax,
            bounds,
        );
        let candidate_field = PdfInkField::radial(
            marker_id,
            &records,
            saved_alpha,
            PdfRadialCoverage::EnhancedMax,
            bounds,
        );
        let bounds_array = [bounds.x_min, bounds.y_min, bounds.x_max, bounds.y_max];
        let baseline_program = radial::compile(
            &records,
            bounds_array,
            saved_alpha,
            PdfRadialCoverage::EnhancedMax,
        )
        .unwrap();
        let candidate_program = radial::compile_per_draw_rgba8_for_test(
            &records,
            bounds_array,
            saved_alpha,
            PdfRadialCoverage::EnhancedMax,
        )
        .unwrap();
        assert_eq!(baseline_program.mask, candidate_program.mask);

        let source_pdf = marker_pdf_with_bounds(marker_id, false, bounds, 128.0, 96.0, saved_alpha);
        let baseline = replace_markers(&source_pdf, &[baseline_field]).unwrap();
        let candidate =
            replace_markers_per_draw_rgba8_for_test(&source_pdf, &[candidate_field]).unwrap();
        let expanded_bounds = expand_source_bounds_by_page_points(bounds, 1.0);
        let expanded_bounds_array = [
            expanded_bounds.x_min,
            expanded_bounds.y_min,
            expanded_bounds.x_max,
            expanded_bounds.y_max,
        ];
        let expanded_program = radial::compile(
            &records,
            expanded_bounds_array,
            saved_alpha,
            PdfRadialCoverage::EnhancedMax,
        )
        .unwrap();
        let expanded_source_pdf =
            marker_pdf_with_bounds(marker_id, false, expanded_bounds, 128.0, 96.0, saved_alpha);
        let expanded_vector_pdf = replace_markers(
            &expanded_source_pdf,
            &[PdfInkField::radial(
                marker_id,
                &records,
                saved_alpha,
                PdfRadialCoverage::EnhancedMax,
                expanded_bounds,
            )],
        )
        .unwrap();
        let baseline_name = format!("{case_id}.baseline.pdf");
        let candidate_name = format!("{case_id}.quantized.pdf");
        write_new_file(&output_dir.join(&baseline_name), &baseline);
        write_new_file(&output_dir.join(&candidate_name), &candidate);
        let expanded_bounds_name = format!("{case_id}.expanded-source-bounds.pdf");
        write_new_file(
            &output_dir.join(&expanded_bounds_name),
            &expanded_vector_pdf,
        );
        let native_mask_pdf = replace_markers_with_mask_for_test(
            &source_pdf,
            &[PdfInkField::radial(
                marker_id,
                &records,
                saved_alpha,
                PdfRadialCoverage::EnhancedMax,
                bounds,
            )],
            RadialMaskOverride::NativeGrayImage(pdf_mask),
        )
        .unwrap();
        let opaque_mask_pdf = replace_markers_with_mask_for_test(
            &source_pdf,
            &[PdfInkField::radial(
                marker_id,
                &records,
                saved_alpha,
                PdfRadialCoverage::EnhancedMax,
                bounds,
            )],
            RadialMaskOverride::Opaque,
        )
        .unwrap();
        let native_mask_name = format!("{case_id}.native-m-image.pdf");
        let opaque_mask_name = format!("{case_id}.opaque-mask.pdf");
        write_new_file(&output_dir.join(&native_mask_name), &native_mask_pdf);
        write_new_file(&output_dir.join(&opaque_mask_name), &opaque_mask_pdf);
        let mut bbox_controls = Vec::new();
        if scale >= 2.0 {
            let dx = 1.0 / (bounds.x_max - bounds.x_min);
            let dy = 1.0 / (bounds.y_max - bounds.y_min);
            for (suffix, expanded_form, expand_mask, expand_outer) in [
                ("mask-bbox-plus1", "mask", true, false),
                ("outer-bbox-plus1", "outer", false, true),
                ("both-bbox-plus1", "both", true, true),
            ] {
                let diagnostic_pdf = expand_native_mask_bboxes_for_test(
                    &native_mask_pdf,
                    bounds,
                    expand_mask,
                    expand_outer,
                );
                let name = format!("{case_id}.{suffix}.pdf");
                write_new_file(&output_dir.join(&name), &diagnostic_pdf);
                let expected_bbox = |expand: bool| {
                    if expand {
                        [
                            -dx as f32 as f64,
                            -dy as f32 as f64,
                            (1.0 + dx) as f32 as f64,
                            (1.0 + dy) as f32 as f64,
                        ]
                    } else {
                        [0.0, 0.0, 1.0, 1.0]
                    }
                };
                bbox_controls.push(json!({
                    "expanded_form": expanded_form,
                    "expansion_sides": "outward on all four sides",
                    "path": name,
                    "sha256": sha256_hex(&diagnostic_pdf),
                    "baseline_native_m_image_pdf": {
                        "path": native_mask_name.clone(),
                        "sha256": sha256_hex(&native_mask_pdf)
                    },
                    "mask_form_graph_path": "outer Form /Resources /ExtGState /GS -> state /SMask /G; mask Form /Resources /XObject /M -> Image",
                    "outer_form_graph_path": "marker-replaced radial Form /Resources /ExtGState /GS; its state /SMask /G references the selected mask Form",
                    "mask_bbox_before": [0, 0, 1, 1],
                    "mask_bbox_after": expected_bbox(expand_mask),
                    "outer_form_bbox_before": [0, 0, 1, 1],
                    "outer_form_bbox_after": expected_bbox(expand_outer),
                    "device_pixel_expansion": 1,
                    "mask_bbox_delta_form_units": if expand_mask { [dx, dy] } else { [0.0, 0.0] },
                    "outer_form_bbox_delta_form_units": if expand_outer { [dx, dy] } else { [0.0, 0.0] },
                    "image_unit_to_mask_form_unit_ctm": native_mask_image_to_field_matrix(bounds).unwrap(),
                    "raw_mask_sha256": raw_mask_sha256,
                    "rgb_program_sha256": sha256_hex(&baseline_program.rgb),
                    "outer_opacity": saved_alpha,
                    "page": [128, 96],
                    "image_count": 1
                }));
            }
        }
        let solid_outer_fill = if scale >= 2.0 {
            let solid_pdf = replace_native_mask_outer_shading_with_fill_for_test(&native_mask_pdf);
            let name = format!("{case_id}.solid-outer-fill.pdf");
            write_new_file(&output_dir.join(&name), &solid_pdf);
            Some(json!({
                "path": name,
                "sha256": sha256_hex(&solid_pdf),
                "baseline_native_m_image_pdf": {
                    "path": native_mask_name.clone(),
                    "sha256": sha256_hex(&native_mask_pdf)
                },
                "outer_content_before": "/GS gs\n/C sh",
                "outer_content_before_sha256": sha256_hex(b"/GS gs\n/C sh"),
                "outer_content_after": "/GS gs\n0 0 0 rg\n0 0 1 1 re f",
                "outer_content_after_sha256": sha256_hex(b"/GS gs\n0 0 0 rg\n0 0 1 1 re f"),
                "rgb_program_sha256": sha256_hex(&baseline_program.rgb),
                "raw_mask_sha256": raw_mask_sha256,
                "outer_opacity": saved_alpha,
                "page": [128, 96],
                "image_count": 1,
                "alpha_diagnostic_only": true,
                "rgb_comparable": false
            }))
        } else {
            None
        };
        let mut case_manifest = json!({
            "case_id": case_id,
            "scale": scale,
            "alpha": alpha_byte,
            "coverage": "EnhancedMax",
            "outer_opacity": saved_alpha,
            "source_y_conversion": "captured GL-up centers reflected about full 96px page height before marker bounds and compiler PDF flip",
            "native_capture_sha256": sha256_hex(&capture_bytes),
            "native_vbo_sha256": vbo_sha256,
            "native_stage_sha256": stage_hashes,
            "bounds": [bounds.x_min, bounds.y_min, bounds.x_max, bounds.y_max],
            "page": [128, 96],
            "pdfs": {
                "baseline": {"path": baseline_name, "sha256": sha256_hex(&baseline)},
                "quantized": {"path": candidate_name, "sha256": sha256_hex(&candidate)},
            },
            "program_sha256": {
                "baseline": {
                    "rgb": sha256_hex(&baseline_program.rgb),
                    "mask": sha256_hex(&baseline_program.mask),
                },
                "quantized": {
                    "rgb": sha256_hex(&candidate_program.rgb),
                    "mask": sha256_hex(&candidate_program.mask),
                },
            },
            "mask_controls": {
                "native_m_image": {
                    "path": native_mask_name,
                    "sha256": sha256_hex(&native_mask_pdf),
                    "native_m_sha256": native_m_sha256,
                    "raw_mask_sha256": raw_mask_sha256,
                    "raw_mask_orientation": "PDF top-down rows; captured GL bottom-up R8 rows reversed once",
                    "width": 128,
                    "height": 96,
                    "bits_per_component": 8,
                    "colorspace": "DeviceGray",
                    "interpolate": false,
                    "image_count": 1,
                    "page": [128, 96],
                    "outer_opacity": saved_alpha,
                    "rgb_program_sha256": sha256_hex(&baseline_program.rgb),
                    "image_unit_to_mask_form_unit_ctm": native_mask_image_to_field_matrix(bounds).unwrap(),
                },
                "opaque_mask": {
                    "path": opaque_mask_name,
                    "sha256": sha256_hex(&opaque_mask_pdf),
                    "mask_program_sha256": sha256_hex(b"{ 1 }"),
                    "image_count": 0,
                    "page": [128, 96],
                    "outer_opacity": saved_alpha,
                    "rgb_program_sha256": sha256_hex(&baseline_program.rgb),
                }
            }
        });
        case_manifest["expanded_source_bounds"] = json!({
            "path": expanded_bounds_name,
            "sha256": sha256_hex(&expanded_vector_pdf),
            "baseline_vector_pdf": {
                "path": baseline_name,
                "sha256": sha256_hex(&baseline)
            },
            "original_bounds_source_y_down": [bounds.x_min, bounds.y_min, bounds.x_max, bounds.y_max],
            "expanded_bounds_source_y_down": [
                expanded_bounds.x_min,
                expanded_bounds.y_min,
                expanded_bounds.x_max,
                expanded_bounds.y_max
            ],
            "expansion_source_y_down_page_points": [1, 1],
            "page_points_per_output_pixel": 1,
            "unchanged_records_sha256": vbo_sha256,
            "marker_page_rect_source_y_down_points": [
                expanded_bounds.x_min,
                expanded_bounds.y_min,
                expanded_bounds.x_max - expanded_bounds.x_min,
                expanded_bounds.y_max - expanded_bounds.y_min
            ],
            "marker_source_transform_xy": [1, 0, 0, 1, expanded_bounds.x_min, expanded_bounds.y_min],
            "form_bbox": [0, 0, 1, 1],
            "outer_opacity": saved_alpha,
            "page": [128, 96],
            "image_count": 0,
            "program_sha256": {
                "rgb": sha256_hex(&expanded_program.rgb),
                "mask": sha256_hex(&expanded_program.mask)
            },
            "baseline_program_sha256": {
                "rgb": sha256_hex(&baseline_program.rgb),
                "mask": sha256_hex(&baseline_program.mask)
            },
            "page_geometry_invariant": "same centers/radii in page coordinates; normalized under expanded bounds then mapped by the matching expanded marker rectangle",
            "reader_scope": "compare all 128x96 premultiplied RGBA samples; no phase fit"
        });
        if !bbox_controls.is_empty() {
            case_manifest["bbox_controls"] = json!(bbox_controls);
        }
        if let Some(solid_outer_fill) = solid_outer_fill {
            case_manifest["solid_outer_fill"] = solid_outer_fill;
        }
        manifest_cases.push(case_manifest);
    }
    let manifest = json!({
        "schema": "sdocx-inkpen2-radial-rgb-quant-pair-v1",
        "policy": "test-only half-up per-draw RGBA8 RGB recurrence; production remains continuous",
        "mask_diagnostics": "optional native DeviceGray image and opaque luminosity-mask controls; diagnostic only",
        "radial_source_sha256": sha256_hex(include_bytes!("radial.rs")),
        "writer_provenance": {
            "test_module_path": "crates/sdocx/src/pdf/ink_field/radial_artifact_tests.rs",
            "test_module_sha256": super::RADIAL_ARTIFACT_TEST_MODULE_SHA256,
            "test_filter": "pdf::ink_field::radial_artifact_tests::write_inkpen2_quantization_pdf_pairs",
            "command": "cargo test -p sdocx --features pdf --lib pdf::ink_field::radial_artifact_tests::write_inkpen2_quantization_pdf_pairs -- --ignored --exact"
        },
        "cases": manifest_cases,
    });
    write_new_file(
        &output_dir.join("manifest.json"),
        &serde_json::to_vec_pretty(&manifest).unwrap(),
    );
}
