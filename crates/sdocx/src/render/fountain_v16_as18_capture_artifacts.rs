//! Opt-in SVG/PDF artifacts for the pinned persisted AS18 Fountain GLV16
//! capture. The saved source arrays pass through the production V16
//! preparation and paint plan; captured native AddPoint upload records and the
//! native Mesa frame remain explicit external evidence.

use super::super::vector::{Group, Scene, Styled, Svg, Transform, ViewBox};
use super::{V16Plan, render};
use crate::{Color, Point, Stroke};
use flate2::read::ZlibDecoder;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;
const SOURCE_PAYLOAD_SHA256: &str =
    "c01022129e9a3a09ee95dc5935d5e177bd507e03d6d9dbd357a5278a64ab72b4";
const SOURCE_FRAME_SHA256: &str =
    "490fa32ab9d69416e4c48dec88b26ad14e362d7b08eb3e1b6a9ada47ce2980c2";
const NATIVE_VECTOR_SHA256: &str =
    "3829df249606e8a8c360780d91929ef21dda075a0c379a6c38b09f8caa9fa56a";
const NATIVE_FRAME_SHA256: &str =
    "318568f0f140349c7662e87f19bb05ee18757d8e17178764a4157242c2742de7";

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn write_new(path: PathBuf, bytes: &[u8]) {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .expect("artifact path must be new");
    file.write_all(bytes).expect("write artifact");
    file.sync_all().expect("sync artifact");
}

fn decode_f32_words(values: &serde_json::Value) -> Vec<f32> {
    values
        .as_array()
        .expect("captured f32 bit array")
        .iter()
        .map(|value| {
            f32::from_bits(
                u32::from_str_radix(value.as_str().expect("f32 word"), 16).expect("valid f32 word"),
            )
        })
        .collect()
}

fn source_stroke(baseline: &serde_json::Value, rainbow: bool) -> Stroke {
    let source = &baseline["cases"][0];
    let arrays = &source["source_arrays"];
    let points = decode_f32_words(&arrays["points_f32_bits"]);
    assert_eq!(points.len(), 204 * 2);
    let count = points.len() / 2;
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../../../conformance/fountain-v16.json"))
            .expect("V16 reference fixture parses");
    let mut stroke: Stroke =
        serde_json::from_value(fixture["stroke"].clone()).expect("V16 reference stroke");
    stroke.points = points
        .as_chunks::<2>()
        .0
        .iter()
        .map(|xy| Point {
            x: f64::from(xy[0]),
            y: f64::from(xy[1]),
        })
        .collect();
    stroke.pressures = decode_f32_words(&arrays["pressure_f32_bits"])
        .into_iter()
        .map(f64::from)
        .collect();
    stroke.tilts = decode_f32_words(&arrays["tilt_f32_bits"])
        .into_iter()
        .map(f64::from)
        .collect();
    stroke.orientations = decode_f32_words(&arrays["orientation_f32_bits"])
        .into_iter()
        .map(f64::from)
        .collect();
    stroke.timestamps = arrays["timestamps_i32"]
        .as_array()
        .expect("captured timestamp array")
        .iter()
        .map(|value| value.as_i64().expect("timestamp"))
        .collect();
    assert_eq!(stroke.pressures.len(), count);
    assert_eq!(stroke.tilts.len(), count);
    assert_eq!(stroke.orientations.len(), count);
    assert_eq!(stroke.timestamps.len(), count);
    stroke.color = Some(Color { r: 0, g: 0, b: 0 });
    stroke.pen_width = f32::from_bits(0x4111_de6a);
    let rendering = stroke.rendering.as_mut().expect("saved rendering metadata");
    rendering.advanced_settings = Some("18;0;100;".into());
    rendering.pen_name = Some("com.samsung.android.sdk.pen.pen.preload.FountainPen".into());
    rendering.tool_type_raw = 2;
    rendering.properties.rainbow_effect = rainbow;
    rendering.properties.fixed_width = false;
    rendering.properties.stylus_channels = true;
    rendering.properties.eraser = false;
    rendering.properties.fixed_opacity = false;
    rendering.properties.alpha_lock = false;
    rendering.style.color_argb = Some(0xff00_0000);
    rendering.style.color_type_raw = Some(1);
    rendering.style.pen_size = Some(stroke.pen_width);
    rendering.style.rainbow_distance = Some(37);
    rendering.style.rainbow_offset = Some(2.75);
    rendering.style.gradient_colors_argb = Some(vec![0xffff_0000, 0xff00_ff00, 0xff00_00ff]);
    stroke
}

