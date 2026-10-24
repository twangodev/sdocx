//! Opt-in paired SVG/PDF export artifacts for the captured Fountain V17 source
//! producer and Mesa replay. This is test-only and makes no model admission
//! change. Run with the V7 producer/Mesa directories and a fresh artifact
//! directory.

use super::super::vector::{ColorValue, Rectangle, Styled, Svg, ViewBox};
use super::{Group, Paint, Scene, Transform, V16Plan, render};
use crate::{Point, Stroke};
use base64::Engine as _;
use sha2::{Digest, Sha256};
use std::{fs, fs::OpenOptions, io::Write, path::PathBuf};

const WIDTH: u32 = 128;
const HEIGHT: u32 = 96;
const RECORD_BYTES: usize = 40;
const PRODUCER_SHA256: &str = "223a7fb07ca15b1d4be44f2d8c3e15ca7a34008fe5bf4b9d5659b91463fab9eb";
const PRODUCER_RECEIPT_SHA256: &str =
    "387e473f18671a3c33536bf3dfaa12dedcc2e910caa45246aafea2280d5772f3";
const MESA_SOURCE_RECEIPT_SHA256: &str =
    "d52ebbcc856fce66c1b2b78ac308760840effa8c8ccd12c8a83e7da37a5265fe";

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn write_new(path: PathBuf, bytes: &[u8]) {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .expect("artifact path must be new");
    file.write_all(bytes).expect("write bounded artifact");
    file.sync_all().expect("sync bounded artifact");
}

fn json_bytes(value: &serde_json::Value) -> Vec<u8> {
    serde_json::to_vec_pretty(value).expect("serialize bounded artifact manifest")
}

fn source_stroke(source_input: &serde_json::Value) -> Stroke {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("../../../../conformance/fountain-v16.json"))
            .expect("pinned Fountain V16 base fixture parses");
    let mut stroke: Stroke = serde_json::from_value(reference["stroke"].clone())
        .expect("pinned Fountain V16 source stroke deserializes");
    stroke.points = source_input["points"]
        .as_array()
        .expect("captured source point array")
        .iter()
        .map(|point| Point {
            x: point[0].as_f64().expect("source x"),
            y: point[1].as_f64().expect("source y"),
        })
        .collect();
    stroke.pressures = source_input["pressures"]
        .as_array()
        .expect("captured pressure array")
        .iter()
        .map(|value| value.as_f64().expect("source pressure"))
        .collect();
    stroke.timestamps = source_input["timestamps"]
        .as_array()
        .expect("captured timestamp array")
        .iter()
        .map(|value| value.as_i64().expect("source timestamp"))
        .collect();
    stroke.tilts = vec![0.0; stroke.points.len()];
    stroke.orientations.clear();
    stroke.pen_width = source_input["pen_width"]
        .as_f64()
        .expect("captured pen width") as f32;
    stroke.color = None;

    let rendering = stroke.rendering.as_mut().expect("saved Fountain rendering");
    rendering.advanced_settings = Some("18;0;100;".into());
    rendering.tool_type_raw = 2;
    rendering.pen_name = Some("com.samsung.android.sdk.pen.pen.preload.FountainPen".into());
    rendering.properties.rainbow_effect = true;
    rendering.properties.fixed_width = source_input["fixed_width_enabled"]
        .as_bool()
        .expect("fixed width setting");
    rendering.properties.stylus_channels = true;
    rendering.properties.eraser = false;
    rendering.properties.fixed_opacity = false;
    rendering.properties.alpha_lock = false;
    rendering.properties.replay_only = false;
    rendering.properties.reveal_mode = false;
    rendering.properties.top_layer_pen = false;
    rendering.properties.generated = false;
    rendering.properties.binary_added = true;
    rendering.properties.straighten = false;
    rendering.style.pen_size = Some(stroke.pen_width);
    rendering.style.initial_tolerance = Some(
        source_input["initial_tolerance"]
            .as_f64()
            .expect("captured tolerance") as f32,
    );
    stroke
}

