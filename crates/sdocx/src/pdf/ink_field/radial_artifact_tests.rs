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

// Capture metadata beyond the retained VBO/R8 planes remains historical.
#[test]
fn native_r8_mask_and_quantized_programs_preserve_pdf_transport() {
    use base64::Engine as _;
    use std::io::Read;

    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../conformance/inkpen2-max-mask-native.json"
    ))
    .unwrap();
    assert_eq!(fixture["schema"], "sdocx-inkpen2-native-max-mask-v1");
    let producer = &fixture["producer"];
    let vbo = decode_hex(
        producer["native_AddPoint_vector_bytes_hex"]
            .as_str()
            .unwrap(),
    );
    assert_eq!(vbo.len(), 900);
    assert_eq!(producer["byte_count"], 900);
    assert_eq!(producer["record_count"], 25);
    let digest = sha256_hex(&vbo);
    assert_eq!(
        digest,
        "570044bb059690f865cfa6f4f99c8ba7801573144242ee0913654973b0d0e987"
    );
    assert_eq!(producer["sha256"].as_str(), Some(digest.as_str()));
    let captures = fixture["captures"].as_array().unwrap();
    assert_eq!(captures.len(), 6);
    assert_eq!(fixture["planes_by_scale"].as_object().unwrap().len(), 3);
    for scale in [1, 2, 4] {
        let plane = &fixture["planes_by_scale"][scale.to_string()];
        assert_eq!(plane["width"], 128);
        assert_eq!(plane["height"], 96);
        assert_eq!(plane["internal_format"], 33321);
        let compressed = base64::engine::general_purpose::STANDARD
            .decode(plane["red_zlib_base64"].as_str().unwrap())
            .unwrap();
        let mut native_m = Vec::new();
        flate2::read::ZlibDecoder::new(compressed.as_slice())
            .read_to_end(&mut native_m)
            .unwrap();
        assert_eq!(native_m.len(), 128 * 96);
        let rgba: Vec<u8> = native_m.iter().flat_map(|&r| [r, 0, 0, 255]).collect();
        let rgba_hash = sha256_hex(&rgba);
        let red_hash = sha256_hex(&native_m);
        assert_eq!(plane["rgba_sha256"].as_str(), Some(rgba_hash.as_str()));
        let pdf_mask = reverse_r8_rows(&native_m, 128, 96);
        assert_eq!(reverse_r8_rows(&pdf_mask, 128, 96), native_m);
        let records = gl_up_records_to_svg_down(&captured_radial_records(&vbo, scale as f32), 96.0);
        let bounds = radial_bounds(&records);
        let bounds_array = [bounds.x_min, bounds.y_min, bounds.x_max, bounds.y_max];
        for alpha in [128_u8, 255] {
            let id = format!("scale{scale}_aa1_alpha{alpha}");
            let matching: Vec<_> = captures
                .iter()
                .filter(|row| row["id"].as_str() == Some(id.as_str()))
                .collect();
            assert_eq!(matching.len(), 1);
            let capture = matching[0];
            assert_eq!(capture["scale"], scale);
            assert_eq!(
                capture["stage0_rgba_sha256"].as_str(),
                Some(rgba_hash.as_str())
            );
            assert_eq!(
                capture["stage0_red_sha256"].as_str(),
                Some(red_hash.as_str())
            );
            let saved_alpha = f32::from(alpha) / 255.0;
            let baseline = radial::compile(
                &records,
                bounds_array,
                saved_alpha,
                PdfRadialCoverage::EnhancedMax,
            )
            .unwrap();
            let quantized = radial::compile_per_draw_rgba8_for_test(
                &records,
                bounds_array,
                saved_alpha,
                PdfRadialCoverage::EnhancedMax,
            )
            .unwrap();
            assert_eq!(baseline.mask, quantized.mask);
            let marker = 0x4950_5141_0000_0001;
            let field = PdfInkField::radial(
                marker,
                &records,
                saved_alpha,
                PdfRadialCoverage::EnhancedMax,
                bounds,
            );
            let fields = [field];
            let source = marker_pdf_with_bounds(marker, false, bounds, 128.0, 96.0, saved_alpha);
            let baseline_pdf = replace_markers(&source, &fields).unwrap();
            let quantized_pdf = replace_markers_per_draw_rgba8_for_test(&source, &fields).unwrap();
            let native_pdf = replace_markers_with_mask_for_test(
                &source,
                &fields,
                RadialMaskOverride::NativeGrayImage(pdf_mask.clone()),
            )
            .unwrap();
            let opaque_pdf =
                replace_markers_with_mask_for_test(&source, &fields, RadialMaskOverride::Opaque)
                    .unwrap();
            for (bytes, expected_images) in [
                (&baseline_pdf, 0),
                (&quantized_pdf, 0),
                (&native_pdf, 1),
                (&opaque_pdf, 0),
            ] {
                let document = Document::load_mem(bytes).unwrap();
                assert_eq!(document.get_pages().len(), 1);
                let images: Vec<_> = document
                    .objects
                    .values()
                    .filter_map(|object| object.as_stream().ok())
                    .filter(|stream| {
                        stream
                            .dict
                            .get(b"Subtype")
                            .and_then(Object::as_name)
                            .is_ok_and(|name| name == b"Image")
                    })
                    .collect();
                assert_eq!(images.len(), expected_images);
                if expected_images == 1 {
                    native_mask_form_ids(&document);
                    let image = images[0];
                    assert_eq!(
                        image.dict.get(b"ColorSpace").unwrap().as_name().unwrap(),
                        b"DeviceGray"
                    );
                    assert_eq!(
                        image
                            .dict
                            .get(b"BitsPerComponent")
                            .unwrap()
                            .as_i64()
                            .unwrap(),
                        8
                    );
                    let decoded = image.decompressed_content().unwrap();
                    assert_eq!(decoded, pdf_mask);
                }
            }
        }
    }
}