fn decode_hex(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0);
    value
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).expect("valid byte hex")
        })
        .collect()
}

fn decode_little_endian_word(value: &str) -> u32 {
    u32::from_le_bytes(decode_hex(value).try_into().expect("one f32 word"))
}

fn native_upload_records(diagnostic: &serde_json::Value) -> Vec<[u32; 7]> {
    let upload = &diagnostic["state"]["current"]["native_upload_checkpoint"];
    let raw = decode_hex(
        upload["vector_hex"]
            .as_str()
            .expect("retained upload bytes"),
    );
    assert_eq!(upload["vector_bytes"].as_u64(), Some(2128));
    assert_eq!(sha256(&raw), NATIVE_VECTOR_SHA256);
    assert_eq!(raw.len(), 76 * 7 * 4);
    raw.as_chunks::<28>()
        .0
        .iter()
        .map(|record| {
            std::array::from_fn(|lane| {
                u32::from_le_bytes(record[lane * 4..lane * 4 + 4].try_into().unwrap())
            })
        })
        .collect()
}

fn compare_plan_to_native(plan: &V16Plan, diagnostic: &serde_json::Value) {
    let native = native_upload_records(diagnostic);
    let arguments = diagnostic["state"]["current"]["rtv_addpoint_arguments"]
        .as_array()
        .expect("captured RTV5 AddPoint inputs");
    assert_eq!(plan.stamps.len(), 76);
    assert_eq!(native.len(), plan.stamps.len());
    assert_eq!(arguments.len(), plan.stamps.len());
    for (index, ((stamp, record), args)) in
        plan.stamps.iter().zip(&native).zip(arguments).enumerate()
    {
        let actual = [
            stamp.x.to_bits(),
            stamp.y.to_bits(),
            stamp.extent_x.to_bits(),
            stamp.extent_y.to_bits(),
            1.0_f32.to_bits(),
            stamp.gradient_key.inner,
            stamp.gradient_key.alpha,
        ];
        assert_eq!(actual, *record, "V16Plan/native upload record {index}");
        let abi = &args["native_addpoint_abi_arguments"];
        assert_eq!(abi["float_argument_f32_bits"], "0000803f");
        assert_eq!(
            stamp.x.to_bits(),
            decode_little_endian_word(abi["vector3_argument_f32_bits"][0].as_str().unwrap(),),
            "AddPoint x {index}"
        );
        assert_eq!(
            stamp.y.to_bits(),
            decode_little_endian_word(abi["vector3_argument_f32_bits"][1].as_str().unwrap(),),
            "AddPoint y {index}"
        );
    }
}