fn palette(source_input: &serde_json::Value) -> Vec<u32> {
    source_input["palette"]
        .as_array()
        .expect("captured palette")
        .iter()
        .map(|color| {
            u32::from_str_radix(color.as_str().expect("ARGB palette string"), 16)
                .expect("valid captured ARGB")
        })
        .collect()
}

fn decoded_native_records(case: &serde_json::Value) -> Vec<[f32; 10]> {
    let raw = base64::engine::general_purpose::STANDARD
        .decode(
            case["native_update_records_base64"]
                .as_str()
                .expect("packed native Update bytes"),
        )
        .expect("native Update base64");
    assert_eq!(
        raw.len(),
        case["native_source_result"]["mesh_bytes"].as_u64().unwrap() as usize
    );
    assert_eq!(raw.len() % RECORD_BYTES, 0);
    assert_eq!(sha256(&raw), case["native_update_records_sha256"]);
    raw.as_chunks::<RECORD_BYTES>()
        .0
        .iter()
        .map(|chunk| {
            std::array::from_fn(|lane| {
                f32::from_le_bytes(chunk[lane * 4..lane * 4 + 4].try_into().unwrap())
            })
        })
        .collect()
}

fn assert_mesa_source_records(records: &[[f32; 10]], mesa_case: &serde_json::Value) {
    let mesa_records = mesa_case["source_update_records"]
        .as_array()
        .expect("Mesa decoded source Update records");
    assert_eq!(mesa_records.len(), records.len());
    for (index, (native, row)) in records.iter().zip(mesa_records).enumerate() {
        assert_eq!(row["index"].as_u64(), Some(index as u64));
        let expected = [
            row["center"][0].as_f64().unwrap() as f32,
            row["center"][1].as_f64().unwrap() as f32,
            row["support_extent"][0].as_f64().unwrap() as f32,
            row["support_extent"][1].as_f64().unwrap() as f32,
            row["stamp_opacity_input"].as_f64().unwrap() as f32,
            row["edge"].as_f64().unwrap() as f32,
            row["coverage_alpha"].as_f64().unwrap() as f32,
            row["rgb"][0].as_f64().unwrap() as f32,
            row["rgb"][1].as_f64().unwrap() as f32,
            row["rgb"][2].as_f64().unwrap() as f32,
        ];
        assert_eq!(
            native.map(f32::to_bits),
            expected.map(f32::to_bits),
            "Mesa record {index}"
        );
        let alpha_word = native[6]
            .to_le_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(row["coverage_alpha_f32_word"], alpha_word);
    }
}

fn compare_prepared_to_native<'a>(
    stroke: &'a Stroke,
    source_input: &serde_json::Value,
    native: &[[f32; 10]],
) -> (crate::ink::PreparedStroke<'a>, serde_json::Value) {
    let colors = palette(source_input);
    let period = source_input["period"].as_i64().unwrap() as i32;
    let offset = source_input["offset"].as_f64().unwrap() as f32;
    let prepared = crate::ink::prepare_stroke_for_context_with_fountain_v17_rainbow(
        stroke,
        crate::ink::InkRenderContext::CanonicalPage,
        &colors,
        period,
        offset,
        Some(1.0),
    );
    assert_eq!(prepared.points.len(), native.len(), "native stamp count");
    let plan = V16Plan::prepare(&prepared).expect("bounded Fountain V17 paint plan");
    assert_eq!(plan.stamps.len(), native.len());
    let colors = prepared
        .fountain_v17_rgb
        .as_ref()
        .expect("explicit Fountain V17 per-stamp colors");
    assert_eq!(colors.len(), native.len());
    for (index, (stamp, row)) in plan.stamps.iter().zip(native).enumerate() {
        let [red, green, blue] = stamp.gradient_key.rgb.expect("rainbow color key");
        let actual = [
            stamp.x,
            stamp.y,
            stamp.extent_x,
            stamp.extent_y,
            1.0_f32,
            0.0_f32,
            f32::from_bits(stamp.gradient_key.alpha),
            f32::from_bits(red),
            f32::from_bits(green),
            f32::from_bits(blue),
        ];
        assert_eq!(
            actual.map(f32::to_bits),
            row.map(f32::to_bits),
            "exact Rust/native Update record lane bits at stamp {index}"
        );
    }
    let summary = serde_json::json!({
        "prepared_record_count": plan.stamps.len(),
        "native_record_count": native.len(),
        "record_lanes_bitwise_equal": true,
        "inverse_scale": 1.0,
        "enhanced_aa": false,
        "profile": {"palette_argb": colors_argb(source_input), "period": period, "initial_offset": offset},
    });
    (prepared, summary)
}