fn svg_matrix_from_native(matrix: &[f64], viewport: &[u32]) -> [f64; 6] {
    assert_eq!(matrix.len(), 16);
    assert_eq!(viewport.len(), 4);
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

fn retained_scene(
    prepared: &crate::PreparedStroke<'_>,
    svg_transform: [f64; 6],
    expect_circle_sidecar: bool,
) -> crate::render::RenderedScene {
    let mut scene = Scene::new(
        Svg::new()
            .width(WIDTH)
            .height(HEIGHT)
            .view_box(ViewBox::new(
                0.0,
                0.0,
                f64::from(WIDTH),
                f64::from(HEIGHT),
                1,
            )),
    );
    scene.retain_text();
    // Matrix/viewport evidence maps source pixels directly to GL framebuffer
    // pixels. glReadPixels begins at lower-left, so SVG applies one Y reversal.
    scene.scope(
        Group::new().transformed(Transform::matrix_unrounded(svg_transform)),
        |svg| {
            assert!(render(svg, prepared, false, None));
        },
    );
    let text = scene.take_native_text();
    let mut ink = scene.take_native_ink();
    let text_error = scene.take_native_text_error();
    assert!(
        text_error.is_none(),
        "retained V16 scene text state: {text_error:?}"
    );
    if expect_circle_sidecar {
        let entries = ink.iter().collect::<Vec<_>>();
        assert_eq!(entries.len(), 1, "one experimental circle sidecar");
        let crate::render::NativeInkPayload::RadialCircles {
            records,
            saved_alpha,
        } = &entries[0].1.payload
        else {
            panic!("experimental PDF must use the existing radial-circle payload");
        };
        assert_eq!(records.len(), 76);
        assert_eq!(*saved_alpha, 1.0);
        assert!(records.iter().all(|record| {
            record[4] == 0.0 && record[5] == 1.0 && record[6..9] == [0.0, 0.0, 0.0]
        }));
    } else {
        // Preserve the historical radial-shading PDF as a comparator after
        // production admits full-coverage circles for ordinary V16 color.
        ink = Default::default();
    }
    crate::render::RenderedScene {
        page: crate::RenderedPage::new(0, WIDTH, HEIGHT, scene.finish()),
        text,
        ink,
        text_error,
    }
}

fn raster_svg(svg: &str) -> Vec<u8> {
    let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default())
        .expect("captured V16 SVG parses");
    let mut pixmap = resvg::tiny_skia::Pixmap::new(WIDTH, HEIGHT).expect("bounded canvas");
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    pixmap.data().to_vec()
}

fn topdown_frame(raw: &[u8]) -> Vec<u8> {
    assert_eq!(raw.len(), (WIDTH * HEIGHT * 4) as usize);
    let row = (WIDTH * 4) as usize;
    let mut result = vec![0; raw.len()];
    for y in 0..HEIGHT as usize {
        let source = (HEIGHT as usize - 1 - y) * row;
        let destination = y * row;
        result[destination..destination + row].copy_from_slice(&raw[source..source + row]);
    }
    result
}

fn vector_pdf(retained: &crate::render::RenderedScene) -> (Vec<u8>, usize, usize) {
    let bytes =
        crate::pdf::render_native_scene_pdf_for_test(retained, &crate::PdfOptions::default())
            .expect("retained V16 scene exports to PDF");
    let pdf = lopdf::Document::load_mem(&bytes).expect("PDF parses");
    assert_eq!(pdf.get_pages().len(), 1);
    let mut images = 0;
    let mut shadings = 0;
    for object in pdf.objects.values() {
        if let Ok(stream) = object.as_stream() {
            images += usize::from(
                stream
                    .dict
                    .get(b"Subtype")
                    .and_then(lopdf::Object::as_name)
                    .ok()
                    == Some(b"Image".as_slice()),
            );
        }
        if let Ok(dict) = object.as_dict() {
            shadings += usize::from(
                dict.get(b"ShadingType")
                    .and_then(lopdf::Object::as_i64)
                    .ok()
                    == Some(3),
            );
        }
    }
    assert_eq!(images, 0, "candidate PDF remains vector");
    assert!(shadings > 0, "V16 radial coverage stays as PDF shading");
    (bytes, images, shadings)
}

fn vector_circle_pdf(retained: &crate::render::RenderedScene) -> (Vec<u8>, usize, usize, usize) {
    let bytes =
        crate::pdf::render_native_scene_pdf_for_test(retained, &crate::PdfOptions::default())
            .expect("experimental uniform-black circle scene exports to PDF");
    let pdf = lopdf::Document::load_mem(&bytes).expect("circle PDF parses");
    assert_eq!(pdf.get_pages().len(), 1);
    let mut images = 0;
    let mut circle_forms = 0;
    let mut circle_segments = 0;
    for object in pdf.objects.values() {
        let Ok(stream) = object.as_stream() else {
            continue;
        };
        images += usize::from(
            stream
                .dict
                .get(b"Subtype")
                .and_then(lopdf::Object::as_name)
                .ok()
                == Some(b"Image".as_slice()),
        );
        if stream
            .dict
            .get(b"Subtype")
            .and_then(lopdf::Object::as_name)
            .ok()
            != Some(b"Form".as_slice())
        {
            continue;
        }
        let Ok(group) = stream.dict.get(b"Group").and_then(lopdf::Object::as_dict) else {
            continue;
        };
        if group.get(b"S").and_then(lopdf::Object::as_name).ok() != Some(b"Transparency".as_slice())
            || !matches!(group.get(b"I"), Ok(lopdf::Object::Boolean(true)))
            || !matches!(group.get(b"K"), Ok(lopdf::Object::Boolean(false)))
        {
            continue;
        }
        let content = stream
            .decompressed_content()
            .expect("decompress native-circle form");
        let operations = lopdf::content::Content::decode(&content).expect("circle form operators");
        let black_fill = operations.operations.iter().any(|operation| {
            operation.operator == "rg"
                && operation.operands.len() == 3
                && operation
                    .operands
                    .iter()
                    .all(|value| value.as_float().ok() == Some(0.0))
        });
        let cubics = operations
            .operations
            .iter()
            .filter(|operation| operation.operator == "c")
            .count();
        if black_fill && cubics > 0 {
            circle_forms += 1;
            circle_segments += cubics;
            assert!(
                operations
                    .operations
                    .iter()
                    .all(|operation| operation.operator != "re"),
                "native circle form contains only vector paths"
            );
        }
    }
    assert_eq!(images, 0, "experimental PDF remains raster-free");
    assert_eq!(circle_forms, 1, "one native radial-circle Form");
    assert_eq!(
        circle_segments,
        76 * 4,
        "four cubic segments per native stamp"
    );
    (bytes, images, circle_forms, circle_segments)
}

fn uniform_black_circle_prepared<'a>(
    prepared: &crate::PreparedStroke<'a>,
    diagnostic: &serde_json::Value,
) -> (crate::PreparedStroke<'a>, serde_json::Value) {
    let candidate = prepared.clone();
    let plan = V16Plan::prepare(&candidate).expect("ordinary uniform-color V16 circle plan");
    let records = plan
        .radial_circle_records()
        .expect("native full-coverage equal-circle admission");
    assert_eq!(records.len(), 76);
    let args = diagnostic["state"]["current"]["rtv_addpoint_arguments"]
        .as_array()
        .expect("captured native AddPoint inputs");
    assert_eq!(args.len(), records.len());
    let upload = native_upload_records(diagnostic);
    assert_eq!(upload.len(), records.len());
    let mut radius_mismatch_indices = Vec::new();
    let mut max_radius_ulp_distance = 0_u32;
    for (index, (record, argument)) in records.iter().zip(args).enumerate() {
        assert_eq!(record[4], 0.0, "radial edge lane {index}");
        assert_eq!(record[5], 1.0, "full coverage lane {index}");
        assert_eq!(
            &record[6..9],
            &[0.0, 0.0, 0.0],
            "native uniform black {index}"
        );
        // The existing V16 circle helper derives its radius from the exact
        // uploaded outer extent. Keep x/y/extents bit-exact with that upload;
        // report, but do not gate on, round-trip differences from AddPoint's
        // original radius lane (f32 extent +/- fringe is not invertible).
        assert_eq!(record[0].to_bits(), upload[index][0], "native x {index}");
        assert_eq!(record[1].to_bits(), upload[index][1], "native y {index}");
        assert_eq!(
            circle_plan_extent_x_bits(&plan, index),
            upload[index][2],
            "native uploaded extent x {index}"
        );
        assert_eq!(
            circle_plan_extent_y_bits(&plan, index),
            upload[index][3],
            "native uploaded extent y {index}"
        );
        let native_radius = decode_little_endian_word(
            argument["native_addpoint_abi_arguments"]["vector3_argument_f32_bits"][2]
                .as_str()
                .unwrap(),
        );
        let distance = record[2].to_bits().abs_diff(native_radius);
        if distance != 0 {
            radius_mismatch_indices.push(index);
        }
        max_radius_ulp_distance = max_radius_ulp_distance.max(distance);
        assert_eq!(
            argument["native_addpoint_abi_arguments"]["float_argument_f32_bits"], "0000803f",
            "native unit transparency {index}"
        );
    }
    (
        candidate,
        serde_json::json!({
            "circle_radius_source": "existing V16Plan radial_circle_records: uploaded outer extent minus half inverse-scale fringe",
            "native_addpoint_radius_lane_exact_match_required": false,
            "bitwise_circle_radius_mismatch_count": radius_mismatch_indices.len(),
            "max_positive_f32_ulp_distance": max_radius_ulp_distance,
            "mismatch_stamp_indices": radius_mismatch_indices,
            "uploaded_center_and_outer_extent_bits_remain_exact": true,
        }),
    )
}