fn colors_argb(source_input: &serde_json::Value) -> Vec<String> {
    source_input["palette"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap().to_owned())
        .collect()
}

fn transformed_scene(prepared: &crate::ink::PreparedStroke<'_>) -> crate::render::RenderedScene {
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
    // Use the SDK's retained-scene PDF path. Partial-coverage stamps remain
    // radial SVG gradients, without manufacturing native-ink markers.
    scene.retain_text();
    // Native target clear color is premultiplied [.04,.08,.12,.4]; render the
    // same straight RGB/opacity as an SVG page background.
    scene.push(
        Rectangle::new()
            .x(0)
            .y(0)
            .width(WIDTH)
            .height(HEIGHT)
            .fill(Paint::Solid(
                ColorValue::rgb_components(0.1, 0.2, 0.3).unwrap(),
            ))
            .fill_opacity(0.4),
    );
    // Recorded native matrix is identity with translation (32,32) in GL-up
    // coordinates. Reverse the framebuffer rows once to canonical SVG top-down
    // coordinates, yielding [1,0,0,-1,32,64].
    let projection = Transform::matrix_unrounded([1.0, 0.0, 0.0, -1.0, 32.0, 64.0]);
    scene.scope(Group::new().transformed(projection), |svg| {
        assert!(render(svg, prepared, false, None));
    });
    let text = scene.take_native_text();
    let ink = scene.take_native_ink();
    let text_error = scene.take_native_text_error();
    assert!(
        text_error.is_none(),
        "retained Fountain scene text state: {text_error:?}"
    );
    assert!(
        ink.is_empty(),
        "partial-alpha Fountain stamps should remain SVG gradients"
    );
    crate::render::RenderedScene {
        page: crate::RenderedPage::new(0, WIDTH, HEIGHT, scene.finish()),
        text,
        ink,
        text_error,
    }
}

fn raster_svg(svg: &str) -> Vec<u8> {
    let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default())
        .expect("Rust SVG parses for bounded raster comparison");
    let mut pixmap = resvg::tiny_skia::Pixmap::new(WIDTH, HEIGHT).expect("bounded pixmap");
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    pixmap.data().to_vec()
}

fn reverse_native_rows(raw: &[u8]) -> Vec<u8> {
    assert_eq!(raw.len(), (WIDTH * HEIGHT * 4) as usize);
    let row_bytes = (WIDTH * 4) as usize;
    let mut top_down = vec![0; raw.len()];
    for y in 0..HEIGHT as usize {
        let src = (HEIGHT as usize - 1 - y) * row_bytes;
        let dst = y * row_bytes;
        top_down[dst..dst + row_bytes].copy_from_slice(&raw[src..src + row_bytes]);
    }
    top_down
}