fn circle_plan_extent_x_bits(plan: &V16Plan, index: usize) -> u32 {
    plan.stamps[index].extent_x.to_bits()
}

fn circle_plan_extent_y_bits(plan: &V16Plan, index: usize) -> u32 {
    plan.stamps[index].extent_y.to_bits()
}

fn captured_frame(capture_dir: &Path, diagnostic: &serde_json::Value, index: usize) -> Vec<u8> {
    let artifact = diagnostic["rgba_artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|artifact| artifact["case_index"].as_u64() == Some(index as u64))
        .expect("paired native frame artifact");
    let path = capture_dir.join(
        Path::new(artifact["path"].as_str().unwrap())
            .file_name()
            .unwrap(),
    );
    let compressed = fs::read(path).expect("compressed native Mesa frame");
    assert_eq!(sha256(&compressed), artifact["compressed_sha256"]);
    let mut decoder = ZlibDecoder::new(compressed.as_slice());
    let mut raw = Vec::new();
    decoder.read_to_end(&mut raw).expect("decode native frame");
    assert_eq!(raw.len(), artifact["raw_bytes"].as_u64().unwrap() as usize);
    assert_eq!(sha256(&raw), NATIVE_FRAME_SHA256);
    raw
}

#[test]
#[ignore = "writes paired captured AS18 GLV16 SVG/PDF artifacts when capture/output directories are set"]
fn write_as18_glv16_capture_svg_pdf_pairs() {
    let capture_dir = PathBuf::from(
        std::env::var_os("SDOCX_FOUNTAIN_V16_204_CAPTURE_DIR")
            .expect("set SDOCX_FOUNTAIN_V16_204_CAPTURE_DIR to the V14 capture directory"),
    );
    let baseline_path = PathBuf::from(
        std::env::var_os("SDOCX_FOUNTAIN_V16_204_BASELINE_RECEIPT")
            .expect("set SDOCX_FOUNTAIN_V16_204_BASELINE_RECEIPT to the pinned HF02 receipt"),
    );
    let output_dir = PathBuf::from(
        std::env::var_os("SDOCX_FOUNTAIN_V16_204_ARTIFACT_DIR")
            .expect("set SDOCX_FOUNTAIN_V16_204_ARTIFACT_DIR to a fresh output directory"),
    );
    fs::create_dir(&output_dir).expect("artifact output directory must be new");
    let diagnostic_bytes = fs::read(capture_dir.join("as18-v16-204-incomplete-diagnostic.json"))
        .expect("V14 incomplete diagnostic receipt");
    let diagnostic: serde_json::Value = serde_json::from_slice(&diagnostic_bytes).unwrap();
    let baseline_bytes = fs::read(&baseline_path).expect("pinned visible baseline receipt");
    assert_eq!(
        sha256(&baseline_bytes),
        "a631d4389c60b5188fc87119c0d196962bb4674848e315082206899055f408b7"
    );
    let baseline: serde_json::Value = serde_json::from_slice(&baseline_bytes).unwrap();
    assert_eq!(
        baseline["source_input"]["payload_sha256"],
        SOURCE_PAYLOAD_SHA256
    );
    assert_eq!(
        baseline["source_input"]["frame_sha256"],
        SOURCE_FRAME_SHA256
    );
    assert_eq!(baseline["source_input"]["point_count"], 204);
    assert_eq!(baseline["cases"][0]["selected_drawable_class"], "GLV16");
    assert_eq!(baseline["cases"][0]["hf02_visible"], 1);
    assert_eq!(
        baseline["cases"][0]["renderer_readback"]["rgba_sha256"],
        NATIVE_FRAME_SHA256
    );
    assert_eq!(
        baseline["cases"][0]["controlled_preclear_upload"]["vector_sha256"],
        NATIVE_VECTOR_SHA256
    );
    assert_eq!(
        diagnostic["state"]["source_payload_sha256"],
        SOURCE_PAYLOAD_SHA256
    );
    assert_eq!(
        diagnostic["state"]["source_frame_sha256"],
        SOURCE_FRAME_SHA256
    );
    assert_eq!(diagnostic["state"]["source_point_count"], 204);
    assert_eq!(diagnostic["state"]["current"]["case"], "rainbow_on");
    assert_eq!(
        diagnostic["state"]["current"]["redraw_entries"][0]["entry"],
        "SPen::FountainPenStrokeDrawableGLV16::RedrawPen(SPen::ObjectStroke const*, SPen::RectF*)"
    );
    assert_eq!(
        diagnostic["state"]["current"]["viewport_at_readback"],
        serde_json::json!([0, 0, WIDTH, HEIGHT])
    );
    assert_eq!(
        diagnostic["state"]["current"]["renderer_readback"]["rgba_sha256"],
        NATIVE_FRAME_SHA256
    );

    let gl = diagnostic["state"]["current"]["renderer_gl_events"]
        .as_array()
        .unwrap();
    assert!(gl.iter().any(|event| event["call"] == "glViewport"
        && event["values"] == serde_json::json!([0, 0, WIDTH, HEIGHT])));
    let matrix = gl
        .iter()
        .find(|event| event["call"] == "glUniformMatrix4fv" && event["location"] == 0)
        .unwrap();
    assert_eq!(matrix["program"], 6);
    // glUniformMatrix4fv consumes f32 values. Recover that recorded precision
    // before comparing or deriving the mapping; JSON's f64 parser can differ
    // by one f64 ULP without changing any native uniform bits.
    let matrix_values = matrix["values"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| f64::from(value.as_f64().unwrap() as f32))
        .collect::<Vec<_>>();
    let matrix_words = matrix_values
        .iter()
        .map(|value| (*value as f32).to_bits())
        .collect::<Vec<_>>();
    assert_eq!(
        matrix_words,
        [
            0x3a88_8889,
            0x0000_0000,
            0x0000_0000,
            0x0000_0000,
            0x0000_0000,
            0x3af2_b9d6,
            0x0000_0000,
            0x0000_0000,
            0x0000_0000,
            0x0000_0000,
            0xb800_0100,
            0x0000_0000,
            0xbf80_0000,
            0xbf80_0000,
            0x0000_0000,
            0x3f80_0000,
        ]
    );
    let color = gl
        .iter()
        .find(|event| event["call"] == "glUniform4fv" && event["location"] == 1)
        .unwrap();
    assert_eq!(color["program"], 6);
    assert_eq!(color["values"], serde_json::json!([0.0, 0.0, 0.0, 1.0]));
    let draw = gl
        .iter()
        .find(|event| event["call"] == "glDrawArraysInstanced")
        .unwrap();
    assert_eq!(draw["state"]["program"], 6);
    assert_eq!(draw["values"], serde_json::json!([5, 0, 4, 76]));
    assert_eq!(
        diagnostic["state"]["current"]["native_upload_checkpoint"]["vector_sha256"],
        NATIVE_VECTOR_SHA256
    );
    let viewport = diagnostic["state"]["current"]["viewport_at_readback"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_u64().unwrap() as u32)
        .collect::<Vec<_>>();
    let svg_transform = svg_matrix_from_native(&matrix_values, &viewport);

    // Both captured native controls use the same saved 204-point arrays and
    // yielded byte-identical Mesa frames. Off is the pinned baseline; on is
    // the V14 incomplete receipt whose only failure is later cache release.
    let native_off = topdown_frame(&captured_frame(&capture_dir, &diagnostic, 0));
    let native_on = topdown_frame(&captured_frame(&capture_dir, &diagnostic, 1));
    assert_eq!(native_off, native_on);
    let mut manifest_cases = Vec::new();
    let mut shared_rust_raster: Option<Vec<u8>> = None;
    let mut shared_svg: Option<String> = None;
    for (name, rainbow, native) in [
        ("rainbow_off", false, &native_off),
        ("rainbow_on", true, &native_on),
    ] {
        let stroke = source_stroke(&baseline, rainbow);
        let prepared = if rainbow {
            crate::ink::prepare_stroke_for_context_with_fountain_v16_rainbow_ignored(
                &stroke,
                crate::ink::InkRenderContext::CanonicalPage,
            )
        } else {
            crate::ink::prepare_stroke_for_context(
                &stroke,
                false,
                crate::ink::InkRenderContext::CanonicalPage,
            )
        };
        let plan = V16Plan::prepare(&prepared).expect("production GLV16 V16Plan");
        assert_eq!(plan.stamps.len(), 76);
        if rainbow {
            compare_plan_to_native(&plan, &diagnostic);
        }
        let retained = retained_scene(&prepared, svg_transform, false);
        let svg = retained.page.svg.clone();
        let raster = raster_svg(&svg);
        let (pdf, image_count, shading_count) = vector_pdf(&retained);

        // Compare the legacy radial-shading PDF with ordinary uniform-color
        // V16 full-coverage circle admission. Both use the same prepared stroke;
        // only the legacy PDF omits the registered sidecar during conversion.
        let (circle_prepared, radius_round_trip) =
            uniform_black_circle_prepared(&prepared, &diagnostic);
        let circle_plan =
            V16Plan::prepare(&circle_prepared).expect("experimental uniform-black V16Plan");
        assert_eq!(circle_plan.stamps.len(), plan.stamps.len());
        let circle_scene = retained_scene(&circle_prepared, svg_transform, true);
        let circle_svg_raster = raster_svg(&circle_scene.page.svg);
        assert_eq!(circle_svg_raster, raster, "uniform-black SVG appearance");
        let (circle_pdf, circle_images, circle_forms, circle_segments) =
            vector_circle_pdf(&circle_scene);
        if let Some(previous) = &shared_rust_raster {
            assert_eq!(*previous, raster, "V16 renderer ignores saved rainbow flag");
        } else {
            shared_rust_raster = Some(raster.clone());
        }
        if let Some(previous) = &shared_svg {
            assert_eq!(*previous, svg, "paired production SVG");
        } else {
            shared_svg = Some(svg.clone());
        }
        let prefix = name.to_owned();
        write_new(output_dir.join(format!("{prefix}.svg")), svg.as_bytes());
        write_new(output_dir.join(format!("{prefix}.pdf")), &pdf);
        write_new(
            output_dir.join(format!("{prefix}.full-coverage-circles-experiment.pdf")),
            &circle_pdf,
        );
        if name == "rainbow_off" {
            write_new(output_dir.join("rust-topdown.rgba"), &raster);
        }
        let native_changed = native
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| **pixel != [0, 0, 0, 0])
            .count();
        let rust_changed = raster
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| **pixel != [0, 0, 0, 0])
            .count();
        manifest_cases.push(serde_json::json!({
            "case": name,
            "captured_native_frame_sha256_topdown": sha256(native),
            "captured_native_nontransparent_pixels": native_changed,
            "rust_svg_raster_sha256_topdown": sha256(&raster),
            "rust_svg_nontransparent_pixels": rust_changed,
            "native_vs_rust_pixel_equality_evaluated": false,
            "source_to_plan_records": 76,
            "native_upload_sha256": NATIVE_VECTOR_SHA256,
            "native_addpoint_input_vs_production_plan_bitwise_compared": rainbow,
            "pdf_pages": 1,
            "pdf_image_objects": image_count,
            "pdf_radial_shadings": shading_count,
            "svg_sha256": sha256(svg.as_bytes()),
            "pdf_sha256": sha256(&pdf),
            "experimental_pdf": {
                "path": format!("{prefix}.full-coverage-circles-experiment.pdf"),
                "representation": "ordinary uniform-color V16 full-coverage radial-circle admission and existing PDF converter",
                "native_uniform_argb": "0xff000000",
                "native_gl_input_color": [0.0, 0.0, 0.0, 1.0],
                "record_source": "same production V16Plan stamps; existing radial_circle_records admission supplies radii",
                "positions_or_radii_fitted": false,
                "radius_round_trip_diagnostic": radius_round_trip,
                "native_full_coverage_records": circle_plan.stamps.len(),
                "registered_radial_circle_forms": circle_forms,
                "cubic_segments": circle_segments,
                "pdf_image_objects": circle_images,
                "pdf_vector_only": circle_images == 0 && circle_forms == 1,
                "svg_scene_raster_matches_ordinary_baseline": circle_svg_raster == raster,
                "pdf_sha256": sha256(&circle_pdf),
            },
        }));
    }
    let manifest = serde_json::json!({
        "schema": "sdocx.fountain.as18-glv16.capture-svg-pdf-artifacts.v1",
        "scope": "source arrays from pinned 204-point saved HF02 frame through the production GLV16 V16Plan; one captured GLV16/RTV5 native upload and paired host Mesa frames",
        "primitive_boundary": "class-level V16 stamp export; this does not prove Samsung device/PDF parity or independent native GPU implementation",
        "source_payload_sha256": SOURCE_PAYLOAD_SHA256,
        "source_frame_sha256": SOURCE_FRAME_SHA256,
        "source_point_count": 204,
        "native_upload_sha256": NATIVE_VECTOR_SHA256,
        "native_rgba_sha256": NATIVE_FRAME_SHA256,
        "native_saved_argb": "0xff000000",
        "native_canvas_px": [WIDTH, HEIGHT],
        "native_viewport": [0, 0, WIDTH, HEIGHT],
        "native_gl_uniform_matrix4fv_program6_location0_f32": matrix_values,
        "svg_transform_from_native_matrix_and_gl_read_origin": svg_transform,
        "native_gl_input_color": color["values"],
        "native_draw": draw["values"],
        "native_rows_flipped_once_for_topdown_artifacts": true,
        "off_on_native_frames_bitwise_equal": true,
        "native_execution": false,
        "artifact_render_executed": true,
        "capture_replay": false,
        "cases": manifest_cases,
    });
    write_new(
        output_dir.join("manifest.json"),
        &serde_json::to_vec_pretty(&manifest).unwrap(),
    );
}