fn frame_metrics(native: &[u8], rust: &[u8]) -> serde_json::Value {
    assert_eq!(native.len(), rust.len());
    let background = [10_u8, 20, 31, 102];
    let mut absolute_error_sum = 0_u64;
    let mut max_absolute_error = 0_u8;
    let mut changed_native = 0_usize;
    let mut changed_rust = 0_usize;
    let mut union_pixels = 0_usize;
    let mut native_bbox: Option<[u32; 4]> = None;
    let mut rust_bbox: Option<[u32; 4]> = None;
    let mut changed_bbox: Option<[u32; 4]> = None;
    for pixel in 0..(WIDTH * HEIGHT) as usize {
        let at = pixel * 4;
        let n = &native[at..at + 4];
        let r = &rust[at..at + 4];
        let n_changed = n != background;
        let r_changed = r != background;
        changed_native += usize::from(n_changed);
        changed_rust += usize::from(r_changed);
        let x = (pixel as u32) % WIDTH;
        let y = (pixel as u32) / WIDTH;
        if n_changed {
            native_bbox = Some(match native_bbox {
                Some([x0, y0, x1, y1]) => [x0.min(x), y0.min(y), x1.max(x), y1.max(y)],
                None => [x, y, x, y],
            });
        }
        if r_changed {
            rust_bbox = Some(match rust_bbox {
                Some([x0, y0, x1, y1]) => [x0.min(x), y0.min(y), x1.max(x), y1.max(y)],
                None => [x, y, x, y],
            });
        }
        if n_changed || r_changed {
            union_pixels += 1;
            changed_bbox = Some(match changed_bbox {
                Some([x0, y0, x1, y1]) => [x0.min(x), y0.min(y), x1.max(x), y1.max(y)],
                None => [x, y, x, y],
            });
        }
        for lane in 0..4 {
            let error = n[lane].abs_diff(r[lane]);
            absolute_error_sum += u64::from(error);
            max_absolute_error = max_absolute_error.max(error);
        }
    }
    serde_json::json!({
        "comparison": "full 128x96 RGBA; native GL rows reversed once to top-down; no crop or phase fit",
        "background_expected_premultiplied_rgba": background,
        "native_changed_pixels_vs_background": changed_native,
        "rust_changed_pixels_vs_background": changed_rust,
        "native_bbox_topdown_xyxy": native_bbox,
        "rust_bbox_topdown_xyxy": rust_bbox,
        "union_bbox_topdown_xyxy": changed_bbox,
        "mean_absolute_channel_error_full_frame": absolute_error_sum as f64 / native.len() as f64,
        "mean_absolute_channel_error_over_union": absolute_error_sum as f64 / (union_pixels.max(1) * 4) as f64,
        "max_absolute_channel_error": max_absolute_error,
        "pixels_in_error_union": union_pixels,
    })
}

#[test]
#[ignore = "writes paired captured Fountain V17 SVG/PDF artifacts when capture/output directories are set"]
fn write_fountain_v17_capture_svg_pdf_pairs() {
    let producer_dir = PathBuf::from(
        std::env::var_os("SDOCX_FOUNTAIN_V17_PRODUCER_DIR")
            .expect("set SDOCX_FOUNTAIN_V17_PRODUCER_DIR to the V7 producer capture directory"),
    );
    let mesa_dir = PathBuf::from(
        std::env::var_os("SDOCX_FOUNTAIN_V17_MESA_DIR")
            .expect("set SDOCX_FOUNTAIN_V17_MESA_DIR to the V7 Mesa capture directory"),
    );
    let output_dir = PathBuf::from(
        std::env::var_os("SDOCX_FOUNTAIN_V17_ARTIFACT_DIR")
            .expect("set SDOCX_FOUNTAIN_V17_ARTIFACT_DIR to a fresh output directory"),
    );
    fs::create_dir(&output_dir).expect("artifact output directory must not already exist");
    let producer_path = producer_dir.join("source-producer-receipt.json");
    let mesa_path = mesa_dir.join("receipt.json");
    let producer_bytes = fs::read(&producer_path).expect("V7 producer receipt");
    let mesa_bytes = fs::read(&mesa_path).expect("V7 Mesa receipt");
    assert_eq!(sha256(&producer_bytes), PRODUCER_RECEIPT_SHA256);
    assert_eq!(sha256(&mesa_bytes), MESA_SOURCE_RECEIPT_SHA256);
    let producer: serde_json::Value = serde_json::from_slice(&producer_bytes).unwrap();
    let mesa: serde_json::Value = serde_json::from_slice(&mesa_bytes).unwrap();
    assert_eq!(producer["producer_script_sha256"], PRODUCER_SHA256);
    assert_eq!(
        producer["schema"],
        "sdocx.fountain.v17.source-alpha-producer.v6"
    );
    assert_eq!(producer["status"], "captured_source_producer_only");
    assert_eq!(
        mesa["source_producer_receipt_sha256"],
        sha256(&producer_bytes)
    );
    assert_eq!(mesa["status"], "captured");
    assert_eq!(
        mesa["limits"]["surface_px"],
        serde_json::json!([WIDTH, HEIGHT])
    );
    assert_eq!(
        mesa["stage_boundary"]["producer_and_draw_are_separate_native_instances"],
        true
    );
    assert_eq!(mesa["stage_boundary"]["producer_GL_Draw"], false);
    assert_eq!(
        mesa["stage_boundary"]["exact_40_byte_mesh_hash_and_count_required_before_Draw"],
        true
    );
    assert!(
        mesa["execution_boundary"]["native_executed"]
            .as_array()
            .unwrap()
            .iter()
            .any(|stage| stage == "Draw")
    );
    assert_eq!(mesa["execution_boundary"]["native_Init_executed"], false);
    assert_eq!(producer["cases"].as_array().unwrap().len(), 2);
    assert_eq!(mesa["cases"].as_array().unwrap().len(), 2);

    let mut manifest_cases = Vec::new();
    for (index, (producer_case, mesa_case)) in producer["cases"]
        .as_array()
        .unwrap()
        .iter()
        .zip(mesa["cases"].as_array().unwrap())
        .enumerate()
    {
        let name = producer_case["name"].as_str().unwrap();
        assert_eq!(mesa_case["name"], name);
        assert_eq!(
            producer_case["native_update_records_sha256"],
            mesa_case["source_update_records_sha256"]
        );
        assert_eq!(
            producer_case["native_update_records_sha256"],
            mesa_case["native_records_sha256"]
        );
        assert_eq!(
            producer_case["source_addpoint_count"],
            mesa_case["source_update_count"]
        );
        assert_eq!(mesa_case["enhanced_aa"], false);
        assert_eq!(mesa_case["inverse_scale_xy"], serde_json::json!([1.0, 1.0]));
        assert_eq!(mesa_case["draw_calls"].as_array().unwrap().len(), 1);
        assert_eq!(mesa_case["gl_error"], 0);
        assert_eq!(
            mesa_case["destination"]["dimensions"],
            serde_json::json!([WIDTH, HEIGHT])
        );
        assert_eq!(
            mesa_case["destination"]["background_premul"],
            serde_json::json!([0.04, 0.08, 0.12, 0.4])
        );
        let native = decoded_native_records(producer_case);
        assert_eq!(
            native.len(),
            mesa_case["source_update_count"].as_u64().unwrap() as usize
        );
        assert_mesa_source_records(&native, mesa_case);
        let stroke = source_stroke(&producer_case["source_input"]);
        let (prepared, geometry_summary) =
            compare_prepared_to_native(&stroke, &producer_case["source_input"], &native);
        let retained = transformed_scene(&prepared);
        let svg = retained.page.svg.clone();
        let svg_bytes = svg.as_bytes();
        let raster = raster_svg(&svg);

        let native_path = PathBuf::from(mesa_case["readback_path"].as_str().unwrap());
        assert!(native_path.starts_with(&mesa_dir));
        let native_raw = fs::read(&native_path).expect("captured native Mesa RGBA frame");
        assert_eq!(native_raw.len(), (WIDTH * HEIGHT * 4) as usize);
        assert_eq!(sha256(&native_raw), mesa_case["readback_rgba_sha256"]);
        let native_top_down = reverse_native_rows(&native_raw);
        assert_eq!(
            &native_top_down[..4],
            &[10, 20, 31, 102],
            "recorded native clear color"
        );
        let metrics = frame_metrics(&native_top_down, &raster);
        assert_eq!(
            metrics["native_bbox_topdown_xyxy"],
            serde_json::json!([41, 0, 42, 64])
        );
        assert_eq!(&raster[..4], &[10, 20, 31, 102], "Rust SVG clear color");

        let pdf_bytes =
            crate::pdf::render_native_scene_pdf_for_test(&retained, &crate::PdfOptions::default())
                .expect("SDK retained-scene PDF writer exports Fountain SVG");
        let pdf = lopdf::Document::load_mem(&pdf_bytes).expect("exported PDF parses");
        assert_eq!(pdf.get_pages().len(), 1);
        let mut image_count = 0;
        let mut shading_count = 0;
        for object in pdf.objects.values() {
            if let Ok(stream) = object.as_stream() {
                image_count += usize::from(
                    stream
                        .dict
                        .get(b"Subtype")
                        .and_then(lopdf::Object::as_name)
                        .ok()
                        == Some(b"Image".as_slice()),
                );
            }
            if let Ok(dict) = object.as_dict() {
                shading_count += usize::from(
                    dict.get(b"ShadingType")
                        .and_then(lopdf::Object::as_i64)
                        .ok()
                        == Some(3),
                );
            }
        }
        assert_eq!(
            image_count, 0,
            "captured partial-coverage output remains vector"
        );
        assert!(shading_count > 0, "PDF retains radial vector shadings");

        let prefix = format!("{index}-{name}");
        let svg_path = output_dir.join(format!("{prefix}.svg"));
        let pdf_path = output_dir.join(format!("{prefix}.pdf"));
        let raster_path = output_dir.join(format!("{prefix}.rust.rgba"));
        let native_path_out = output_dir.join(format!("{prefix}.native-topdown.rgba"));
        write_new(svg_path.clone(), svg_bytes);
        write_new(pdf_path.clone(), &pdf_bytes);
        write_new(raster_path.clone(), &raster);
        write_new(native_path_out.clone(), &native_top_down);
        manifest_cases.push(serde_json::json!({
            "name": name,
            "producer_update_sha256": producer_case["native_update_records_sha256"],
            "mesa_source_update_sha256": mesa_case["source_update_records_sha256"],
            "prepared_geometry": geometry_summary,
            "frame_metrics": metrics,
            "native_full_frame_sha256_raw_gl_rows": mesa_case["readback_rgba_sha256"],
            "native_full_frame_sha256_topdown": sha256(&native_top_down),
            "rust_raster_sha256_topdown": sha256(&raster),
            "svg": {"path": svg_path.file_name().unwrap().to_string_lossy(), "sha256": sha256(svg_bytes)},
            "pdf": {"path": pdf_path.file_name().unwrap().to_string_lossy(), "sha256": sha256(&pdf_bytes), "pages": 1, "image_objects": image_count, "radial_shadings": shading_count},
            "native_raster": native_path_out.file_name().unwrap().to_string_lossy(),
            "rust_raster": raster_path.file_name().unwrap().to_string_lossy(),
        }));
    }
    let manifest = serde_json::json!({
        "schema": "sdocx.fountain.v17.capture-svg-pdf-artifacts.v1",
        "scope": "Rust SVG/PDF export checked against captured native Update records and complete 128x96 Mesa readback frames; producer and Draw are separate native stages",
        "producer_receipt_sha256": sha256(&producer_bytes),
        "mesa_receipt_sha256": sha256(&mesa_bytes),
        "producer_executable_sha256": PRODUCER_SHA256,
        "producer_receipt_schema": producer["schema"],
        "matrix_f32_words": mesa["cases"][0]["matrix_f32_words"],
        "svg_projection_after_single_native_row_flip": [1, 0, 0, -1, 32, 64],
        "background_source_straight_rgba": [0.1, 0.2, 0.3, 0.4],
        "native_background_premultiplied_rgba8": [10, 20, 31, 102],
        "native_enhanced_aa": false,
        "cases": manifest_cases,
        "native_execution": false,
        "capture_replay": false,
        "artifact_generation": true,
        "root_invocation_required_for_build_and_artifact_render": true,
    });
    write_new(output_dir.join("manifest.json"), &json_bytes(&manifest));
}
